//! Thin, safe wrappers around the `git` command line.

use std::path::{Path, PathBuf};
use std::process::Command;

fn run(root: &Path, args: &[&str]) -> Result<Vec<u8>, String> {
    let out = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .output()
        .map_err(|e| format!("could not run git: {e}"))?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        return Err(format!("git {}: {}", args.join(" "), err.trim()));
    }
    Ok(out.stdout)
}

fn run_text(root: &Path, args: &[&str]) -> Result<String, String> {
    Ok(String::from_utf8_lossy(&run(root, args)?)
        .trim()
        .to_string())
}

pub fn toplevel(dir: &Path) -> Result<PathBuf, String> {
    run_text(dir, &["rev-parse", "--show-toplevel"])
        .map(PathBuf::from)
        .map_err(|_| format!("{} is not inside a git repository", dir.display()))
}

pub fn resolve_commit(root: &Path, rev: &str) -> Result<String, String> {
    if rev.starts_with('-') {
        return Err(format!("invalid revision \"{rev}\""));
    }
    run_text(
        root,
        &[
            "rev-parse",
            "--verify",
            "--quiet",
            &format!("{rev}^{{commit}}"),
        ],
    )
    .map_err(|_| format!("revision \"{rev}\" not found"))
}

pub fn is_dirty(root: &Path) -> Result<bool, String> {
    Ok(!run(root, &["status", "--porcelain", "-z"])?.is_empty())
}

/// File contents at a commit, or None when the file doesn't exist there.
pub fn show(root: &Path, commit: &str, path: &str) -> Result<Option<Vec<u8>>, String> {
    let spec = format!("{commit}:{path}");
    let exists = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["cat-file", "-e", &spec])
        .output()
        .map_err(|e| format!("could not run git: {e}"))?
        .status
        .success();
    if !exists {
        return Ok(None);
    }
    run(root, &["cat-file", "blob", &spec]).map(Some)
}

fn split_z(bytes: &[u8]) -> Vec<String> {
    bytes
        .split(|b| *b == 0)
        .filter(|s| !s.is_empty())
        .map(|s| String::from_utf8_lossy(s).into_owned())
        .collect()
}

/// Every file the worker's tree has: tracked plus untracked-but-not-ignored.
pub fn worktree_files(root: &Path) -> Result<Vec<String>, String> {
    let mut files = split_z(&run(
        root,
        &[
            "ls-files",
            "-z",
            "--cached",
            "--others",
            "--exclude-standard",
        ],
    )?);
    files.sort();
    files.dedup();
    Ok(files)
}

/// Changes between the base commit and the working tree (committed + uncommitted + untracked).
pub fn changes_since(
    root: &Path,
    base: &str,
) -> Result<Vec<(omni_core::protect::Change, String)>, String> {
    use omni_core::protect::Change;
    let fields = split_z(&run(
        root,
        &["diff", "--no-renames", "--name-status", "-z", base],
    )?);
    let mut out = Vec::new();
    let mut it = fields.into_iter();
    while let (Some(status), Some(path)) = (it.next(), it.next()) {
        let change = match status.chars().next() {
            Some('A') => Change::Added,
            Some('D') => Change::Deleted,
            _ => Change::Modified,
        };
        out.push((change, path));
    }
    for path in split_z(&run(
        root,
        &["ls-files", "-z", "--others", "--exclude-standard"],
    )?) {
        out.push((Change::Added, path));
    }
    Ok(out)
}
