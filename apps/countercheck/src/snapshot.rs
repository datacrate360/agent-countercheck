//! A clean copy of the worker's files, with protected files put back to their base version.

use std::fs;
use std::path::{Component, Path, PathBuf};

/// Reject paths that could escape the snapshot (git never produces them, but be strict). On
/// Windows this also refuses drive and UNC prefixes like `C:` and `\\server`.
pub fn safe_rel(path: &str) -> Result<PathBuf, String> {
    let p = Path::new(path);
    if p.is_absolute() || p.components().any(|c| !matches!(c, Component::Normal(_))) {
        return Err(format!("refusing unsafe path \"{path}\""));
    }
    Ok(p.to_path_buf())
}

/// `path` below `base` with `/` between the parts on every OS, so receipts, scans and the tree
/// hash name the same file the same way on Windows and Unix.
pub fn rel_slash(base: &Path, path: &Path) -> String {
    let rel = path.strip_prefix(base).unwrap_or(path);
    if cfg!(windows) {
        rel.components()
            .map(|c| c.as_os_str().to_string_lossy())
            .collect::<Vec<_>>()
            .join("/")
    } else {
        rel.to_string_lossy().into_owned()
    }
}

/// Like `canonicalize`, without Windows' `\\?\` prefix: `cmd` can't start in such a folder, and
/// paths compared with `starts_with` must be spelled the same way.
pub fn canonical(p: &Path) -> std::io::Result<PathBuf> {
    let real = p.canonicalize()?;
    if cfg!(windows) {
        let text = real.to_string_lossy();
        if let Some(rest) = text.strip_prefix(r"\\?\UNC\") {
            return Ok(PathBuf::from(format!(r"\\{rest}")));
        }
        if let Some(rest) = text.strip_prefix(r"\\?\")
            && rest.as_bytes().get(1) == Some(&b':')
        {
            return Ok(PathBuf::from(rest));
        }
    }
    Ok(real)
}

/// Copy `files` from `root` into `dest`. Symlinks are recreated as links, never followed. Returns
/// the symlinks that had to be written as small text files instead (Windows without Developer
/// Mode refuses to create symlinks; git does the same with `core.symlinks=false`).
pub fn copy_files(root: &Path, dest: &Path, files: &[String]) -> Result<Vec<String>, String> {
    let mut as_text = Vec::new();
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
            if cfg!(windows) {
                if link_or_text(&target, &to, || make_symlink(&src, &target, &to))
                    .map_err(|e| format!("{f}: {e}"))?
                {
                    as_text.push(f.clone());
                }
            } else {
                make_symlink(&src, &target, &to).map_err(|e| format!("{f}: {e}"))?;
            }
        } else if meta.is_file() {
            fs::copy(&src, &to).map_err(|e| format!("{f}: {e}"))?;
        }
    }
    Ok(as_text)
}

#[cfg(unix)]
fn make_symlink(_src: &Path, target: &Path, to: &Path) -> std::io::Result<()> {
    std::os::unix::fs::symlink(target, to)
}

/// Windows has two kinds of symlink; pick the one matching what the link points at.
#[cfg(windows)]
fn make_symlink(src: &Path, target: &Path, to: &Path) -> std::io::Result<()> {
    if fs::metadata(src).is_ok_and(|m| m.is_dir()) {
        std::os::windows::fs::symlink_dir(target, to)
    } else {
        std::os::windows::fs::symlink_file(target, to)
    }
}

/// Create the link with `create`; if the system refuses, write the link's target path (with `/`)
/// as the file's text, like git does. True when it fell back to text.
fn link_or_text(
    target: &Path,
    to: &Path,
    create: impl FnOnce() -> std::io::Result<()>,
) -> Result<bool, String> {
    if create().is_ok() {
        return Ok(false);
    }
    let text = target.to_string_lossy().replace('\\', "/");
    fs::write(to, text).map_err(|e| e.to_string())?;
    Ok(true)
}

