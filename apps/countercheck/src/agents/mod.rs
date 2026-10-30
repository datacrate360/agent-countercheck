//! The agents Countercheck can hook, and `countercheck hook install / uninstall / status`.
//!
//! Each agent is one [`Target`]: where it keeps its hooks, what to add there, and the step the user
//! must take afterwards (approve the hook, reload). To add an agent: a module next to this one with
//! a `target()` (and its stop hook, if its input or answer differs from Claude Code's), one line in
//! [`all`], one `HookCmd` variant in `main.rs` if it has its own stop hook, a docs page in
//! `docs/hooks/`, and a test file `tests/hook_<agent>.rs`. Nothing here decides a verdict: every
//! stop hook calls the shared `hook::check`.

pub mod json;

mod claude;
pub mod copilot;

use json::Node;
use std::io::IsTerminal;
use std::path::{Path, PathBuf};

/// One thing to put in an agent's hooks file: an entry appended to the array at `path` (keys from
/// the top), unless an entry there already mentions one of `needles` (the Countercheck command
/// and its aliases).
pub struct Entry {
    pub path: Vec<&'static str>,
    /// The entry as JSON text, keys in the order the agent's docs show them.
    pub value: String,
    pub needles: Vec<&'static str>,
}

impl Entry {
    fn in_node(&self, n: &Node) -> bool {
        self.needles.iter().any(|needle| n.mentions(needle))
    }
}

pub struct Target {
    /// The name on the command line and on receipts (`claude-code`).
    pub agent: &'static str,
    pub label: &'static str,
    /// Where the hooks go for one project, relative to the project root.
    pub project_file: &'static str,
    /// Where they go for every project, relative to the home folder (`None`: the agent has none).
    pub user_file: Option<&'static str>,
    /// Top-level keys a new file needs (`"version": 1`), set only when missing.
    pub top: Vec<(&'static str, serde_json::Value)>,
    /// What to add, given the command that runs Countercheck (`countercheck`, or `--bin`).
    pub entries: fn(bin: &str) -> Vec<Entry>,
    /// Other files this agent also reads hooks from: a Countercheck hook there too runs twice.
    pub also_reads: &'static [&'static str],
    /// What the user must do after install (approve the hook, restart the session).
    pub after: &'static str,
    /// The docs page, relative to the repository.
    pub docs: &'static str,
}

/// Every agent `hook install` knows. Keep the order: it is the order `hook status` prints.
pub fn all() -> Vec<Target> {
    vec![
        claude::claude_code(),
        claude::cortex_code(),
        copilot::target(),
    ]
}

/// Agents with hooks Countercheck can't write for you (no project file, or not JSON).
pub const MANUAL: &[(&str, &str)] = &[(
    "hermes",
    "hooks live in ~/.hermes/config.yaml: see docs/hooks/hermes.md",
)];

/// The command a hook runs: `<bin> hook <sub>`, with `bin` in double quotes when it has spaces
/// (`C:/Program Files/...`), as a JSON string.
pub fn command(bin: &str, sub: &str) -> String {
    let bin = if bin.contains(char::is_whitespace) && !bin.starts_with('"') {
        format!("\"{bin}\"")
    } else {
        bin.to_string()
    };
    quoted(&format!("{bin} hook {sub}"))
}

/// `s` as a JSON string (for building entries).
pub fn quoted(s: &str) -> String {
    serde_json::to_string(s).unwrap_or_else(|_| "\"\"".to_string())
}

fn find(agent: &str) -> Result<Target, String> {
    if let Some((_, how)) = MANUAL.iter().find(|(a, _)| *a == agent) {
        return Err(format!("{agent}: {how}"));
    }
    all().into_iter().find(|t| t.agent == agent).ok_or_else(|| {
        let names: Vec<&str> = all().iter().map(|t| t.agent).collect();
        format!("unknown agent \"{agent}\"; known: {}", names.join(", "))
    })
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    Project,
    User,
    /// Claude Code's managed settings (an admin-owned system folder): hooks there can't be switched
    /// off from user, project or local settings, so an agent can't undo them.
    Managed,
}

/// Claude Code's managed-settings drop-in folder for this OS (`COUNTERCHECK_MANAGED_DIR` replaces
/// the system folder, for tests and unusual installs). Countercheck writes its own file there, so it
/// never edits an organization's `managed-settings.json`.
fn managed_file() -> PathBuf {
    let dir = match std::env::var_os("COUNTERCHECK_MANAGED_DIR") {
        Some(d) => PathBuf::from(d),
        None if cfg!(windows) => PathBuf::from(r"C:\Program Files\ClaudeCode"),
        None if cfg!(target_os = "macos") => {
            PathBuf::from("/Library/Application Support/ClaudeCode")
        }
        None => PathBuf::from("/etc/claude-code"),
    };
    dir.join("managed-settings.d").join("50-countercheck.json")
}

