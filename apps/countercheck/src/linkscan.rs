//! Look inside the folders named in `link` (they come from the worker's tree): files and symlinks
//! that point back into the project (`countercheck_core::linked`), and a fingerprint to tell
//! whether a folder changed while a session ran. The folders are hostile: symlinks inside them are
//! never followed, files are read only when small, and every walk is capped.

use countercheck_core::linked::{self, Pointer};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Component, Path, PathBuf};

/// How deep the pointer scan looks: an editable install's files sit at
/// `lib/python3.x/site-packages/<file>` (Windows: `Lib/site-packages/<file>`), a workspace
/// package's symlink at `node_modules/<name>` or `node_modules/@scope/<name>`.
const SCAN_DEPTH: usize = 4;
/// Entries one scan or fingerprint looks at before it gives up.
const MAX_ENTRIES: usize = 500_000;
/// Files larger than this are not read for pointers (`.pth` and finder files are tiny).
const MAX_FILE_BYTES: u64 = 64 * 1024;
/// Compare paths without case where the file system usually ignores it (Windows, and macOS's
/// default APFS), so `~/code/proj` typed in lower case still names `~/Code/proj`.
const FOLD_CASE: bool = cfg!(any(windows, target_os = "macos"));
/// Bytes one pointer scan reads in total.
const MAX_READ_BYTES: u64 = 64 * 1024 * 1024;

/// The project's absolute path in every form a tool may have written it: as git gives it, its
/// canonical form, and (macOS) without the `/private` that `/var` and `/tmp` resolve to.
fn roots(root: &Path) -> Vec<String> {
    let mut out = vec![root.to_string_lossy().to_string()];
    if let Ok(c) = crate::snapshot::canonical(root) {
        out.push(c.to_string_lossy().to_string());
    }
    let extra: Vec<String> = out
        .iter()
        .filter_map(|r| r.strip_prefix("/private").map(str::to_string))
        .filter(|r| r.starts_with("/var/") || r.starts_with("/tmp/"))
        .collect();
    out.extend(extra);
    out.sort();
    out.dedup();
    out
}

/// `a/./b/../c` -> `a/c`, without touching the disk (a symlink's target may not exist).
fn lexical(p: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for c in p.components() {
        match c {
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            other => out.push(other.as_os_str()),
        }
    }
    out
}

struct Scan<'a> {
    roots: Vec<String>,
    linked: &'a [String],
    entries: usize,
    read: u64,
    found: Vec<Pointer>,
    incomplete: bool,
}

impl Scan<'_> {
    fn walk(&mut self, real: &Path, shown: &str, depth: usize) {
        let Ok(rd) = fs::read_dir(real) else {
            return;
        };
        let mut items: Vec<_> = rd.flatten().collect();
        items.sort_by_key(|e| e.file_name());
        for e in items {
            self.entries += 1;
            if self.entries > MAX_ENTRIES || self.read > MAX_READ_BYTES {
                self.incomplete = true;
                return;
            }
            let name = e.file_name().to_string_lossy().to_string();
            let file = format!("{shown}/{name}");
            let Ok(meta) = fs::symlink_metadata(e.path()) else {
                continue;
            };
            let ft = meta.file_type();
            if ft.is_symlink() {
                if let Ok(target) = fs::read_link(e.path()) {
                    let abs = lexical(&real.join(target));
                    if let Some(rel) =
                        linked::project_rel(&abs.to_string_lossy(), &self.roots, FOLD_CASE)
                        && linked::outside_linked(&rel, self.linked)
                    {
                        self.found.push(Pointer { file, target: rel });
                    }
                }
            } else if ft.is_dir() {
                if depth < SCAN_DEPTH {
                    self.walk(&e.path(), &file, depth + 1);
                }
            } else if ft.is_file() && meta.len() <= MAX_FILE_BYTES {
                self.read += meta.len();
                let Ok(bytes) = fs::read(e.path()) else {
                    continue;
                };
                let text = String::from_utf8_lossy(&bytes);
                for target in linked::pointers_in(&text, &self.roots, self.linked, FOLD_CASE) {
                    self.found.push(Pointer {
                        file: file.clone(),
                        target,
                    });
                }
            }
        }
    }
}

/// Every file or symlink (up to `SCAN_DEPTH` levels into each linked folder) that names a project
/// path outside the linked folders, and how many there are in all. A folder too large to scan
/// counts as one pointer: what wasn't looked at can't be vouched for.
pub fn pointers(root: &Path, linked: &[String]) -> (Vec<Pointer>, usize) {
    let mut scan = Scan {
        roots: roots(root),
        linked,
        entries: 0,
        read: 0,
        found: Vec::new(),
        incomplete: false,
    };
    for name in linked {
        let Ok(rel) = crate::snapshot::safe_rel(name) else {
            continue;
        };
        // The linked folder itself may be a symlink (a shared venv): follow that one link only.
        let Ok(real) = crate::snapshot::canonical(&root.join(rel)) else {
            continue;
        };
        if !real.is_dir() {
            continue;
        }
        scan.incomplete = false;
        scan.walk(&real, name.trim_end_matches('/'), 1);
        if scan.incomplete {
            scan.found.push(Pointer {
                file: name.trim_end_matches('/').to_string(),
                target: format!(
                    "(not fully scanned: more than {MAX_ENTRIES} entries or {} MiB)",
                    MAX_READ_BYTES / 1024 / 1024
                ),
            });
        }
    }
    scan.found.sort();
    scan.found.dedup();
    let total = scan.found.len();
    scan.found.truncate(linked::MAX_LISTED);
    (scan.found, total)
}

