//! The verify flow: lock the checks, snapshot, restore protected files, run, decide, seal.

use crate::progress::Reporter;
use crate::{external, git, golden, runner, scan, snapshot};
use countercheck_core::config::{self, CHECKS_PATH};
use countercheck_core::gitobj::{self, Diff, Files};
use countercheck_core::progress::Stage;
use countercheck_core::protect::{Change, Protected, ProtectedChange};
use countercheck_core::receipt::{self, Receipt, RepoInfo, Worker};
use countercheck_core::{hook_settings, uncovered, watched};
use std::path::PathBuf;

/// Matrix files and coverage item lists may be larger than the check file (up to 10,000 items).
const MAX_LIST_BYTES: usize = 1024 * 1024;

pub struct Options {
    pub repo: PathBuf,
    pub base: String,
    pub claim: Option<String>,
    pub worker: Worker,
    pub sandbox: runner::Sandbox,
    /// The sandbox was asked for by name (`--sandbox windows`): a run it can't confine is an
    /// error, not a run without it.
    pub sandbox_required: bool,
    pub out: Option<PathBuf>,
    /// `--checks-from`: the checks come from outside the repository (ADR-0002).
    pub checks_from: Option<String>,
    /// `--checks-sha256`: the pin for a `--checks-from` file.
    pub checks_sha256: Option<String>,
    /// The revision the agent's hook settings are compared with, when it isn't `base`: the hooks
    /// pass the commit their session started from, which nothing in the project can move, while
    /// `base` may come from `COUNTERCHECK_BASE`, which a settings file's `env` block can set.
    pub settings_base: Option<String>,
}

/// Where the checks and the files they name (matrix files, coverage lists, golden references)
/// are read from.
enum Rules {
    /// The target repository's base revision (locked).
    Base,
    /// The worker's working tree (not locked).
    WorkTree,
    /// Another repository at a pinned commit (`--checks-from <repo>@<commit>:<path>`).
    Suite(Box<git::Revision>),
    /// A single external file: it may not name other files.
    File,
}