/// The full path of a program on PATH (a stable link like /opt/homebrew/bin/countercheck), for
/// managed hooks, which run for every user whatever their PATH.
fn which(name: &str) -> Option<PathBuf> {
    if Path::new(name).components().count() > 1 {
        return None;
    }
    let exts: &[&str] = if cfg!(windows) { &["", ".exe"] } else { &[""] };
    let paths = std::env::var_os("PATH")?;
    std::env::split_paths(&paths)
        .flat_map(|d| exts.iter().map(move |e| d.join(format!("{name}{e}"))))
        .find(|p| p.is_file())
}

pub struct Options {
    pub agent: String,
    pub scope: Scope,
    pub repo: PathBuf,
    /// Write without asking.
    pub yes: bool,
    /// Show the change, write nothing.
    pub dry_run: bool,
    /// The command that runs Countercheck in the hook.
    pub bin: String,
}

fn home() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
}

/// The file a target's hooks go in, and the project root (for project scope).
fn file_for(t: &Target, scope: Scope, repo: &Path) -> Result<(PathBuf, Option<PathBuf>), String> {
    match scope {
        Scope::Project => {
            let root = crate::git::toplevel(repo)
                .map_err(|_| format!("{} is not inside a git repository", repo.display()))?;
            Ok((root.join(t.project_file), Some(root)))
        }
        Scope::User => {
            let rel = t
                .user_file
                .ok_or_else(|| format!("{} has no user-wide hooks file", t.label))?;
            let home = home().ok_or("no home folder (HOME is not set)")?;
            Ok((home.join(rel), None))
        }
        Scope::Managed if t.agent == "claude-code" => Ok((managed_file(), None)),
        Scope::Managed => Err(format!(
            "--scope managed is Claude Code's managed settings; for {} use --scope user or project",
            t.label
        )),
    }
}

/// The file's text, or empty when it doesn't exist. A symlink or something too large is refused:
/// the write would land somewhere else, or it isn't a settings file.
fn read_settings(path: &Path) -> Result<Option<String>, String> {
    match std::fs::symlink_metadata(path) {
        Err(_) => Ok(None),
        Ok(m) if m.file_type().is_symlink() => Err(format!(
            "{} is a symlink; edit it by hand (see the docs page)",
            path.display()
        )),
        Ok(m) if !m.is_file() => Err(format!("{} is not a file", path.display())),
        Ok(m) if m.len() as usize > json::MAX_BYTES => Err(format!(
            "{} is larger than {} KiB",
            path.display(),
            json::MAX_BYTES / 1024
        )),
        Ok(_) => std::fs::read_to_string(path)
            .map(Some)
            .map_err(|e| format!("{}: {e}", path.display())),
    }
}

fn parse_settings(path: &Path, text: Option<&str>) -> Result<Node, String> {
    match text {
        None => Ok(Node::Obj(Vec::new())),
        Some(t) if t.trim().is_empty() => Ok(Node::Obj(Vec::new())),
        Some(t) => match json::parse(t) {
            Ok(n) if n.is_object() => Ok(n),
            Ok(_) => Err(format!("{}: not a JSON object", path.display())),
            Err(e) => Err(format!("{}: {e}", path.display())),
        },
    }
}

/// The array at `path` (created when missing).
fn array_at<'a>(root: &'a mut Node, path: &[&str]) -> Result<&'a mut Vec<Node>, String> {
    let mut node = root;
    for (i, key) in path.iter().enumerate() {
        let last = i + 1 == path.len();
        node = node.entry(key, || {
            if last {
                Node::Arr(Vec::new())
            } else {
                Node::Obj(Vec::new())
            }
        })?;
    }
    node.items_mut()
        .ok_or_else(|| format!("\"{}\" is not a list", path.join(".")))
}

/// Add the target's entries; the number added.
fn add(t: &Target, node: &mut Node, bin: &str) -> Result<usize, String> {
    for (k, v) in &t.top {
        node.entry(k, || json::from_value(v))?;
    }
    let mut added = 0;
    for e in (t.entries)(bin) {
        let arr = array_at(node, &e.path)?;
        if !arr.iter().any(|n| e.in_node(n)) {
            arr.push(json::parse(&e.value).map_err(|err| format!("internal: {err}"))?);
            added += 1;
        }
    }
    Ok(added)
}

