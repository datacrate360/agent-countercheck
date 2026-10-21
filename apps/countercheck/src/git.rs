//! Thin, safe wrappers around the `git` command line.

use countercheck_core::gitobj::{self, Files, Format, Kind};
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

/// `git -C root`, without the repository's replace refs: `git replace` would let a worker swap
/// what a commit id shows while the id stays the same.
fn git(root: &Path) -> Command {
    let mut c = Command::new("git");
    c.arg("--no-replace-objects").arg("-C").arg(root);
    c
}

fn run(root: &Path, args: &[&str]) -> Result<Vec<u8>, String> {
    let out = git(root)
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

/// This work tree's own git directory (absolute; per worktree, so each has its own HEAD).
pub fn git_dir(root: &Path) -> Result<PathBuf, String> {
    run_text(root, &["rev-parse", "--absolute-git-dir"]).map(PathBuf::from)
}

pub fn is_dirty(root: &Path) -> Result<bool, String> {
    Ok(!run(root, &["status", "--porcelain", "-z"])?.is_empty())
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

/// Most files and folders read from one revision; a bigger tree is refused, not truncated.
const MAX_ENTRIES: usize = 1_000_000;
/// Largest single object read from git.
const MAX_OBJECT_BYTES: u64 = 512 * 1024 * 1024;

/// A revision's files, read from git's object store and checked against their ids: every object
/// on the way from the commit id to a file's bytes is hashed again, so a rewritten object file or
/// a replace ref is refused instead of believed. The index, git filters and the worker's config
/// play no part.
pub struct Revision {
    format: Format,
    files: Files,
    cat: Cat,
}

impl Revision {
    /// Read the tree of `commit` (a full commit id, e.g. from [`resolve_commit`]).
    pub fn load(root: &Path, commit: &str) -> Result<Self, String> {
        // SHA-1 ids have 40 hex characters, SHA-256 ids 64 (no `--show-object-format` before git 2.29).
        let format = [Format::Sha1, Format::Sha256]
            .into_iter()
            .find(|f| gitobj::is_full_id(*f, commit))
            .ok_or_else(|| format!("\"{commit}\" is not a full commit id"))?;
        let mut cat = Cat::start(root, format)?;
        let body = cat.read(commit, "commit")?;
        let mut files = Files::new();
        let mut todo = vec![(String::new(), gitobj::commit_tree(format, &body)?)];
        let mut seen = 0usize;
        while let Some((prefix, tree)) = todo.pop() {
            for e in gitobj::parse_tree(format, &cat.read(&tree, "tree")?)? {
                seen += 1;
                if seen > MAX_ENTRIES {
                    return Err(format!(
                        "the revision has more than {MAX_ENTRIES} files and folders"
                    ));
                }
                let path = format!("{prefix}{}", e.name);
                match e.kind {
                    Kind::Tree => todo.push((format!("{path}/"), e.id)),
                    Kind::File | Kind::Symlink => {
                        files.insert(
                            path,
                            gitobj::FileId {
                                symlink: e.kind == Kind::Symlink,
                                id: e.id,
                            },
                        );
                    }
                    Kind::Submodule => {} // another repository's files, never in the copy
                }
            }
        }
        Ok(Self { format, files, cat })
    }

    pub fn format(&self) -> Format {
        self.format
    }

    /// Every file (and symlink) in the revision, by `/`-separated path.
    pub fn files(&self) -> &Files {
        &self.files
    }

    /// A file's bytes (a symlink's target), or None when the revision has no such file.
    pub fn read(&mut self, path: &str) -> Result<Option<Vec<u8>>, String> {
        let Some(f) = self.files.get(path) else {
            return Ok(None);
        };
        let id = f.id.clone();
        self.cat.read(&id, "blob").map(Some)
    }
}

/// One `git cat-file --batch` process; every object it returns is checked against its id.
struct Cat {
    format: Format,
    child: Child,
    input: ChildStdin,
    output: BufReader<ChildStdout>,
}

impl Cat {
    fn start(root: &Path, format: Format) -> Result<Self, String> {
        let mut child = git(root)
            .args(["cat-file", "--batch"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| format!("could not run git: {e}"))?;
        let (Some(input), Some(output)) = (child.stdin.take(), child.stdout.take()) else {
            let _ = child.kill();
            let _ = child.wait();
            return Err("could not talk to git cat-file".to_string());
        };
        Ok(Self {
            format,
            child,
            input,
            output: BufReader::new(output),
        })
    }

    fn read(&mut self, id: &str, kind: &str) -> Result<Vec<u8>, String> {
        let broken = || format!("git could not read object {id} of the base revision");
        writeln!(self.input, "{id}").map_err(|_| broken())?;
        self.input.flush().map_err(|_| broken())?;
        let mut header = String::new();
        self.output.read_line(&mut header).map_err(|_| broken())?;
        let mut parts = header.split_whitespace();
        let (Some(got_id), Some(got_kind), Some(size)) = (parts.next(), parts.next(), parts.next())
        else {
            return Err(broken());
        };
        let size: u64 = size.parse().map_err(|_| broken())?;
        if got_id != id || size > MAX_OBJECT_BYTES {
            return Err(broken());
        }
        let mut body = Vec::new();
        (&mut self.output)
            .take(size + 1)
            .read_to_end(&mut body)
            .map_err(|_| broken())?;
        if body.pop() != Some(b'\n') || body.len() as u64 != size {
            return Err(broken());
        }
        if got_kind != kind || gitobj::object_id(self.format, kind, &body) != id {
            return Err(format!(
                "git object {id} of the base revision does not match its id: the repository's \
                 .git folder was changed, so countercheck can't trust what the base holds"
            ));
        }
        Ok(body)
    }
}

impl Drop for Cat {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
