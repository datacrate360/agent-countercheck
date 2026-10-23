//! `countercheck report`: read the saved receipts and write one self-contained HTML page.

use crate::git;
use countercheck_core::receipt::Receipt;
use countercheck_core::report::{self, Entry};
use std::path::{Path, PathBuf};

/// Receipts bigger than this are skipped (a receipt is a few KB; output tails are capped).
const MAX_RECEIPT_BYTES: u64 = 8 * 1024 * 1024;

pub struct Options {
    pub repo: PathBuf,
    /// Receipt folder (default `<repo>/.countercheck/receipts`).
    pub receipts: Option<PathBuf>,
    /// Output file (default `<receipts>/report.html`, git-ignored with the receipts).
    pub out: Option<PathBuf>,
    pub open: bool,
}

/// Where the receipts are, and what the page shows as the project.
pub struct Located {
    pub dir: PathBuf,
    /// The repository's folder name.
    pub title: String,
    /// The repository's folder (else the receipt folder), with the home folder shortened to `~`.
    pub path: String,
}

/// `path` with the home folder written as `~`, so a shared report doesn't show the user name.
pub fn display_path(path: &Path) -> String {
    let home = std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .filter(|h| !h.is_empty())
        .map(PathBuf::from);
    // Compare spelled the same way: canonical, without Windows' `\\?\` (git and HOME can differ,
    // e.g. macOS /private/var, or a Windows 8.3 short name like RUNNER~1).
    let real = crate::snapshot::canonical(path).unwrap_or_else(|_| path.to_path_buf());
    let homes = home
        .iter()
        .flat_map(|h| [Some(h.clone()), crate::snapshot::canonical(h).ok()])
        .flatten();
    if let Some(rest) = homes
        .filter_map(|h| {
            real.strip_prefix(&h)
                .or_else(|_| path.strip_prefix(&h))
                .ok()
                .map(Path::to_path_buf)
        })
        .next()
    {
        let sep = std::path::MAIN_SEPARATOR;
        return if rest.as_os_str().is_empty() {
            "~".to_string()
        } else {
            format!("~{sep}{}", rest.display())
        };
    }
    path.display().to_string()
}

/// The receipt folder, the page title (the repository's folder name) and the project's path.
pub fn locate(repo: &Path, receipts: Option<&Path>) -> Result<Located, String> {
    let root = git::toplevel(repo).ok();
    let dir = match (receipts, &root) {
        (Some(d), _) => d.to_path_buf(),
        (None, Some(r)) => r.join(".countercheck").join("receipts"),
        (None, None) => {
            return Err(format!(
                "{} is not inside a git repository; pass --receipts <folder>",
                repo.display()
            ));
        }
    };
    let title = root
        .as_deref()
        .and_then(Path::file_name)
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "receipts".to_string());
    let path = display_path(root.as_deref().unwrap_or(&dir));
    Ok(Located { dir, title, path })
}

/// Write the report; returns its path and how many receipts it shows.
pub fn run(opts: &Options) -> Result<(PathBuf, usize), String> {
    let Located { dir, title, path } = locate(&opts.repo, opts.receipts.as_deref())?;
    let entries = read_receipts(&dir)?;
    if entries.is_empty() {
        return Err(format!(
            "no receipts in {}; run `countercheck verify` first",
            dir.display()
        ));
    }
    let now = humantime::format_rfc3339_seconds(std::time::SystemTime::now()).to_string();
    let html = report::render_html(
        &entries,
        &report::Page {
            title: &title,
            path: &path,
            generated_at: &now,
        },
    );
    let out = opts.out.clone().unwrap_or_else(|| dir.join("report.html"));
    std::fs::write(&out, html).map_err(|e| format!("{}: {e}", out.display()))?;
    if opts.open {
        open_in_browser(out.as_os_str());
    }
    Ok((out, entries.len().min(report::MAX_RUNS)))
}

/// Every `*.json` in `dir` that parses as a receipt; anything else is skipped.
pub fn read_receipts(dir: &Path) -> Result<Vec<Entry>, String> {
    if !dir.exists() {
        return Ok(Vec::new()); // nothing verified yet: the caller says how to start
    }
    let rd = std::fs::read_dir(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let mut out = Vec::new();
    for e in rd.flatten() {
        let path = e.path();
        if path.extension().is_none_or(|x| x != "json") {
            continue;
        }
        let Ok(meta) = e.metadata() else { continue };
        if !meta.is_file() || meta.len() > MAX_RECEIPT_BYTES {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        let Ok(receipt) = serde_json::from_str::<Receipt>(&text) else {
            continue;
        };
        let seal_ok = receipt.seal_is_valid();
        out.push(Entry {
            file: e.file_name().to_string_lossy().into_owned(),
            receipt,
            seal_ok,
        });
    }
    Ok(out)
}

/// Best effort: if no browser opens, the printed path is enough.
pub fn open_in_browser(path: &std::ffi::OsStr) {
    let mut cmd = if cfg!(target_os = "macos") {
        std::process::Command::new("open")
    } else if cfg!(windows) {
        std::process::Command::new("cmd")
    } else {
        std::process::Command::new("xdg-open")
    };
    add_target(&mut cmd, path);
    let _ = cmd
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn();
}

/// Windows: `cmd /D /C start "" "<path or url>"`, written out as is. The usual argument quoting
/// leaves a path without spaces unquoted, and cmd would then split it at `&` or `^`. Windows paths
/// and our URLs never contain `"`.
#[cfg(windows)]
fn add_target(cmd: &mut std::process::Command, path: &std::ffi::OsStr) {
    use std::os::windows::process::CommandExt;
    cmd.args(["/D", "/C", "start"]).raw_arg(format!(
        "\"\" \"{}\"",
        path.to_string_lossy().replace('"', "")
    ));
}

#[cfg(not(windows))]
fn add_target(cmd: &mut std::process::Command, path: &std::ffi::OsStr) {
    cmd.arg(path);
}