/// Take out every entry that mentions a Countercheck command, and the lists and objects that leaves
/// empty along each entry's path; the number removed.
fn remove(t: &Target, node: &mut Node, bin: &str) -> usize {
    fn walk(node: &mut Node, path: &[&str], e: &Entry) -> usize {
        let Some((key, rest)) = path.split_first() else {
            return 0;
        };
        let Some(child) = node.get_mut(key) else {
            return 0;
        };
        let n = if rest.is_empty() {
            match child.items_mut() {
                Some(a) => {
                    let before = a.len();
                    a.retain(|n| !e.in_node(n));
                    before - a.len()
                }
                None => 0,
            }
        } else {
            walk(child, rest, e)
        };
        if n > 0 && child.is_empty_container() {
            node.remove(key);
        }
        n
    }
    (t.entries)(bin)
        .iter()
        .map(|e| walk(node, &e.path, e))
        .sum()
}

/// The change as `-`/`+` lines with up to 3 unchanged lines around each change.
fn diff(path: &Path, old: &str, new: &str) -> String {
    let a: Vec<&str> = old.lines().collect();
    let b: Vec<&str> = new.lines().collect();
    let ops = similar::capture_diff_slices(similar::Algorithm::Myers, &a, &b);
    let mut lines: Vec<(char, &str)> = Vec::new();
    for op in ops {
        match op {
            similar::DiffOp::Equal { old_index, len, .. } => {
                lines.extend(a[old_index..old_index + len].iter().map(|l| (' ', *l)))
            }
            similar::DiffOp::Delete {
                old_index, old_len, ..
            } => lines.extend(a[old_index..old_index + old_len].iter().map(|l| ('-', *l))),
            similar::DiffOp::Insert {
                new_index, new_len, ..
            } => lines.extend(b[new_index..new_index + new_len].iter().map(|l| ('+', *l))),
            similar::DiffOp::Replace {
                old_index,
                old_len,
                new_index,
                new_len,
            } => {
                lines.extend(a[old_index..old_index + old_len].iter().map(|l| ('-', *l)));
                lines.extend(b[new_index..new_index + new_len].iter().map(|l| ('+', *l)));
            }
        }
    }
    let near = |i: usize| {
        let lo = i.saturating_sub(3);
        let hi = (i + 4).min(lines.len());
        lines[lo..hi].iter().any(|(m, _)| *m != ' ')
    };
    let mut out = format!("--- {0}\n+++ {0}\n", path.display());
    let mut skipped = false;
    for (i, (mark, text)) in lines.iter().enumerate() {
        if *mark == ' ' && !near(i) {
            if !skipped {
                out.push_str("@@ ... @@\n");
                skipped = true;
            }
            continue;
        }
        skipped = false;
        out.push_str(&format!("{mark}{text}\n"));
    }
    out
}

/// Ask on a terminal; refuse (nothing written) when nobody can answer.
fn consent(yes: bool) -> Result<bool, String> {
    if yes {
        return Ok(true);
    }
    if !std::io::stdin().is_terminal() {
        return Err("nothing written: re-run with --yes to write this change".to_string());
    }
    eprint!("Write this change? [y/N] ");
    let mut answer = String::new();
    std::io::stdin()
        .read_line(&mut answer)
        .map_err(|e| e.to_string())?;
    Ok(matches!(answer.trim(), "y" | "Y" | "yes"))
}

/// Write through a temporary file in the same folder, so a crash never leaves half a file. The
/// first time, the original is kept next to it as `<file>.countercheck-backup`.
fn write_settings(path: &Path, old: Option<&str>, new: &str) -> Result<Option<PathBuf>, String> {
    let dir = path.parent().ok_or("no parent folder")?;
    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let mut backup = None;
    if let Some(old) = old {
        let mut name = path.file_name().unwrap_or_default().to_os_string();
        name.push(".countercheck-backup");
        let b = path.with_file_name(name);
        if std::fs::symlink_metadata(&b).is_err() {
            std::fs::write(&b, old).map_err(|e| format!("{}: {e}", b.display()))?;
            backup = Some(b);
        }
    }
    let tmp = tempfile::NamedTempFile::new_in(dir).map_err(|e| e.to_string())?;
    std::fs::write(tmp.path(), new).map_err(|e| e.to_string())?;
    tmp.persist(path)
        .map_err(|e| format!("{}: {}", path.display(), e.error))?;
    Ok(backup)
}

