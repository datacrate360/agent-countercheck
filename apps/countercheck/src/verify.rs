//! The verify flow: lock the checks, snapshot, restore protected files, run, decide, seal.

use crate::{git, golden, runner, scan, snapshot};
use countercheck_core::config::{self, CHECKS_PATH};
use countercheck_core::protect::{Protected, ProtectedChange};
use countercheck_core::receipt::{self, Receipt, RepoInfo, Worker};
use std::path::PathBuf;

/// Matrix files and coverage item lists may be larger than the check file (up to 10,000 items).
const MAX_LIST_BYTES: usize = 1024 * 1024;

pub struct Options {
    pub repo: PathBuf,
    pub base: String,
    pub claim: Option<String>,
    pub worker: Worker,
    pub sandbox: runner::Sandbox,
    pub out: Option<PathBuf>,
}

pub fn verify(opts: &Options) -> Result<(Receipt, PathBuf), String> {
    let root = git::toplevel(&opts.repo)?;
    let base_commit = git::resolve_commit(&root, &opts.base)?;
    let head_commit = git::resolve_commit(&root, "HEAD")?;
    let dirty = git::is_dirty(&root)?;

    // 1. The checks: from the base revision if they exist there (locked), else the worker's copy.
    let (checks_text, checks_locked, checks_source) =
        match git::show(&root, &base_commit, CHECKS_PATH)? {
            Some(bytes) => (
                String::from_utf8_lossy(&bytes).into_owned(),
                true,
                format!("{CHECKS_PATH} @ {}", opts.base),
            ),
            None => match std::fs::read_to_string(root.join(CHECKS_PATH)) {
                Ok(t) => (
                    t,
                    false,
                    format!("{CHECKS_PATH} (working tree, not in base)"),
                ),
                Err(_) => {
                    return Err(format!(
                        "no {CHECKS_PATH} found; run `countercheck init`, edit it and commit it"
                    ));
                }
            },
        };
    let checks = config::parse(&checks_text)?;
    // Matrix files and coverage item lists come from the same place as the check file, so the
    // worker can't shrink the lists.
    let mut locked_texts = std::collections::BTreeMap::new();
    for path in checks.locked_files() {
        let bytes = if checks_locked {
            git::show(&root, &base_commit, &path)?
                .ok_or_else(|| format!("{path} is not in the base revision"))?
        } else {
            std::fs::read(root.join(&path)).map_err(|e| format!("{path}: {e}"))?
        };
        if bytes.len() > MAX_LIST_BYTES {
            return Err(format!("{path} is larger than {MAX_LIST_BYTES} bytes"));
        }
        locked_texts.insert(path, String::from_utf8_lossy(&bytes).into_owned());
    }
    let planned = config::plan(&checks, &locked_texts)?;
    let protected = Protected::new(&checks.protected_patterns())?;

    // 2. A clean copy of the worker's files.
    let scratch = tempfile::Builder::new()
        .prefix("countercheck-")
        .tempdir()
        .map_err(|e| format!("temp dir: {e}"))?;
    let scratch_root = scratch
        .path()
        .canonicalize()
        .map_err(|e| format!("temp dir: {e}"))?;
    let work = scratch_root.join("work");
    let tmp = scratch_root.join("tmp");
    std::fs::create_dir_all(&work).map_err(|e| e.to_string())?;
    std::fs::create_dir_all(&tmp).map_err(|e| e.to_string())?;
    snapshot::copy_files(&root, &work, &git::worktree_files(&root)?)?;

    // 3. Protected files the worker changed go back to their base version.
    let mut protected_changes = Vec::new();
    for (change, path) in git::changes_since(&root, &base_commit)? {
        if countercheck_core::protect::is_junk(&path) || !protected.matches(&path) {
            continue;
        }
        let pc = ProtectedChange::decide(&path, change);
        if pc.action == "restored_from_base" {
            let base_bytes = git::show(&root, &base_commit, &path)?;
            snapshot::restore(&work, &path, base_bytes.as_deref())?;
        }
        protected_changes.push(pc);
    }
    protected_changes.sort_by(|a, b| a.path.cmp(&b.path));
    protected_changes.dedup_by(|a, b| a.path == b.path);
    let tree_sha256 = snapshot::tree_hash(&work)?;
    let mut linked = Vec::new();
    for name in &checks.link {
        if snapshot::link_dir(&root, &work, name)? {
            linked.push(name.clone());
        }
    }

    // 4. Scan the worker's files before any command can change them, then run every check.
    let mut scan_results = Vec::new();
    if !checks.forbids.is_empty() || !checks.coverages.is_empty() {
        let files = scan::files(&work)?;
        scan_results.extend(scan::forbid_all(&checks.forbids, &work, &files)?);
        scan_results.extend(scan::coverage_all(
            &checks.coverages,
            &locked_texts,
            &work,
            &files,
        )?);
    }
    let mut results = Vec::new();
    for spec in &planned {
        let Some(g) = &spec.golden else {
            results.push(runner::run_check(spec, &work, &tmp, opts.sandbox)?);
            continue;
        };
        // The reference comes from the base revision (like the check file) when the checks are
        // locked, so neither the worker nor a check command can change it.
        let expected = if checks_locked {
            match git::show(&root, &base_commit, &g.expected)? {
                Some(bytes) => Ok(bytes),
                None => Err(format!(
                    "the expected file {} is not in the base revision",
                    g.expected
                )),
            }
        } else {
            std::fs::read(work.join(&g.expected))
                .map_err(|_| format!("the expected file {} was not found", g.expected))
        };
        let expected = expected.and_then(|b| {
            if b.len() as u64 > countercheck_core::golden::MAX_FILE_BYTES {
                Err(format!(
                    "the expected file {} is larger than 64 MiB",
                    g.expected
                ))
            } else {
                Ok(b)
            }
        });
        golden::clear_output(&work, g)?;
        let mut result = runner::run_check(spec, &work, &tmp, opts.sandbox)?;
        golden::judge(&work, g, &expected, &mut result);
        results.push(result);
    }
    results.extend(scan_results);

    // 5. Decide, seal, save.
    let (verdict, reasons) = receipt::decide(checks_locked, &results, &protected_changes);
    let claim_status = receipt::claim_status(opts.claim.as_deref(), verdict);
    let receipt = Receipt {
        schema: receipt::SCHEMA.to_string(),
        id: String::new(),
        receipt_sha256: String::new(),
        created_at: humantime::format_rfc3339_seconds(std::time::SystemTime::now()).to_string(),
        countercheck_version: env!("CARGO_PKG_VERSION").to_string(),
        repo: RepoInfo {
            base_rev: opts.base.clone(),
            base_commit,
            head_commit,
            dirty,
        },
        worker: opts.worker.clone(),
        claim: opts.claim.clone(),
        claim_status,
        checks_locked,
        checks_source,
        sandbox: opts.sandbox.label().to_string(),
        tree_sha256,
        linked,
        protected_changes,
        checks: results,
        verdict,
        reasons,
    }
    .seal()?;

    let dir = opts
        .out
        .clone()
        .unwrap_or_else(|| root.join(".countercheck").join("receipts"));
    std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    ensure_receipts_ignored(&root, &dir);
    let path = dir.join(format!("{}.json", receipt.id));
    let text = serde_json::to_string_pretty(&receipt).map_err(|e| e.to_string())?;
    std::fs::write(&path, text).map_err(|e| format!("{}: {e}", path.display()))?;
    Ok((receipt, path))
}

/// Receipts inside the repo get a .gitignore so they never show up as worker changes.
fn ensure_receipts_ignored(root: &std::path::Path, dir: &std::path::Path) {
    if dir.starts_with(root) {
        let gi = dir.join(".gitignore");
        if !gi.exists() {
            let _ = std::fs::write(gi, "*\n");
        }
    }
}
