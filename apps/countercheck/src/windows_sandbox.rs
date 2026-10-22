//! The Windows check sandbox: `countercheck sandbox-exec` (hidden), started by the runner for
//! every check, sets itself up and then runs the check. No admin rights needed.
//!
//! 1. **A SID for this run** ([`run_sid`]): random, shaped like a local account no machine has.
//!    `verify` grants it full control on the check folder and its temp dir when they are created
//!    (so everything copied in inherits it); this helper grants it on the check's
//!    `writes = [...]` locations and takes it off again afterwards.
//! 2. **A write-restricted token**: the check runs with this user's token, all privileges but
//!    traverse removed, and every write must also be allowed for the run's SID, the logon
//!    session's, `Everyone` or `RESTRICTED` (the last three for system objects like the NUL
//!    device; on disk almost nothing grants them write). Reads work as before, so Git for
//!    Windows' `sh`, Python and node run unchanged; writes anywhere else are refused.
//! 3. **A Job Object** with kill-on-close: everything the check started, even what it started
//!    in the background with `start` or `&`, stops when the check ends or times out.
//! 4. Run the check with its timeout, stop what is left, remove the grants on `writes`.
//!
//! Not done, and the sandbox label says so: blocking the internet. Windows' unprivileged option
//! (an AppContainer) blocks it and keeps localhost within the container, but can't read
//! toolchains outside Program Files (a Python in the user's folders, the repository's
//! `node_modules`) or run Git for Windows' `sh`; a firewall rule needs admin rights. See
//! docs/decisions/0003-windows-sandbox.md.
//!
//! What each machine gives is checked by `--probe` (a real refused write), and if a step fails
//! at run time the check fails (exit 125) rather than run with less than the receipt says.

use countercheck_winsandbox as ffi;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Exit code when the sandbox itself couldn't be set up (the check didn't run).
pub const SETUP_FAILED: i32 = 125;
/// Exit code when the check ran out of time (the runner reports it as a timeout).
pub const TIMED_OUT: i32 = 124;
/// Printed (to the check's output) when the check ran out of time.
pub const TIMEOUT_LINE: &str = "[countercheck] the check timed out";

/// The `Everyone` group (`S-1-1-0`) and the `RESTRICTED` SID (`S-1-5-12`).
const EVERYONE: &str = "S-1-1-0";
const RESTRICTED: &str = "S-1-5-12";

/// Most files and folders a `writes` location may hold for us to grant and revoke access.
const MAX_ENTRIES: usize = 100_000;

/// `FILE_ATTRIBUTE_REPARSE_POINT`: a symlink, junction or other link. Never followed or changed.
const REPARSE_POINT: u32 = 0x400;

/// Options for one sandboxed run.
pub struct Run {
    pub work: PathBuf,
    pub writes: Vec<PathBuf>,
    pub sid: String,
    pub timeout_secs: u64,
    /// The check's whole command line (see `runner::windows_command_line`).
    pub cmdline: String,
}

/// The SID of this countercheck run: made once per process, used for every check.
pub fn run_sid() -> Result<String, String> {
    static SID: std::sync::OnceLock<Result<String, String>> = std::sync::OnceLock::new();
    SID.get_or_init(|| {
        let mut buf = [0u8; 16];
        getrandom::fill(&mut buf).map_err(|e| format!("random SID: {e}"))?;
        let mut parts = [0u32; 4];
        for (p, chunk) in parts.iter_mut().zip(buf.as_chunks::<4>().0) {
            *p = u32::from_le_bytes(*chunk);
        }
        Ok(ffi::sddl::run_sid(parts))
    })
    .clone()
}