/// True when `name` runs: a path that exists, or a program on PATH.
fn runs(name: &str) -> bool {
    let p = Path::new(name);
    if p.components().count() > 1 {
        return p.is_file();
    }
    let exts: Vec<String> = if cfg!(windows) {
        vec![String::new(), ".exe".into(), ".cmd".into(), ".bat".into()]
    } else {
        vec![String::new()]
    };
    std::env::var_os("PATH").is_some_and(|paths| {
        std::env::split_paths(&paths)
            .any(|d| exts.iter().any(|e| d.join(format!("{name}{e}")).is_file()))
    })
}

fn committed(root: &Path, path: &Path) -> bool {
    path.strip_prefix(root).ok().is_some_and(|rel| {
        crate::git::is_committed(root, &rel.to_string_lossy().replace('\\', "/"))
    })
}

pub fn install(o: &Options) -> Result<(), String> {
    let t = find(&o.agent)?;
    let (path, root) = file_for(&t, o.scope, &o.repo)?;
    let old = read_settings(&path)?;
    let mut node = parse_settings(&path, old.as_deref())?;
    // Managed hooks run for every user of the machine: name countercheck by its full path.
    let bin = match o.scope {
        Scope::Managed => which(&o.bin)
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|| o.bin.clone()),
        _ => o.bin.clone(),
    };
    let added = add(&t, &mut node, &bin)?;
    let new = json::pretty(&node);
    if added == 0 {
        println!(
            "{}: the Countercheck hooks are already in {}",
            t.label,
            path.display()
        );
        return Ok(());
    }
    print!("{}", diff(&path, old.as_deref().unwrap_or(""), &new));
    if !runs(&bin) {
        eprintln!(
            "warning: `{bin}` is not on PATH here, so the hook would not run: pass --bin <full path to countercheck>"
        );
    }
    if let Some(root) = &root {
        for other in t.also_reads {
            if let Ok(Some(text)) = read_settings(&root.join(other))
                && text.contains("countercheck hook")
            {
                eprintln!(
                    "warning: {} also reads {other}, which has Countercheck hooks too: each stop would be checked twice; keep one",
                    t.label
                );
            }
        }
    }
    if o.dry_run {
        println!("(dry run: nothing written)");
        return Ok(());
    }
    if !consent(o.yes)? {
        println!("nothing written");
        return Ok(());
    }
    let backup = write_settings(&path, old.as_deref(), &new).map_err(|e| admin_hint(o, e))?;
    println!("wrote {}", path.display());
    if let Some(b) = backup {
        println!("the previous version is in {}", b.display());
    }
    if let Some(root) = &root {
        if std::fs::symlink_metadata(root.join(countercheck_core::config::CHECKS_PATH)).is_err() {
            println!("next: countercheck init, edit the checks, and commit them");
        }
        let rel = path
            .strip_prefix(root)
            .unwrap_or(&path)
            .display()
            .to_string();
        let rel = rel.replace('\\', "/");
        if countercheck_core::hook_settings::FILES.contains(&rel.as_str()) {
            println!(
                "commit it, so Countercheck can tell if an agent removes the hook: git add {rel} && git commit -m \"Countercheck hooks\""
            );
        } else {
            // Say so rather than promise a tamper check this file doesn't get.
            println!(
                "commit it: git add {rel} && git commit -m \"Countercheck hooks\" (Countercheck does not yet check this file for an agent removing the hook)"
            );
        }
    }
    if o.scope == Scope::Managed {
        println!(
            "Managed hooks can't be switched off from user, project or local settings. If your organization already \
             delivers managed settings by MDM or the claude.ai console, Claude Code may use that source instead of \
             this file: ask your administrator to add the hook there. Cloud sessions don't read this file."
        );
    }
    println!("{}", t.after);
    println!(
        "more: https://github.com/datacrate360/agent-countercheck/blob/PROD/{}",
        t.docs
    );
    Ok(())
}

/// A write refused in the managed folder needs administrator rights: say how.
fn admin_hint(o: &Options, e: String) -> String {
    if o.scope != Scope::Managed {
        return e;
    }
    let how = if cfg!(windows) {
        "run it again from a terminal opened as administrator".to_string()
    } else {
        format!(
            "run it again with sudo: sudo {} hook install --agent {} --scope managed",
            std::env::current_exe()
                .map(|p| p.display().to_string())
                .unwrap_or_else(|_| "countercheck".to_string()),
            o.agent
        )
    };
    format!("{e}\nThe managed settings folder needs administrator rights: {how}")
}

