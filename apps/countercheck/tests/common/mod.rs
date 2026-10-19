//! Shared by the end-to-end tests.

use std::path::PathBuf;

/// The `sh` the tests use to set up repositories: `sh` on Unix; on Windows Git for Windows' `sh.exe`
/// (on `PATH`, else next to `git.exe`), the same one countercheck runs checks with.
pub fn sh_program() -> PathBuf {
    if !cfg!(windows) {
        return PathBuf::from("sh");
    }
    let on_path = std::env::var_os("PATH").and_then(|p| {
        std::env::split_paths(&p)
            .map(|d| d.join("sh.exe"))
            .find(|p| p.is_file())
    });
    if let Some(p) = on_path {
        return p;
    }
    let out = std::process::Command::new("git")
        .arg("--exec-path")
        .output()
        .unwrap();
    let exec = PathBuf::from(String::from_utf8_lossy(&out.stdout).trim());
    exec.ancestors()
        .skip(1)
        .take(3)
        .map(|d| d.join("bin").join("sh.exe"))
        .find(|p| p.is_file())
        .unwrap_or_else(|| panic!("no sh.exe found near {}", exec.display()))
}

/// `git init` for a test repository: a fixed identity, and files kept byte for byte (no CRLF
/// conversion on Windows), so every OS sees the same bytes.
pub const GIT_INIT: &str = "git init -q -b main && git config user.email t@t && git config user.name t && git config core.autocrlf false";