/// Allow `sid` full control on one file or folder (a folder passes it on to what is created
/// inside). The object itself must not be a link.
fn grant_one(path: &Path, sid: &str, dir: bool) -> Result<(), String> {
    let dacl = ffi::file_dacl(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let new = ffi::sddl::add_full_control(&dacl, sid, dir)
        .map_err(|e| format!("{}: {e}", path.display()))?;
    if new != dacl {
        ffi::set_file_dacl(path, &new).map_err(|e| format!("{}: {e}", path.display()))?;
    }
    Ok(())
}

/// Take every entry for `sid` off one file or folder.
fn revoke_one(path: &Path, sid: &str) -> Result<(), String> {
    let dacl = ffi::file_dacl(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let (new, removed) =
        ffi::sddl::remove_sid(&dacl, sid).map_err(|e| format!("{}: {e}", path.display()))?;
    if removed {
        ffi::set_file_dacl(path, &new).map_err(|e| format!("{}: {e}", path.display()))?;
    }
    Ok(())
}

fn is_link(meta: &std::fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    meta.file_type().is_symlink() || meta.file_attributes() & REPARSE_POINT != 0
}

/// Call `f(path, is_dir)` on `root` and everything below it, never entering or touching a link
/// (a junction a check left in a `writes` folder can't lead us to other files).
fn walk(root: &Path, f: &mut dyn FnMut(&Path, bool) -> Result<(), String>) -> Result<(), String> {
    let meta = std::fs::symlink_metadata(root).map_err(|e| format!("{}: {e}", root.display()))?;
    if is_link(&meta) {
        return Err(format!(
            "{} is a link; a writes location must be a real folder",
            root.display()
        ));
    }
    let mut stack = vec![(root.to_path_buf(), meta.is_dir())];
    let mut seen = 0usize;
    while let Some((path, dir)) = stack.pop() {
        seen += 1;
        if seen > MAX_ENTRIES {
            return Err(format!(
                "{} holds more than {MAX_ENTRIES} files and folders",
                root.display()
            ));
        }
        f(&path, dir)?;
        if !dir {
            continue;
        }
        let entries = std::fs::read_dir(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        for e in entries {
            let e = e.map_err(|e| format!("{}: {e}", path.display()))?;
            let meta = std::fs::symlink_metadata(e.path())
                .map_err(|err| format!("{}: {err}", e.path().display()))?;
            if is_link(&meta) {
                continue;
            }
            stack.push((e.path(), meta.is_dir()));
        }
    }
    Ok(())
}

/// Let the run's SID write in the check folder and its temp dir. Called by `verify` while they
/// are still empty, so everything created in them inherits it; nothing else is changed.
pub fn grant_scratch(work: &Path, tmp: &Path) -> Result<(), String> {
    let sid = run_sid()?;
    for dir in [work, tmp] {
        grant_one(dir, &sid, true).map_err(|e| format!("Windows sandbox: {e}"))?;
    }
    Ok(())
}

/// Grant `sid` on a `writes` location and everything already in it (created if missing, as on
/// macOS the check may create it).
fn grant_writes(path: &Path, sid: &str) -> Result<(), String> {
    std::fs::create_dir_all(path).map_err(|e| format!("{}: {e}", path.display()))?;
    walk(path, &mut |p, dir| grant_one(p, sid, dir))
}

/// Take `sid` off a `writes` location again, including what the check created there. Errors are
/// reported but the walk goes on.
fn revoke_writes(path: &Path, sid: &str) {
    if !path.exists() {
        return;
    }
    let _ = walk(path, &mut |p, _| {
        if let Err(e) = revoke_one(p, sid) {
            eprintln!("[countercheck] could not remove the sandbox's access from {e}");
        }
        Ok(())
    });
}

/// Start `cmdline` write-restricted to `sid`, in a kill-on-close Job Object, and wait for it up
/// to `timeout`: its exit code, or `None` on timeout. Everything it started is stopped either way.
fn run_restricted(
    cmdline: &str,
    cwd: &Path,
    sid: &str,
    timeout: Duration,
) -> Result<Option<u32>, String> {
    let (user, logon) = ffi::current_sids().map_err(|e| format!("token: {e}"))?;
    // Everyone and RESTRICTED as well, like Chromium's restricted tokens and Windows' own
    // write-restricted services: the NUL device (`>NUL`, /dev/null) and the named-object
    // directories that MSYS (Git's sh) and .NET's crypto library create objects in are granted to
    // them, not to the logon session. On disk, they open only what grants write to everyone.
    let restricting = [sid, logon.as_str(), EVERYONE, RESTRICTED];
    let dacl = ffi::sddl::default_dacl(&[user.as_str(), sid, logon.as_str()])?;
    let child = ffi::spawn_restricted(&ffi::Restricted {
        command_line: cmdline,
        cwd,
        restricting_sids: &restricting,
        default_dacl: &dacl,
    })
    .map_err(|e| format!("could not start the check: {e}"))?;
    let mut info = win32job::ExtendedLimitInfo::new();
    info.limit_kill_on_job_close();
    let job = match win32job::Job::create_with_limit_info(&info)
        .and_then(|j| j.assign_process(child.raw_handle()).map(|()| j))
    {
        Ok(j) => j,
        Err(e) => {
            let _ = child.kill();
            return Err(format!("Job Object: {e}"));
        }
    };
    if let Err(e) = child.resume() {
        let _ = child.kill();
        return Err(format!("could not start the check: {e}"));
    }
    let status = child.wait(timeout).map_err(|e| format!("wait: {e}"));
    // Closing the job stops everything still in it: background servers, `start`ed programs.
    drop(job);
    let _ = child.wait(Duration::from_secs(5));
    status
}

/// `countercheck sandbox-exec --probe`: `restricted=1` when this machine refused a write from a
/// write-restricted process (and let it write where it was allowed), else `restricted=0`.
pub fn probe_main() -> u8 {
    let ok = probe_write_refused().unwrap_or_else(|e| {
        eprintln!("[countercheck] Windows sandbox probe: {e}");
        false
    });
    println!("restricted={}", u8::from(ok));
    0
}

fn probe_write_refused() -> Result<bool, String> {
    let dir = tempfile::Builder::new()
        .prefix("countercheck-probe-")
        .tempdir()
        .map_err(|e| e.to_string())?;
    let allowed = dir.path().join("allowed");
    let denied = dir.path().join("denied");
    std::fs::create_dir_all(&allowed).map_err(|e| e.to_string())?;
    std::fs::create_dir_all(&denied).map_err(|e| e.to_string())?;
    let sid = run_sid()?;
    grant_one(&allowed, &sid, true)?;
    let cmd = std::env::var_os("ComSpec")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute() && p.is_file())
        .unwrap_or_else(|| PathBuf::from(r"C:\Windows\System32\cmd.exe"));
    let line = format!(
        "\"{}\" /D /C \"echo x>allowed\\a.txt & echo x>denied\\b.txt\"",
        cmd.display()
    );
    run_restricted(&line, dir.path(), &sid, Duration::from_secs(30))?;
    Ok(allowed.join("a.txt").exists() && !denied.join("b.txt").exists())
}

/// Parse `--probe` output.
pub fn parse_probe(out: &str) -> bool {
    out.contains("restricted=1")
}

/// Whether the sandbox works here, probed once per process with the given binary.
pub fn probe(exe: &Path) -> bool {
    static OK: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *OK.get_or_init(|| {
        std::process::Command::new(exe)
            .args(["sandbox-exec", "--probe"])
            .stdin(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .output()
            .ok()
            .filter(|o| o.status.success())
            .is_some_and(|o| parse_probe(&String::from_utf8_lossy(&o.stdout)))
    })
}

/// `countercheck sandbox-exec --sid ... --cmdline ...`: grant, run, clean up; the check's exit code.
pub fn run_main(run: Run) -> i32 {
    let mut granted = Vec::new();
    for w in &run.writes {
        granted.push(w.clone());
        if let Err(e) = grant_writes(w, &run.sid) {
            eprintln!("[countercheck] the Windows sandbox could not allow writes to {e}");
            for g in &granted {
                revoke_writes(g, &run.sid);
            }
            return SETUP_FAILED;
        }
    }
    let status = run_restricted(
        &run.cmdline,
        &run.work,
        &run.sid,
        Duration::from_secs(run.timeout_secs),
    );
    for g in &granted {
        revoke_writes(g, &run.sid);
    }
    match status {
        // Windows exit codes are 32-bit; passed on as is (a crash code stays a failure).
        Ok(Some(code)) => code as i32,
        Ok(None) => {
            eprintln!("{TIMEOUT_LINE} after {} s", run.timeout_secs);
            TIMED_OUT
        }
        Err(e) => {
            eprintln!("[countercheck] the Windows sandbox could not run the check: {e}");
            SETUP_FAILED
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn probe_output_is_parsed() {
        assert!(parse_probe("restricted=1\n"));
        assert!(!parse_probe("restricted=0\n"));
        assert!(!parse_probe(""));
    }

    #[test]
    fn one_sid_per_run() {
        let a = run_sid().unwrap();
        assert!(a.starts_with("S-1-5-21-"), "{a}");
        assert_eq!(run_sid().unwrap(), a);
    }
}
