//! Run one check command in the snapshot, optionally inside the macOS sandbox.

use countercheck_core::budget;
use countercheck_core::config::{PlannedCheck, Shell};
use countercheck_core::receipt::CheckResult;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};
use wait_timeout::ChildExt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sandbox {
    /// Plain child process in the snapshot directory.
    None,
    /// macOS Seatbelt: no network, writes only inside the snapshot and temp folders.
    Seatbelt,
}

impl Sandbox {
    pub fn label(self) -> &'static str {
        match self {
            Sandbox::None if cfg!(windows) => "none (no sandbox on Windows yet)",
            Sandbox::None => "none",
            Sandbox::Seatbelt => {
                "macos-seatbelt (no internet, localhost only; writes only in the check folder)"
            }
        }
    }
}

/// True when sandbox-exec can apply a profile here (it can't inside another sandbox).
pub fn seatbelt_works() -> bool {
    if cfg!(windows) {
        return false;
    }
    Command::new("/usr/bin/sandbox-exec")
        .args(["-p", "(version 1)(allow default)", "/usr/bin/true"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

const TAIL_BYTES: u64 = 16 * 1024;
const TAIL_LINES: usize = 40;

/// Seatbelt profile string literal: escape backslash and quote.
fn sb_str(p: &Path) -> String {
    p.to_string_lossy()
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
}

fn seatbelt_profile(workdir: &Path, tmp: &Path, network: bool) -> String {
    if network {
        // `network = true`: the project says this check needs the internet. Writes stay confined.
        return format!(
            "(version 1)\n(allow default)\n\
             (deny file-write*)\n\
             (allow file-write* (subpath \"{}\") (subpath \"{}\") (literal \"/dev/null\") \
             (regex #\"^/dev/tty\") (regex #\"^/dev/fd/\"))\n",
            sb_str(workdir),
            sb_str(tmp)
        );
    }
    format!(
        // Network: the internet is blocked, this machine's loopback is allowed. Web tests start a
        // local server and a browser that talk over localhost (Playwright, Cypress, pytest-httpserver);
        // blocking that turned honest work into a false FAILED.
        "(version 1)\n(allow default)\n(deny network*)\n\
         (allow network-bind (local ip \"localhost:*\"))\n\
         (allow network-inbound (local ip \"localhost:*\"))\n\
         (allow network-outbound (remote ip \"localhost:*\"))\n\
         (deny file-write*)\n\
         (allow file-write* (subpath \"{}\") (subpath \"{}\") (literal \"/dev/null\") \
         (regex #\"^/dev/tty\") (regex #\"^/dev/fd/\"))\n",
        sb_str(workdir),
        sb_str(tmp)
    )
}

/// Shown when a check needs `sh` and Windows has none.
pub const NO_SH: &str = "countercheck could not find sh to run this check: install Git for Windows \
    (https://git-scm.com/download/win), or set `shell = \"pwsh\"` on this check in .countercheck/checks.toml";

/// Search `PATH` for `name`, skipping relative entries (a worker could plant a program there).
fn on_path(name: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .filter(|d| d.is_absolute())
        .map(|d| d.join(name))
        .find(|p| p.is_file())
}

/// Git for Windows' `sh.exe`: on `PATH`, else next to `git.exe`, else the default install folder.
#[cfg(windows)]
fn find_sh() -> Option<PathBuf> {
    static SH: std::sync::OnceLock<Option<PathBuf>> = std::sync::OnceLock::new();
    SH.get_or_init(|| {
        if let Some(p) = on_path("sh.exe") {
            return Some(p);
        }
        let exec = Command::new("git")
            .arg("--exec-path")
            .output()
            .ok()
            .filter(|o| o.status.success())
            .map(|o| PathBuf::from(String::from_utf8_lossy(&o.stdout).trim()));
        let program_files: Vec<PathBuf> = ["ProgramFiles", "ProgramW6432"]
            .iter()
            .filter_map(std::env::var_os)
            .map(PathBuf::from)
            .collect();
        sh_candidates(exec.as_deref(), &program_files)
            .into_iter()
            .find(|p| p.is_file())
    })
    .clone()
}

/// Where Git for Windows keeps `sh.exe`, given `git --exec-path` (e.g.
/// `C:/Program Files/Git/mingw64/libexec/git-core`) and the Program Files folders, in order.
#[cfg_attr(not(windows), allow(dead_code))]
fn sh_candidates(exec_path: Option<&Path>, program_files: &[PathBuf]) -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Some(exec) = exec_path {
        // <exec>/../../bin (older layouts), then <exec>/../../.. (the Git folder) /bin and /usr/bin.
        for up in [exec.ancestors().nth(2), exec.ancestors().nth(3)]
            .into_iter()
            .flatten()
        {
            out.push(up.join("bin").join("sh.exe"));
            out.push(up.join("usr").join("bin").join("sh.exe"));
        }
    }
    for dir in program_files {
        out.push(dir.join("Git").join("bin").join("sh.exe"));
    }
    out.push(PathBuf::from(r"C:\Program Files\Git\bin\sh.exe"));
    out
}

/// `sh` is looked up on `PATH` when it starts, as always on Unix.
#[cfg(not(windows))]
fn find_sh() -> Option<PathBuf> {
    Some(PathBuf::from("sh"))
}

/// The program for a check's shell, or a plain reason why it can't run here.
fn shell_program(shell: Shell) -> Result<PathBuf, String> {
    match shell {
        Shell::Sh => find_sh().ok_or_else(|| NO_SH.to_string()),
        Shell::Pwsh if cfg!(windows) => on_path("pwsh.exe")
            .or_else(|| on_path("powershell.exe"))
            .or_else(|| {
                let root = std::env::var_os("SystemRoot")?;
                let p = PathBuf::from(root).join(r"System32\WindowsPowerShell\v1.0\powershell.exe");
                p.is_file().then_some(p)
            })
            .ok_or_else(|| {
                "countercheck could not find PowerShell (pwsh or powershell.exe) to run this check"
                    .to_string()
            }),
        Shell::Pwsh => on_path("pwsh").ok_or_else(|| {
            "countercheck could not find pwsh (PowerShell) on PATH to run this check".to_string()
        }),
        Shell::Cmd if cfg!(windows) => Ok(std::env::var_os("ComSpec")
            .map(PathBuf::from)
            .filter(|p| p.is_absolute() && p.is_file())
            .unwrap_or_else(|| PathBuf::from("cmd.exe"))),
        Shell::Cmd => Err("shell = \"cmd\" needs Windows; this check can't run here".to_string()),
    }
}

/// Give `run` to the shell. `cmd` gets the command line as written (`/S /C "<run>"`): the usual
/// argument quoting would add backslashes that cmd doesn't understand.
fn add_run(cmd: &mut Command, shell: Shell, run: &str) {
    match shell {
        Shell::Sh => {
            cmd.arg("-c").arg(run);
        }
        Shell::Pwsh => {
            cmd.args(["-NoProfile", "-NonInteractive", "-Command"])
                .arg(run);
        }
        Shell::Cmd => {
            cmd.args(["/D", "/S", "/C"]);
            #[cfg(windows)]
            std::os::windows::process::CommandExt::raw_arg(cmd, format!("\"{run}\""));
            #[cfg(not(windows))]
            cmd.arg(run);
        }
    }
}

/// A check that could not start (e.g. its shell is missing): failed, with the reason as output.
fn not_started(spec: &PlannedCheck, why: String) -> CheckResult {
    CheckResult {
        name: spec.name.clone(),
        run: spec.run.clone(),
        exit_code: None,
        timed_out: false,
        duration_ms: 0,
        passed: false,
        output_sha256: countercheck_core::sha256_hex(why.as_bytes()),
        output_tail: why,
        network: spec.network,
    }
}

/// Kill the check and everything it started.
#[cfg(unix)]
fn kill_tree(child: &mut std::process::Child) {
    // Kill the whole process group so test runners' children die too. The child leads its
    // own group, so the group id is its pid. Spelled `-s KILL -- -<pgid>`: procps-ng 4's
    // `kill -KILL -<pgid>` misreads the group (e.g. -1234 as -1, which kills every process).
    let pgid = child.id();
    if pgid > 1 {
        let _ = Command::new("kill")
            .args(["-s", "KILL", "--", &format!("-{pgid}")])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }
    let _ = child.kill();
}

/// Windows: put the check in a Job Object that kills every process in it when it is closed, so
/// nothing a check started outlives it (a background server would otherwise keep countercheck's
/// output open, and a timed-out test runner's children would keep running). `None` if Windows
/// refused; `kill_tree` then falls back to `taskkill /T`.
#[cfg(windows)]
fn watch_tree(child: &std::process::Child) -> Option<win32job::Job> {
    use std::os::windows::io::AsRawHandle;
    let mut info = win32job::ExtendedLimitInfo::new();
    info.limit_kill_on_job_close();
    let job = win32job::Job::create_with_limit_info(&info).ok()?;
    job.assign_process(child.as_raw_handle() as isize).ok()?;
    Some(job)
}

/// Kill the check and everything it started: close its Job Object, and `taskkill /T` in case a
/// process got out before the check was put in the job.
#[cfg(windows)]
fn kill_tree(child: &mut std::process::Child, job: Option<win32job::Job>) {
    drop(job);
    let taskkill = std::env::var_os("SystemRoot")
        .map(|r| PathBuf::from(r).join(r"System32\taskkill.exe"))
        .filter(|p| p.is_file())
        .unwrap_or_else(|| PathBuf::from("taskkill.exe"));
    let _ = Command::new(taskkill)
        .args(["/T", "/F", "/PID", &child.id().to_string()])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    let _ = child.kill();
}

pub fn run_check(
    spec: &PlannedCheck,
    workdir: &Path,
    tmp: &Path,
    sandbox: Sandbox,
) -> Result<CheckResult, String> {
    let program = match shell_program(spec.shell) {
        Ok(p) => p,
        Err(why) => return Ok(not_started(spec, why)),
    };
    let log = tempfile::tempfile_in(tmp).map_err(|e| format!("temp file: {e}"))?;
    let out = log.try_clone().map_err(|e| e.to_string())?;
    let err = log.try_clone().map_err(|e| e.to_string())?;

    let mut cmd = match sandbox {
        Sandbox::None => Command::new(&program),
        Sandbox::Seatbelt => {
            let mut c = Command::new("/usr/bin/sandbox-exec");
            c.arg("-p")
                .arg(seatbelt_profile(workdir, tmp, spec.network))
                .arg(&program);
            c
        }
    };
    add_run(&mut cmd, spec.shell, &spec.run);
    cmd.current_dir(workdir)
        .env("COUNTERCHECK", "1")
        .env("TMPDIR", tmp);
    if cfg!(windows) {
        // Windows programs use TEMP/TMP, not TMPDIR: keep their temp files in the check folder too.
        cmd.env("TEMP", tmp).env("TMP", tmp);
    }
    cmd.envs(spec.env.iter().map(|(k, v)| (k, v)))
        .stdin(Stdio::null())
        .stdout(Stdio::from(out))
        .stderr(Stdio::from(err));
    #[cfg(unix)]
    std::os::unix::process::CommandExt::process_group(&mut cmd, 0);

    let started = Instant::now();
    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound && spec.shell != Shell::Sh => {
            return Ok(not_started(
                spec,
                format!("countercheck could not start {}: {e}", program.display()),
            ));
        }
        Err(e) => return Err(format!("check \"{}\": {e}", spec.name)),
    };
    // Unix: the check leads its own process group (set before it started).
    #[cfg(windows)]
    let job = watch_tree(&child);
    let status = child
        .wait_timeout(Duration::from_secs(spec.timeout_secs))
        .map_err(|e| e.to_string())?;
    let (exit_code, timed_out) = match status {
        Some(s) => {
            // Stop what the check left running in the background (a server, a `sleep &`), so it
            // can't outlive the check, its timeout or the run. Windows: closing the job. Unix:
            // killing the check's process group; the group id can't be reused while any of its
            // processes is alive, so this only reaches what the check started.
            #[cfg(windows)]
            drop(job);
            #[cfg(unix)]
            kill_tree(&mut child);
            (s.code(), false)
        }
        None => {
            #[cfg(windows)]
            kill_tree(&mut child, job);
            #[cfg(unix)]
            kill_tree(&mut child);
            let _ = child.wait();
            (None, true)
        }
    };
    let duration_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
    let metric_name = spec.budget.as_ref().map(|b| b.metric.as_str());
    let (output_sha256, output_tail, metric) = read_output(log, metric_name)?;
    let mut result = CheckResult {
        name: spec.name.clone(),
        run: spec.run.clone(),
        exit_code,
        timed_out,
        duration_ms,
        passed: !timed_out && exit_code == Some(0),
        output_tail,
        output_sha256,
        network: spec.network,
    };
    if let Some(b) = &spec.budget
        && result.passed
    {
        let (ok, line) = budget::judge(b, metric);
        result.passed = ok;
        result.output_tail = if result.output_tail.is_empty() {
            line
        } else {
            format!("{}\n{line}", result.output_tail)
        };
    }
    // Only when the command itself failed (not when it passed but missed a budget).
    let command_failed = timed_out || exit_code != Some(0);
    if command_failed
        && sandbox == Sandbox::Seatbelt
        && !spec.network
        && looks_like_network_error(&result.output_tail)
    {
        // A hint only: the verdict never changes, or a worker could fail on purpose behind a
        // blocked network call to turn FAILED into something softer.
        result.output_tail.push_str(NO_INTERNET_HINT);
    }
    Ok(result)
}

/// Error text that usually means a blocked or failed connection (Python, Node, curl, Go, Java).
/// Only decides whether to show a hint, never the verdict, so a guess is fine here.
fn looks_like_network_error(output: &str) -> bool {
    const SIGNS: [&str; 13] = [
        "operation not permitted",
        "nodename nor servname",
        "name or service not known",
        "temporary failure in name resolution",
        "getaddrinfo",
        "could not resolve",
        "enotfound",
        "eai_again",
        "network is unreachable",
        "no route to host",
        "urlopen error",
        "unknownhostexception",
        "dial tcp",
    ];
    let lower = output.to_lowercase();
    SIGNS.iter().any(|s| lower.contains(s))
}

/// Added to a failed check that ran without internet access.
pub const NO_INTERNET_HINT: &str = "\n[countercheck] this check ran without internet access (sandbox); \
    if it needs the internet, set `network = true` on it in .countercheck/checks.toml";

/// Longest output line looked at for a `METRIC` value; longer lines are skipped.
const MAX_METRIC_LINE: usize = 4096;

/// Hash of the whole output, its last lines, and the last value of `metric` if asked for.
fn read_output(
    mut log: File,
    metric: Option<&str>,
) -> Result<(String, String, Option<f64>), String> {
    use sha2::{Digest, Sha256};
    log.seek(SeekFrom::Start(0)).map_err(|e| e.to_string())?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 64 * 1024];
    let mut total: u64 = 0;
    // The metric can be anywhere in the output, not just the tail: scan every line.
    let mut line: Vec<u8> = Vec::new();
    let mut too_long = false;
    let mut value = None;
    let mut end_line = |line: &mut Vec<u8>, too_long: &mut bool| {
        if let Some(name) = metric
            && !*too_long
            && let Some(v) = budget::metric_in_line(&String::from_utf8_lossy(line), name)
        {
            value = Some(v);
        }
        line.clear();
        *too_long = false;
    };
    loop {
        let n = log.read(&mut buf).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        let chunk = buf.get(..n).unwrap_or(&[]);
        hasher.update(chunk);
        total = total.saturating_add(n as u64);
        if metric.is_some() {
            for &b in chunk {
                if b == b'\n' {
                    end_line(&mut line, &mut too_long);
                } else if line.len() < MAX_METRIC_LINE {
                    line.push(b);
                } else {
                    too_long = true;
                }
            }
        }
    }
    end_line(&mut line, &mut too_long);
    let hash: String = hasher
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    let start = total.saturating_sub(TAIL_BYTES);
    log.seek(SeekFrom::Start(start))
        .map_err(|e| e.to_string())?;
    let mut tail = Vec::new();
    log.read_to_end(&mut tail).map_err(|e| e.to_string())?;
    let text = String::from_utf8_lossy(&tail);
    let lines: Vec<&str> = text.lines().collect();
    let keep = lines.len().saturating_sub(TAIL_LINES);
    Ok((hash, lines.get(keep..).unwrap_or(&[]).join("\n"), value))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn git_for_windows_sh_is_looked_for_next_to_git() {
        let exec = Path::new("C:/Program Files/Git/mingw64/libexec/git-core");
        let c = sh_candidates(Some(exec), &[PathBuf::from("D:/Apps")]);
        let git = Path::new("C:/Program Files/Git");
        assert!(c.contains(&git.join("bin").join("sh.exe")), "{c:?}");
        assert!(
            c.contains(&git.join("usr").join("bin").join("sh.exe")),
            "{c:?}"
        );
        let pf = c.iter().position(|p| p.starts_with("D:/Apps")).unwrap();
        let next_to_git = c.iter().position(|p| p.starts_with(git)).unwrap();
        assert!(next_to_git < pf, "next to git.exe comes first: {c:?}");
        assert_eq!(
            c.last().unwrap(),
            &PathBuf::from(r"C:\Program Files\Git\bin\sh.exe")
        );
        assert_eq!(sh_candidates(None, &[]).len(), 1);
    }

    #[test]
    fn a_check_whose_shell_is_missing_fails_with_the_reason() {
        assert!(NO_SH.contains("Git for Windows") && NO_SH.contains("shell = \"pwsh\""));
        let spec = PlannedCheck {
            name: "t".into(),
            run: "pytest".into(),
            timeout_secs: 5,
            env: Vec::new(),
            golden: None,
            budget: None,
            network: false,
            shell: Shell::Sh,
        };
        let r = not_started(&spec, NO_SH.to_string());
        assert!(!r.passed && !r.timed_out && r.exit_code.is_none());
        assert_eq!(r.output_tail, NO_SH);
    }

    #[test]
    fn the_sandbox_label_says_when_there_is_none_on_windows() {
        let label = Sandbox::None.label();
        if cfg!(windows) {
            assert_eq!(label, "none (no sandbox on Windows yet)");
            assert!(!seatbelt_works());
        } else {
            assert_eq!(label, "none");
        }
    }
}