pub fn verify(opts: &Options) -> Result<(Receipt, PathBuf), String> {
    let root = git::toplevel(&opts.repo)?;
    let dir = opts
        .out
        .clone()
        .unwrap_or_else(|| root.join(".countercheck").join("receipts"));
    // Before the clean copy, so the receipt folder is git-ignored (and not copied) from the start.
    if std::fs::create_dir_all(&dir).is_ok() {
        ensure_receipts_ignored(&root, &dir);
    }
    // What this run is doing, for the report and live UI; removed when this function returns.
    // First clear what killed runs left behind.
    crate::progress::sweep(&dir);
    let mut progress = Reporter::start(&dir, &opts.worker, opts.claim.as_deref());
    let base_commit = git::resolve_commit(&root, &opts.base)?;
    let head_commit = git::resolve_commit(&root, "HEAD")?;
    let git_dirty = git::is_dirty(&root)?;
    // The base, read from git objects checked against their ids (not the index, filters or
    // replace refs, which the worker can change).
    let mut base = git::Revision::load(&root, &base_commit)?;

    // 1. The checks: from outside the repository when asked (pinned), else from the base revision
    // if they exist there (locked), else the worker's copy.
    let mut checks_external = None;
    let (checks_text, checks_locked, checks_source, mut rules) =
        if let Some(src) = &opts.checks_from {
            let loaded = external::load(src, opts.checks_sha256.as_deref(), &root)?;
            let mut label = loaded.label;
            if base.read(CHECKS_PATH)?.is_some() || root.join(CHECKS_PATH).exists() {
                // External replaces, never merges: a file the worker can edit must not add or
                // soften rules.
                label.push_str(&format!("; the repository's own {CHECKS_PATH} was ignored"));
            }
            checks_external = Some(loaded.info);
            let rules = match loaded.suite {
                Some(rev) => Rules::Suite(Box::new(rev)),
                None => Rules::File,
            };
            (loaded.text, loaded.locked, label, rules)
        } else {
            match base.read(CHECKS_PATH)? {
                Some(bytes) => (
                    String::from_utf8_lossy(&bytes).into_owned(),
                    true,
                    format!("{CHECKS_PATH} @ {}", opts.base),
                    Rules::Base,
                ),
                None => match std::fs::read_to_string(root.join(CHECKS_PATH)) {
                    Ok(t) => (
                        t,
                        false,
                        format!("{CHECKS_PATH} (working tree, not in base)"),
                        Rules::WorkTree,
                    ),
                    Err(_) => {
                        return Err(format!(
                            "no {CHECKS_PATH} found; run `countercheck init`, edit it and commit it"
                        ));
                    }
                },
            }
        };
    let checks = config::parse(&checks_text)?;
    if matches!(rules, Rules::File)
        && (!checks.locked_files().is_empty() || checks.checks.iter().any(|c| c.golden.is_some()))
    {
        return Err(
            "--checks-from: a check file given as a file can't name other files (matrix_file, coverage items, golden); put the suite in a git repository and use <repo>@<commit>:<path>"
                .to_string(),
        );
    }
    // Matrix files and coverage item lists come from the same place as the check file, so the
    // worker can't shrink the lists.
    let mut locked_texts = std::collections::BTreeMap::new();
    for path in checks.locked_files() {
        let bytes = match &mut rules {
            Rules::Base => base
                .read(&path)?
                .ok_or_else(|| format!("{path} is not in the base revision"))?,
            Rules::Suite(suite) => suite
                .read(&path)?
                .ok_or_else(|| format!("{path} is not in the --checks-from commit"))?,
            Rules::WorkTree | Rules::File => std::fs::read(root.join(snapshot::safe_rel(&path)?))
                .map_err(|e| format!("{path}: {e}"))?,
        };
        if bytes.len() > MAX_LIST_BYTES {
            return Err(format!("{path} is larger than {MAX_LIST_BYTES} bytes"));
        }
        locked_texts.insert(path, String::from_utf8_lossy(&bytes).into_owned());
    }
    let planned = config::plan(&checks, &locked_texts)?;
    let sandbox = windows_sh_check(opts, &planned)?;
    let protected = Protected::new(&checks.protected_patterns())?;

    // 2. A clean copy of the worker's files.
    progress.stage(Stage::Copy);
    let scratch = tempfile::Builder::new()
        .prefix("countercheck-")
        .tempdir()
        .map_err(|e| format!("temp dir: {e}"))?;
    let scratch_root = snapshot::canonical(scratch.path()).map_err(|e| format!("temp dir: {e}"))?;
    let work = scratch_root.join("work");
    let tmp = scratch_root.join("tmp");
    std::fs::create_dir_all(&work).map_err(|e| e.to_string())?;
    std::fs::create_dir_all(&tmp).map_err(|e| e.to_string())?;
    // Windows sandbox: the run's SID may write here. Granted while both are empty, so every file
    // copied in inherits it and no existing tree is walked.
    #[cfg(windows)]
    if matches!(
        sandbox,
        runner::Sandbox::RestrictedToken
            | runner::Sandbox::RestrictedTokenPartial
            | runner::Sandbox::WindowsRunner
    ) {
        crate::windows_sandbox::grant_scratch(&work, &tmp)?;
    }
    // The full sandbox: the runner account may write in both too, and runs countercheck from a
    // copy in `bin`, which it may read and run but not change (so a check can't swap it).
    #[cfg(windows)]
    if sandbox == runner::Sandbox::WindowsRunner {
        let c = crate::windows_runner::load()
            .ok_or("the Windows runner account is not set up (countercheck windows-setup)")?;
        for dir in [&work, &tmp] {
            crate::windows_sandbox::grant_full(dir, &c.sid)
                .map_err(|e| format!("Windows sandbox: {e}"))?;
        }
        let exe = crate::windows_runner::helper_copy(&c, &scratch_root.join("bin"))?;
        crate::windows_runner::set_helper(exe);
    }
    let links_as_text = snapshot::copy_files(&root, &work, &git::worktree_files(&root)?)?;

    // 3. What changed: the clean copy's own bytes against the base. Protected files the worker
    // changed go back to their base version.
    progress.stage(Stage::Protect);
    let copied = snapshot::file_ids(&work, base.format(), &links_as_text)?;
    let diff = compare(&mut base, &copied, &work)?;
    let dirty =
        git_dirty || head_differs(&root, &head_commit, &base_commit, &diff, &copied, &work)?;
    let mut protected_changes = Vec::new();
    let mut changes = Vec::new();
    for (d, path) in diff {
        let change = match d {
            Diff::Added => Change::Added,
            Diff::Deleted => Change::Deleted,
            Diff::Modified => Change::Modified,
            Diff::LineEndings => {
                // The same file with CRLF line endings (e.g. `core.autocrlf`): not a change, but
                // a protected file still runs with its exact base bytes.
                if protected.matches(&path) {
                    snapshot::restore(&work, &path, base.read_real(&path)?.as_deref())?;
                }
                continue;
            }
        };
        changes.push((change, path.clone()));
        if countercheck_core::protect::is_junk(&path) || !protected.matches(&path) {
            continue;
        }
        let pc = ProtectedChange::decide(&path, change);
        if pc.action == "restored_from_base" {
            snapshot::restore(&work, &path, base.read_real(&path)?.as_deref())?;
        }
        protected_changes.push(pc);
    }
    // Watched files: the worker's version (in the clean copy) against the base, by line text.
    let watched = if checks.watched.is_empty() {
        None
    } else {
        let globs = Protected::labeled(&checks.watched, "watched pattern")?;
        let mut counts = |path: &str, change: Change| -> Option<(u64, u64)> {
            let was = match change {
                Change::Added => Vec::new(),
                _ => base.read_real(path).ok().flatten()?,
            };
            let now = match change {
                Change::Deleted => Vec::new(),
                _ => std::fs::read(work.join(snapshot::safe_rel(path).ok()?)).ok()?,
            };
            watched::line_delta(&was, &now)
        };
        Some(watched::find(
            &changes,
            &globs,
            &protected,
            checks.watched_mode,
            &mut counts,
        ))
    };
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
        // `scope = "added"` compares with the base. An unreadable base file counts as absent,
        // so every line is new: stricter, never looser.
        let mut base_file = |p: &str| base.read_real(p).ok().flatten();
        scan_results.extend(scan::forbid_all(
            &checks.forbids,
            &work,
            &files,
            &mut base_file,
        )?);
        scan_results.extend(scan::coverage_all(
            &checks.coverages,
            &locked_texts,
            &work,
            &files,
        )?);
    }
    // Extra write locations, checked before anything runs (only the sandbox restricts writes).
    let mut writes = Vec::new();
    for spec in &planned {
        writes.push(if sandbox.confines_writes() {
            runner::resolve_writes(spec, &root)?
        } else {
            Vec::new()
        });
    }
    let mut results = Vec::new();
    for (i, (spec, writes)) in planned.iter().zip(&writes).enumerate() {
        progress.check(i, planned.len(), &spec.name, spec.timeout_secs);
        let Some(g) = &spec.golden else {
            let mut result = runner::run_check(spec, &work, &tmp, sandbox, writes)?;
            note_links_as_text(&mut result, &links_as_text);
            results.push(result);
            continue;
        };
        // The reference comes from the base revision (like the check file) when the checks are
        // locked, so neither the worker nor a check command can change it.
        let expected = if let Rules::Suite(suite) = &mut rules {
            match suite.read_real(&g.expected)? {
                Some(bytes) => Ok(bytes),
                None => Err(format!(
                    "the expected file {} is not in the --checks-from commit",
                    g.expected
                )),
            }
        } else if checks_locked {
            match base.read_real(&g.expected)? {
                Some(bytes) => Ok(bytes),
                None => Err(format!(
                    "the expected file {} is not in the base revision",
                    g.expected
                )),
            }
        } else {
            snapshot::safe_rel(&g.expected)
                .and_then(|rel| std::fs::read(work.join(rel)).map_err(|e| e.to_string()))
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
        let mut result = runner::run_check(spec, &work, &tmp, sandbox, writes)?;
        golden::judge(&work, g, &expected, &mut result);
        note_links_as_text(&mut result, &links_as_text);
        results.push(result);
    }
    results.extend(scan_results);
    progress.stage(Stage::Seal);

    // 5. Decide (uncovered changed files can only add a reason or turn VERIFIED into UNVERIFIED),
    // seal, save.
    let (verdict, mut reasons) = receipt::decide(checks_locked, &results, &protected_changes);
    if checks_external.as_ref().is_some_and(|e| !e.pinned) {
        // Same verdict (not locked), but the right reason.
        for r in reasons.iter_mut() {
            if r.starts_with("the checks are not in the base revision") {
                *r = "the --checks-from file is not pinned (no --checks-sha256), so the worker could have changed it".to_string();
            }
        }
    }
    let uncovered = if checks.uncovered_enabled() {
        let covers = Protected::labeled(&checks.covers_patterns(), "covers pattern")?;
        Some(uncovered::find(
            &changes,
            &covers,
            &protected,
            checks.uncovered,
        ))
    } else {
        None
    };
    let (verdict, reasons) = match &uncovered {
        Some(u) => uncovered::apply(verdict, reasons, u),
        None => (verdict, reasons),
    };
    let (verdict, reasons) = match &watched {
        Some(w) => watched::apply(verdict, reasons, w),
        None => (verdict, reasons),
    };
    // The agent's own hook settings, from the real working tree (git-ignored local files too),
    // against the session's start when the hook knows it (see `Options::settings_base`).
    let mut settings_rev = match &opts.settings_base {
        Some(rev) => {
            let c = git::resolve_commit(&root, rev)?;
            if c == base_commit {
                None
            } else {
                Some(git::Revision::load(&root, &c)?)
            }
        }
        None => None,
    };
    let settings_from = match settings_rev.as_mut() {
        Some(r) => r,
        None => &mut base,
    };
    let mut hook_settings = Vec::new();
    for path in hook_settings::FILES {
        let was = match settings_from.files().get(path) {
            None => hook_settings::File::Absent,
            Some(f) if f.symlink => {
                hook_settings::File::Symlink(settings_from.read(path)?.unwrap_or_default())
            }
            Some(_) => hook_settings::File::Regular(settings_from.read(path)?.unwrap_or_default()),
        };
        hook_settings.extend(hook_settings::compare(
            path,
            &was,
            &read_settings(&root, path),
        ));
    }
    let (verdict, reasons) = hook_settings::apply(verdict, reasons, &hook_settings);
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
        checks_external,
        sandbox: sandbox.label().to_string(),
        tree_sha256,
        linked,
        protected_changes,
        checks: results,
        uncovered,
        watched,
        hook_settings,
        verdict,
        reasons,
    }
    .seal()?;

    std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    ensure_receipts_ignored(&root, &dir);
    let path = dir.join(format!("{}.json", receipt.id));
    let text = serde_json::to_string_pretty(&receipt).map_err(|e| e.to_string())?;
    std::fs::write(&path, text).map_err(|e| format!("{}: {e}", path.display()))?;
    Ok((receipt, path))
}

