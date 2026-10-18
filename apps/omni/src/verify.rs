//! The verify flow: lock the checks, snapshot, restore protected files, run, decide, seal.

use crate::{git, runner, snapshot};
use omni_core::config::{self, CHECKS_PATH};
use omni_core::protect::{Protected, ProtectedChange};
use omni_core::receipt::{self, Receipt, RepoInfo, Worker};
use std::path::PathBuf;

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
                        "no {CHECKS_PATH} found; run `omni init`, edit it and commit it"
                    ));
                }
            },
        };
    let checks = config::parse(&checks_text)?;
    let protected = Protected::new(&checks.protected_patterns())?;

    // 2. A clean copy of the worker's files.
    let scratch = tempfile::Builder::new()
        .prefix("omni-verify-")
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
        if omni_core::protect::is_junk(&path) || !protected.matches(&path) {
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

    // 4. Run every check.
    let mut results = Vec::new();
    for spec in &checks.checks {
        results.push(runner::run_check(spec, &work, &tmp, opts.sandbox)?);
    }

    // 5. Decide, seal, save.
    let (verdict, reasons) = receipt::decide(checks_locked, &results, &protected_changes);
    let claim_status = receipt::claim_status(opts.claim.as_deref(), verdict);
    let receipt = Receipt {
        schema: receipt::SCHEMA.to_string(),
        id: String::new(),
        receipt_sha256: String::new(),
        created_at: humantime::format_rfc3339_seconds(std::time::SystemTime::now()).to_string(),
        omni_version: env!("CARGO_PKG_VERSION").to_string(),
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
        .unwrap_or_else(|| root.join(".omni").join("receipts"));
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