/// A fingerprint of a linked folder: every path, type, size, modification time and symlink target,
/// hashed. `None` when the folder isn't there. Python's `__pycache__` folders are left out: running
/// the project writes them, and the checks never read them (`PYTHONPYCACHEPREFIX`, see runner).
pub fn fingerprint(root: &Path, name: &str) -> Option<String> {
    let rel = crate::snapshot::safe_rel(name).ok()?;
    let real = crate::snapshot::canonical(&root.join(rel)).ok()?;
    if !real.is_dir() {
        return None;
    }
    let mut h = Sha256::new();
    let mut n = 0usize;
    if !walk_fingerprint(&real, "", &mut h, &mut n) {
        return Some("too-large".to_string());
    }
    Some(format!("{:x}", h.finalize()))
}

fn walk_fingerprint(dir: &Path, shown: &str, h: &mut Sha256, n: &mut usize) -> bool {
    let Ok(rd) = fs::read_dir(dir) else {
        return true;
    };
    let mut items: Vec<_> = rd.flatten().collect();
    items.sort_by_key(|e| e.file_name());
    for e in items {
        *n += 1;
        if *n > MAX_ENTRIES {
            return false;
        }
        let name = e.file_name().to_string_lossy().to_string();
        let path = format!("{shown}/{name}");
        let Ok(meta) = fs::symlink_metadata(e.path()) else {
            continue;
        };
        let ft = meta.file_type();
        if ft.is_dir() {
            if name == "__pycache__" {
                continue;
            }
            h.update(format!("d\0{path}\n"));
            if !walk_fingerprint(&e.path(), &path, h, n) {
                return false;
            }
        } else if ft.is_symlink() {
            let target = fs::read_link(e.path()).unwrap_or_default();
            h.update(format!("l\0{path}\0{}\n", target.to_string_lossy()));
        } else {
            let mtime = meta
                .modified()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_nanos())
                .unwrap_or(0);
            h.update(format!("f\0{path}\0{}\0{mtime}\n", meta.len()));
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_an_editable_install_and_a_workspace_link_but_not_a_venv_naming_itself() {
        let d = tempfile::tempdir().unwrap();
        let root = crate::snapshot::canonical(d.path()).unwrap();
        let sp = root.join(".venv/lib/python3.12/site-packages");
        fs::create_dir_all(&sp).unwrap();
        fs::create_dir_all(root.join(".venv/bin")).unwrap();
        fs::create_dir_all(root.join("src/calc")).unwrap();
        fs::write(
            sp.join("__editable__.calc-0.0.1.pth"),
            format!("{}\n", root.join("src").display()),
        )
        .unwrap();
        fs::write(
            root.join(".venv/bin/pytest"),
            format!("#!{}/.venv/bin/python\n", root.display()),
        )
        .unwrap();
        let (found, total) = pointers(&root, &[".venv".to_string()]);
        assert_eq!(total, 1, "{found:?}");
        assert_eq!(
            found[0].file,
            ".venv/lib/python3.12/site-packages/__editable__.calc-0.0.1.pth"
        );
        assert_eq!(found[0].target, "src");

        #[cfg(unix)]
        {
            fs::create_dir_all(root.join("node_modules")).unwrap();
            std::os::unix::fs::symlink("../src/calc", root.join("node_modules/calc")).unwrap();
            let (found, _) = pointers(&root, &["node_modules".to_string()]);
            assert_eq!(found.len(), 1, "{found:?}");
            assert_eq!(found[0].target, "src/calc");
        }
    }

    #[test]
    fn the_fingerprint_changes_with_a_file_but_not_with_a_bytecode_cache() {
        let d = tempfile::tempdir().unwrap();
        let root = d.path();
        let sp = root.join(".venv/lib/site-packages/pkg");
        fs::create_dir_all(&sp).unwrap();
        fs::write(sp.join("__init__.py"), "x = 1\n").unwrap();
        let before = fingerprint(root, ".venv").unwrap();
        fs::create_dir_all(sp.join("__pycache__")).unwrap();
        fs::write(sp.join("__pycache__/__init__.cpython-312.pyc"), "cache").unwrap();
        assert_eq!(fingerprint(root, ".venv").unwrap(), before);
        fs::write(sp.join("evil.pth"), "import os\n").unwrap();
        assert_ne!(fingerprint(root, ".venv").unwrap(), before);
        assert_eq!(fingerprint(root, "missing"), None);
    }
}