/// One agent hook settings file as it is in the working tree: never followed through a symlink,
/// and a file too large to parse is passed on as its first bytes (it then counts as changed).
pub fn read_settings(root: &std::path::Path, rel: &str) -> hook_settings::File {
    let path = root.join(rel);
    match std::fs::symlink_metadata(&path) {
        Err(_) => hook_settings::File::Absent,
        Ok(m) if m.file_type().is_symlink() => hook_settings::File::Symlink(
            std::fs::read_link(&path)
                .map(|t| t.to_string_lossy().into_owned().into_bytes())
                .unwrap_or_default(),
        ),
        Ok(m) if !m.is_file() => hook_settings::File::Symlink(Vec::new()),
        Ok(_) => {
            use std::io::Read;
            let mut bytes = Vec::new();
            let cap = hook_settings::MAX_BYTES as u64 + 1;
            match std::fs::File::open(&path).and_then(|f| f.take(cap).read_to_end(&mut bytes)) {
                Ok(_) => hook_settings::File::Regular(bytes),
                // Unreadable: treat like a file Countercheck won't read.
                Err(_) => hook_settings::File::Symlink(Vec::new()),
            }
        }
    }
}

/// Which agent's shell `verify` runs in, when `--agent` isn't given: each agent marks the commands
/// it runs (Claude Code `CLAUDE_CODE_CHILD_SESSION` / `CLAUDECODE`, Cursor `CURSOR_AGENT`, Codex
/// `CODEX_SANDBOX*`, Hermes `HERMES_SESSION_ID`). A label like `--agent`, never a proof: the
/// worker controls its environment, and it changes no check and no verdict. An agent started by
/// another inherits both markers; the most specific one (the innermost spawner's) wins.
pub fn agent_from_env() -> Option<&'static str> {
    let set = |k: &str| std::env::var_os(k).is_some_and(|v| !v.is_empty());
    if set("CLAUDE_CODE_CHILD_SESSION") || set("CLAUDECODE") {
        Some("claude-code")
    } else if set("CURSOR_AGENT") {
        Some("cursor")
    } else if set("CODEX_SANDBOX") || set("CODEX_SANDBOX_NETWORK_DISABLED") {
        Some("codex")
    } else if set("HERMES_SESSION_ID") {
        Some("hermes")
    } else {
        None
    }
}