/// Make `name` (a folder in the worker's tree) visible in the snapshot: a symlink on Unix, a
/// directory junction on Windows (no admin rights needed), else a copy.
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
    link_folder(&src, &to).map_err(|e| format!("{name}: {e}"))?;
    Ok(true)
}

#[cfg(unix)]
fn link_folder(src: &Path, to: &Path) -> Result<(), String> {
    std::os::unix::fs::symlink(src, to).map_err(|e| e.to_string())
}

#[cfg(windows)]
fn link_folder(src: &Path, to: &Path) -> Result<(), String> {
    if junction::create(src, to).is_ok() {
        return Ok(());
    }
    let _ = fs::remove_dir(to); // a half-made junction
    copy_dir(src, to)
}

/// Copy a folder; symlinks inside it are skipped, never followed.
#[cfg(windows)]
fn copy_dir(src: &Path, to: &Path) -> Result<(), String> {
    fs::create_dir_all(to).map_err(|e| format!("{}: {e}", to.display()))?;
    let rd = fs::read_dir(src).map_err(|e| format!("{}: {e}", src.display()))?;
    for entry in rd {
        let entry = entry.map_err(|e| e.to_string())?;
        let ft = entry.file_type().map_err(|e| e.to_string())?;
        let dest = to.join(entry.file_name());
        if ft.is_dir() {
            copy_dir(&entry.path(), &dest)?;
        } else if ft.is_file() {
            fs::copy(entry.path(), &dest).map_err(|e| format!("{}: {e}", dest.display()))?;
        }
    }
    Ok(())
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
        let rel = rel_slash(base, &path);
        if ft.is_dir() {
            walk(base, &path, out)?;
        } else if ft.is_symlink() {
            let target = fs::read_link(&path).map_err(|e| e.to_string())?;
            let mut target = target.to_string_lossy().into_owned();
            if cfg!(windows) {
                target = target.replace('\\', "/");
            }
            out.push((rel, format!("link:{target}")));
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
        if cfg!(windows) {
            assert!(safe_rel("C:/Windows/x").is_err());
            assert!(safe_rel("C:x").is_err());
            assert!(safe_rel(r"\\server\share\x").is_err());
        }
    }

    #[test]
    fn a_refused_symlink_becomes_a_text_file_with_its_target() {
        let d = tempfile::tempdir().unwrap();
        let to = d.path().join("link");
        let refused = || Err(std::io::Error::other("symlinks need Developer Mode"));
        let fell_back = link_or_text(Path::new(r"..\shared\config.toml"), &to, refused).unwrap();
        assert!(fell_back);
        assert_eq!(fs::read_to_string(&to).unwrap(), "../shared/config.toml");
        let made = link_or_text(Path::new("x"), &d.path().join("other"), || Ok(())).unwrap();
        assert!(!made);
    }

    #[test]
    fn paths_use_forward_slashes_and_the_tree_hash_ignores_the_os() {
        let d = tempfile::tempdir().unwrap();
        let base = d.path();
        let nested = base.join("src").join("sub").join("a.txt");
        assert_eq!(rel_slash(base, &nested), "src/sub/a.txt");
        fs::create_dir_all(nested.parent().unwrap()).unwrap();
        fs::write(&nested, "hello\n").unwrap();
        fs::write(base.join("top.txt"), "top\n").unwrap();
        // The hash is over "path\0sha256\n" lines with `/` paths, sorted: the same on every OS.
        let line =
            |p: &str, c: &str| format!("{p}\0{}\n", countercheck_core::sha256_hex(c.as_bytes()));
        let expected = countercheck_core::sha256_hex(
            format!(
                "{}{}",
                line("src/sub/a.txt", "hello\n"),
                line("top.txt", "top\n")
            )
            .as_bytes(),
        );
        assert_eq!(tree_hash(base).unwrap(), expected);
    }
}
