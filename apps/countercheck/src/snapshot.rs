//! A clean copy of the worker's files, with protected files put back to their base version.

use std::fs;
use std::path::{Component, Path, PathBuf};

/// Reject paths that could escape the snapshot (git never produces them, but be strict).
fn safe_rel(path: &str) -> Result<PathBuf, String> {
    let p = Path::new(path);
    if p.is_absolute() || p.components().any(|c| !matches!(c, Component::Normal(_))) {
        return Err(format!("refusing unsafe path \"{path}\""));
    }
    Ok(p.to_path_buf())
}

/// Copy `files` from `root` into `dest`. Symlinks are recreated as links, never followed.
pub fn copy_files(root: &Path, dest: &Path, files: &[String]) -> Result<(), String> {
    for f in files {
        if countercheck_core::protect::is_junk(f) {
            continue;
        }
        let rel = safe_rel(f)?;
        let src = root.join(&rel);
        let Ok(meta) = fs::symlink_metadata(&src) else {
            continue; // in the index but deleted in the working tree
        };
        let to = dest.join(&rel);
        if let Some(parent) = to.parent() {
            fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
        }
        if meta.file_type().is_symlink() {
            let target = fs::read_link(&src).map_err(|e| format!("{f}: {e}"))?;
            std::os::unix::fs::symlink(&target, &to).map_err(|e| format!("{f}: {e}"))?;
        } else if meta.is_file() {
            fs::copy(&src, &to).map_err(|e| format!("{f}: {e}"))?;
        }
    }
    Ok(())
}

/// Make `name` (a folder in the worker's tree) visible in the snapshot as a symlink.
pub fn link_dir(root: &Path, dest: &Path, name: &str) -> Result<bool, String> {
    let rel = safe_rel(name)?;
    let src = root.join(&rel);
    let to = dest.join(&rel);
    if !src.is_dir() || fs::symlink_metadata(&to).is_ok() {
        return Ok(false);
    }
    if let Some(parent) = to.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
    }
    std::os::unix::fs::symlink(&src, &to).map_err(|e| format!("{name}: {e}"))?;
    Ok(true)
}

/// Put a protected file back to its base content (or remove it if it didn't exist at base).
pub fn restore(dest: &Path, path: &str, base_content: Option<&[u8]>) -> Result<(), String> {
    let to = dest.join(safe_rel(path)?);
    match base_content {
        Some(bytes) => {
            if let Some(parent) = to.parent() {
                fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
            }
            if fs::symlink_metadata(&to).is_ok() {
                fs::remove_file(&to).map_err(|e| format!("{path}: {e}"))?;
            }
            fs::write(&to, bytes).map_err(|e| format!("{path}: {e}"))
        }
        None => match fs::remove_file(&to) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(format!("{path}: {e}")),
        },
    }
}

/// SHA-256 over the sorted (path, content hash) list of everything in the snapshot.
pub fn tree_hash(dest: &Path) -> Result<String, String> {
    let mut entries = Vec::new();
    walk(dest, dest, &mut entries)?;
    entries.sort();
    let mut joined = String::new();
    for (path, hash) in entries {
        joined.push_str(&path);
        joined.push('\0');
        joined.push_str(&hash);
        joined.push('\n');
    }
    Ok(countercheck_core::sha256_hex(joined.as_bytes()))
}

fn walk(base: &Path, dir: &Path, out: &mut Vec<(String, String)>) -> Result<(), String> {
    let rd = fs::read_dir(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    for entry in rd {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();
        let ft = entry.file_type().map_err(|e| e.to_string())?;
        let rel = path
            .strip_prefix(base)
            .unwrap_or(&path)
            .to_string_lossy()
            .into_owned();
        if ft.is_dir() {
            walk(base, &path, out)?;
        } else if ft.is_symlink() {
            let target = fs::read_link(&path).map_err(|e| e.to_string())?;
            out.push((rel, format!("link:{}", target.to_string_lossy())));
        } else {
            let bytes = fs::read(&path).map_err(|e| format!("{rel}: {e}"))?;
            out.push((rel, countercheck_core::sha256_hex(&bytes)));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unsafe_paths_are_rejected() {
        assert!(safe_rel("../etc/passwd").is_err());
        assert!(safe_rel("/etc/passwd").is_err());
        assert!(safe_rel("a/./b").is_ok() || safe_rel("a/./b").is_err());
        assert!(safe_rel("src/main.rs").is_ok());
    }
}