/// The agent session `verify` runs in, when its environment says (Hermes `HERMES_SESSION_ID`).
pub fn session_from_env() -> Option<String> {
    std::env::var("HERMES_SESSION_ID").ok().and_then(|s| {
        countercheck_core::receipt::clean_label(&s, countercheck_core::receipt::MAX_SESSION_CHARS)
    })
}

/// The clean copy against the base, by bytes. A file that differs only by CRLF line endings is
/// marked [`Diff::LineEndings`].
/// The sandbox this run gets. Windows: Git for Windows' `sh` can't start under the sandbox's
/// write-restricted token (MSYS secures its shared memory and signal pipes to the user account
/// only, and a token that may write there may write everywhere the user can), so in a run with
/// `sh` checks those run without it, each says so on the receipt, and the label says the run was
/// partial; asked for by name (`--sandbox windows`), such a run is an error.
fn windows_sh_check(
    opts: &Options,
    planned: &[config::PlannedCheck],
) -> Result<runner::Sandbox, String> {
    // The full sandbox runs every shell as the runner account (ADR-0004).
    if opts.sandbox != runner::Sandbox::RestrictedToken {
        return Ok(opts.sandbox);
    }
    let sh = planned
        .iter()
        .filter(|c| c.shell == config::Shell::Sh)
        .count();
    if sh == 0 {
        eprintln!(
            "note: on Windows checks can reach the internet (writes are still confined; the receipt \
             records this)."
        );
        return Ok(opts.sandbox);
    }
    if opts.sandbox_required {
        return Err(format!(
            "{sh} check(s) use sh, and Git for Windows' sh can't run in the Windows sandbox (MSYS \
             secures its shared memory to your user account only); set shell = \"pwsh\" or \"cmd\" \
             on them in .countercheck/checks.toml, or use --sandbox auto to run them unsandboxed \
             (the receipt says which)"
        ));
    }
    eprintln!(
        "note: {sh} sh check(s) run without the Windows sandbox (Git for Windows' sh can't run in it; \
         the receipt marks them). Checks with shell = \"pwsh\" or \"cmd\" are sandboxed; the \
         internet is not blocked on Windows."
    );
    Ok(runner::Sandbox::RestrictedTokenPartial)
}