pub fn uninstall(o: &Options) -> Result<(), String> {
    let t = find(&o.agent)?;
    let (path, root) = file_for(&t, o.scope, &o.repo)?;
    let Some(old) = read_settings(&path)? else {
        println!(
            "{}: {} doesn't exist; nothing to remove",
            t.label,
            path.display()
        );
        return Ok(());
    };
    let mut node = parse_settings(&path, Some(&old))?;
    if remove(&t, &mut node, &o.bin) == 0 {
        println!("{}: no Countercheck hooks in {}", t.label, path.display());
        return Ok(());
    }
    let new = json::pretty(&node);
    print!("{}", diff(&path, &old, &new));
    if o.dry_run {
        println!("(dry run: nothing written)");
        return Ok(());
    }
    if !consent(o.yes)? {
        println!("nothing written");
        return Ok(());
    }
    write_settings(&path, Some(&old), &new).map_err(|e| admin_hint(o, e))?;
    println!("wrote {}", path.display());
    if let Some(root) = &root {
        let rel = path.strip_prefix(root).unwrap_or(&path);
        if countercheck_core::hook_settings::FILES
            .contains(&rel.to_string_lossy().replace('\\', "/").as_str())
        {
            println!(
                "commit it too: until then Countercheck reports the removed hook (an agent could have done it)"
            );
        } else {
            println!("commit it too");
        }
    }
    Ok(())
}

/// One line per agent and scope: installed or not, committed or not.
pub fn status(repo: &Path, bin: &str) -> Result<(), String> {
    let root = crate::git::toplevel(repo).ok();
    for t in all() {
        for scope in [Scope::Project, Scope::User, Scope::Managed] {
            let Ok((path, _)) = file_for(&t, scope, repo) else {
                continue;
            };
            let state = match read_settings(&path) {
                Err(e) => format!("can't read: {e}"),
                Ok(None) => "no hooks file".to_string(),
                Ok(Some(text)) => match parse_settings(&path, Some(&text)) {
                    Err(e) => e,
                    Ok(node) => {
                        let entries = (t.entries)(bin);
                        let have = entries.iter().filter(|e| e.in_node(&node)).count();
                        let mut s = match have {
                            0 => "not installed".to_string(),
                            n if n == entries.len() => "installed".to_string(),
                            n => format!("partly installed ({n} of {})", entries.len()),
                        };
                        if have > 0
                            && scope == Scope::Project
                            && let Some(root) = &root
                            && !committed(root, &path)
                        {
                            s.push_str(", not committed (commit it)");
                        }
                        s
                    }
                },
            };
            let where_ = match scope {
                Scope::Project => "project",
                Scope::User => "user",
                Scope::Managed => "managed",
            };
            println!(
                "{:<12} {:<8} {}  {}",
                t.agent,
                where_,
                state,
                path.display()
            );
        }
    }
    for (agent, how) in MANUAL {
        println!("{agent:<12} {:<8} {how}", "-");
    }
    if !runs(bin) {
        println!("warning: `{bin}` is not on PATH here; hooks that call it would not run");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_then_remove_leaves_the_rest_as_it_was() {
        let t = claude::claude_code();
        let text = "{\n  \"model\": \"opus\",\n  \"hooks\": {\n    \"Stop\": [\n      {\n        \"hooks\": [\n          {\n            \"type\": \"command\",\n            \"command\": \"say done\"\n          }\n        ]\n      }\n    ]\n  }\n}\n";
        let mut n = json::parse(text).unwrap();
        assert_eq!(add(&t, &mut n, "countercheck").unwrap(), 2);
        assert_eq!(
            add(&t, &mut n, "countercheck").unwrap(),
            0,
            "twice adds nothing"
        );
        let added = json::pretty(&n);
        assert!(added.starts_with("{\n  \"model\": \"opus\""), "{added}");
        assert!(added.contains("say done") && added.contains("countercheck hook claude-stop"));
        assert_eq!(remove(&t, &mut n, "countercheck"), 2);
        assert_eq!(json::pretty(&n), text);
    }

    #[test]
    fn a_hooks_value_that_isnt_an_object_is_refused() {
        let t = claude::claude_code();
        let mut n = json::parse("{\"hooks\": []}").unwrap();
        assert!(add(&t, &mut n, "countercheck").is_err());
    }
}