fn compare(
    base: &mut git::Revision,
    copied: &Files,
    work: &std::path::Path,
) -> Result<Vec<(Diff, String)>, String> {
    let mut diff = gitobj::diff(base.files(), copied);
    let mut same = Vec::new();
    for (d, path) in diff.iter_mut() {
        let both_files = base.files().get(path.as_str()).is_some_and(|f| !f.symlink)
            && copied.get(path.as_str()).is_some_and(|f| !f.symlink);
        if *d != Diff::Modified || !both_files {
            continue;
        }
        let Some(mut was) = base.read(path)? else {
            continue;
        };
        let now = std::fs::read(work.join(snapshot::safe_rel(path)?))
            .map_err(|e| format!("{path}: {e}"))?;
        // Stored in Git LFS: compare with the real content (checked against the pointer). If it
        // isn't available or doesn't check out, the file counts as changed; a protected one then
        // fails with a plain error when it is restored.
        if gitobj::lfs_pointer(&was).is_some() {
            match base.read_real(path) {
                Ok(Some(real)) => was = real,
                _ => continue,
            }
            if was == now {
                same.push(path.clone());
                continue;
            }
        }
        if gitobj::eol_equal(&was, &now) {
            *d = Diff::LineEndings;
        }
    }
    diff.retain(|(_, p)| !same.contains(p));
    Ok(diff)
}

/// True when the clean copy differs from HEAD beyond line endings: uncommitted work, even when
/// git's index was told to look away.
fn head_differs(
    root: &std::path::Path,
    head: &str,
    base: &str,
    base_diff: &[(Diff, String)],
    copied: &Files,
    work: &std::path::Path,
) -> Result<bool, String> {
    let real = |diff: &[(Diff, String)]| diff.iter().any(|(d, _)| *d != Diff::LineEndings);
    if head == base {
        return Ok(real(base_diff));
    }
    let mut head = git::Revision::load(root, head)?;
    Ok(real(&compare(&mut head, copied, work)?))
}

/// Say on a check's output when symlinks in its clean copy are text files (Windows refused them).
fn note_links_as_text(result: &mut receipt::CheckResult, paths: &[String]) {
    if paths.is_empty() {
        return;
    }
    let mut shown = paths.iter().take(5).cloned().collect::<Vec<_>>().join(", ");
    if paths.len() > 5 {
        shown.push_str(", ...");
    }
    let note = format!(
        "[countercheck] Windows refused to create symlinks, so {} symlink(s) in the clean copy are text \
         files holding the link target, like git's core.symlinks=false (Developer Mode allows symlinks): {shown}",
        paths.len()
    );
    if result.output_tail.is_empty() {
        result.output_tail = note;
    } else {
        result.output_tail = format!("{}\n{note}", result.output_tail);
    }
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
