//! Run one check command in the snapshot, optionally inside the macOS, Linux or Windows sandbox.

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
    /// Linux: Landlock confines writes; with `namespaces`, also a loopback-only network and a
    /// PID namespace (`countercheck sandbox-exec`).
    #[cfg_attr(not(target_os = "linux"), allow(dead_code))]
    Landlock { namespaces: bool },
    /// Windows: a write-restricted token confines writes; a Job Object stops everything the
    /// check started (`countercheck sandbox-exec`). The internet is not blocked.
    #[cfg_attr(not(windows), allow(dead_code))]
    RestrictedToken,
    /// Windows, a run with `sh` checks: `pwsh`/`cmd` checks run in the restricted token, `sh`
    /// checks without it (Git for Windows' `sh`, MSYS, can't start under a write-restricted token:
    /// its shared memory and signal pipes are secured to the user account only). Each `sh` check
    /// says so on the receipt (`sandboxed: false`), and the label says the run was partial.
    #[cfg_attr(not(windows), allow(dead_code))]
    RestrictedTokenPartial,
    /// Windows after `countercheck windows-setup` (ADR-0004): checks run as the low-rights
    /// `countercheck-runner` account inside the restricted token and Job Object; a firewall rule
    /// blocks that account's internet (not loopback), and Git's `sh` runs sandboxed too.
    #[cfg_attr(not(windows), allow(dead_code))]
    WindowsRunner,
}

/// Why an `sh` check with `network = true` ran outside the Windows full sandbox.
pub const SH_NETWORK_UNSANDBOXED: &str = "network = true: this check runs as you, because the runner \
    account has no internet, and Git for Windows' sh can't run in your restricted token; set shell = \
    \"pwsh\" or \"cmd\" on it to keep its writes confined";

/// Why an `sh` check ran outside the Windows sandbox (on the receipt, per check).
pub const SH_UNSANDBOXED: &str = "Git for Windows' sh can't run in a restricted token (MSYS secures its \
    shared memory and signal pipes to the user account only); set shell = \"pwsh\" or \"cmd\" on this check \
    to sandbox it";

impl Sandbox {
    pub fn label(self) -> &'static str {
        match self {
            Sandbox::None => "none",
            Sandbox::Seatbelt => {
                "macos-seatbelt (no internet, localhost only; writes only in the check folder)"
            }
            Sandbox::Landlock { namespaces: true } => {
                "linux-landlock (no internet, localhost only; writes only in the check folder)"
            }
            Sandbox::Landlock { namespaces: false } => {
                "linux-landlock (writes only in the check folder; internet NOT blocked: this system doesn't allow user namespaces)"
            }
            Sandbox::RestrictedToken => {
                "windows-restricted-token (writes only in the check folder; internet NOT blocked)"
            }
            Sandbox::RestrictedTokenPartial => {
                "windows-restricted-token, partial: sh checks ran unsandboxed (Git for Windows can't run in a restricted token); pwsh/cmd checks: writes only in the check folder; internet NOT blocked"
            }
            Sandbox::WindowsRunner => {
                "windows-runner (no internet, localhost only; writes only in the check folder; checks run as countercheck-runner)"
            }
        }
    }

    /// Writes outside the check folder are refused.
    pub fn confines_writes(self) -> bool {
        !matches!(self, Sandbox::None)
    }

    /// The sandbox one check runs in, and why it runs outside the run's sandbox if it does: in a
    /// partial Windows run, `sh` checks run without it and the others in the restricted token.
    /// Under the Windows full sandbox, a `network = true` check runs as the developer (the runner
    /// account has no internet): restricted for `pwsh`/`cmd`, unsandboxed for `sh`.
    pub fn for_check(self, shell: Shell, network: bool) -> (Sandbox, Option<&'static str>) {
        match self {
            Sandbox::RestrictedTokenPartial if shell == Shell::Sh => {
                (Sandbox::None, Some(SH_UNSANDBOXED))
            }
            Sandbox::RestrictedTokenPartial => (Sandbox::RestrictedToken, None),
            Sandbox::WindowsRunner if network && shell == Shell::Sh => {
                (Sandbox::None, Some(SH_NETWORK_UNSANDBOXED))
            }
            Sandbox::WindowsRunner if network => (Sandbox::RestrictedToken, None),
            s => (s, None),
        }
    }

    /// The internet is blocked (localhost still works).
    pub fn blocks_internet(self) -> bool {
        matches!(
            self,
            Sandbox::Seatbelt | Sandbox::Landlock { namespaces: true } | Sandbox::WindowsRunner
        )
    }
}

/// The best sandbox this system has: Seatbelt on macOS, Landlock on Linux, a restricted token on
/// Windows, else none.
pub fn auto_sandbox() -> Sandbox {
    if seatbelt_works() {
        return Sandbox::Seatbelt;
    }
    if cfg!(windows) {
        return windows_auto();
    }
    linux_auto()
}

#[cfg(windows)]
fn windows_auto() -> Sandbox {
    match std::env::current_exe() {
        Ok(exe) if crate::windows_runner::probe(&exe) => Sandbox::WindowsRunner,
        Ok(exe) if crate::windows_sandbox::probe(&exe) => Sandbox::RestrictedToken,
        _ => Sandbox::None,
    }
}

#[cfg(not(windows))]
fn windows_auto() -> Sandbox {
    Sandbox::None
}

#[cfg(target_os = "linux")]
fn linux_auto() -> Sandbox {
    let Ok(exe) = std::env::current_exe() else {
        return Sandbox::None;
    };
    let caps = crate::linux_sandbox::probe(&exe);
    if caps.landlock {
        Sandbox::Landlock {
            namespaces: caps.namespaces,
        }
    } else {
        Sandbox::None
    }
}

#[cfg(not(target_os = "linux"))]
fn linux_auto() -> Sandbox {
    Sandbox::None
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

fn seatbelt_profile(workdir: &Path, tmp: &Path, network: bool, writes: &[PathBuf]) -> String {
    // Extra locations from `writes = [...]` (validated and resolved before the check runs).
    let extra: String = writes
        .iter()
        .map(|p| format!(" (subpath \"{}\")", sb_str(p)))
        .collect();
    if network {
        // `network = true`: the project says this check needs the internet. Writes stay confined.
        return format!(
            "(version 1)\n(allow default)\n\
             (deny file-write*)\n\
             (allow file-write* (subpath \"{}\") (subpath \"{}\"){extra} (literal \"/dev/null\") \
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
         (allow file-write* (subpath \"{}\") (subpath \"{}\"){extra} (literal \"/dev/null\") \
         (regex #\"^/dev/tty\") (regex #\"^/dev/fd/\"))\n",
        sb_str(workdir),
        sb_str(tmp)
    )
}

/// The extra write locations of a check (`writes = [...]`), with placeholders filled in and
/// symlinks resolved, refused if they could reach the repository or the home folder. Only used
/// under the sandbox; without one, nothing restricts writes anyway.
pub fn resolve_writes(spec: &PlannedCheck, repo: &Path) -> Result<Vec<PathBuf>, String> {
    if spec.writes.is_empty() {
        return Ok(Vec::new());
    }
    let (home_var, user_var) = if cfg!(windows) {
        ("USERPROFILE", "USERNAME")
    } else {
        ("HOME", "USER")
    };
    let home = std::env::var(home_var).map_err(|_| format!("writes: {home_var} is not set"))?;
    let user = std::env::var(user_var).unwrap_or_default();
    let uid = if cfg!(windows) {
        if spec.writes.iter().any(|t| t.contains("{uid}")) {
            return Err(format!(
                "check \"{}\": writes: {{uid}} isn't available on Windows; use {{user}} or {{home}}",
                spec.name
            ));
        }
        String::new()
    } else if spec.writes.iter().any(|t| t.contains("{uid}")) {
        Command::new("/usr/bin/id")
            .arg("-u")
            .output()
            .ok()
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
            .unwrap_or_default()
    } else {
        String::new()
    };
    let real_home = resolve_existing(Path::new(&home));
    let real_repo = resolve_existing(repo);
    // Windows: `{home}` fills in with forward slashes, like the rest of a check file's paths.
    let home_for_template = if cfg!(windows) {
        display_path(&real_home).replace('\\', "/")
    } else {
        home.clone()
    };
    let mut out = Vec::new();
    for t in &spec.writes {
        let path = countercheck_core::writes::expand(t, &uid, &user, &home_for_template)
            .map_err(|e| format!("check \"{}\": {e}", spec.name))?;
        let real = PathBuf::from(display_path(&resolve_existing(Path::new(&path))));
        countercheck_core::writes::check_resolved(
            &spec.name,
            &comparable(&real)?,
            &comparable(&real_home)?,
            &comparable(&real_repo)?,
        )?;
        out.push(real);
    }
    Ok(out)
}

/// A resolved path without Windows' `\\?\` prefix (which `canonicalize` adds), for the receipt
/// and the sandbox. Unchanged elsewhere.
pub fn display_path(p: &Path) -> String {
    let s = p.to_string_lossy();
    match s.strip_prefix(r"\\?\") {
        Some(rest) if !rest.starts_with("UNC") => rest.to_string(),
        _ => s.into_owned(),
    }
}

/// A resolved path in the form `writes::check_resolved` compares: on Windows `/` separators and
/// lowercase (Windows paths ignore case), and only on a local drive.
fn comparable(p: &Path) -> Result<String, String> {
    let s = display_path(p);
    if !cfg!(windows) {
        return Ok(s);
    }
    if s.starts_with(r"\\") {
        return Err(format!(
            "writes path {s} is on a network share; the sandbox needs a local drive"
        ));
    }
    Ok(s.replace('\\', "/").to_lowercase())
}

/// `path` with symlinks resolved as far as it exists, the rest appended (a location the check
/// creates doesn't exist yet). `/var/tmp/x` becomes `/private/var/tmp/x` on macOS.
fn resolve_existing(path: &Path) -> PathBuf {
    let mut rest = Vec::new();
    let mut cur = path.to_path_buf();
    loop {
        if let Ok(real) = cur.canonicalize() {
            return rest.iter().rev().fold(real, |p: PathBuf, seg| p.join(seg));
        }
        match (cur.file_name().map(|s| s.to_os_string()), cur.parent()) {
            (Some(name), Some(parent)) => {
                rest.push(name);
                cur = parent.to_path_buf();
            }
            _ => return path.to_path_buf(),
        }
    }
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

/// One argument quoted the way Rust's standard library quotes it on Windows (the rules
/// `CommandLineToArgvW`, the C runtime and MSYS parse): in quotes when it is empty or has a space
/// or tab, a `"` escaped with a backslash, backslashes before a `"` (or the closing quote) doubled.
pub fn quote_windows_arg(out: &mut String, arg: &str) {
    let quote = arg.is_empty() || arg.contains([' ', '\t']);
    if quote {
        out.push('"');
    }
    let mut backslashes = 0usize;
    for c in arg.chars() {
        if c == '\\' {
            backslashes += 1;
        } else {
            if c == '"' {
                out.extend(std::iter::repeat_n('\\', backslashes + 1));
            }
            backslashes = 0;
        }
        out.push(c);
    }
    if quote {
        out.extend(std::iter::repeat_n('\\', backslashes));
        out.push('"');
    }
}

/// The whole Windows command line for a check, exactly as `Command` builds it without a sandbox:
/// the program in quotes, then `-c <run>` (sh), `-NoProfile -NonInteractive -Command <run>`
/// (pwsh) or `/D /S /C "<run>"` as written (cmd). The Windows sandbox starts it itself.
#[cfg_attr(not(windows), allow(dead_code))]
pub fn windows_command_line(program: &Path, shell: Shell, run: &str) -> Result<String, String> {
    let program = program.to_string_lossy();
    if program.contains('"') {
        return Err(format!("the shell's path has a quote in it: {program}"));
    }
    let mut out = format!("\"{program}\"");
    let args: &[&str] = match shell {
        Shell::Sh => &["-c"],
        Shell::Pwsh => &["-NoProfile", "-NonInteractive", "-Command"],
        Shell::Cmd => {
            out.push_str(&format!(" /D /S /C \"{run}\""));
            return Ok(out);
        }
    };
    for a in args.iter().copied().chain([run]) {
        out.push(' ');
        quote_windows_arg(&mut out, a);
    }
    Ok(out)
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
        writes: Vec::new(),
        sandboxed: None,
        unsandboxed_reason: None,
        needs_network: false,
    }
}

/// Seconds in `ps -o etime` format (`[[dd-]hh:]mm:ss`).
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn etime_secs(s: &str) -> Option<u64> {
    let (days, rest) = match s.split_once('-') {
        Some((d, r)) => (d.parse::<u64>().ok()?, r),
        None => (0, s),
    };
    let mut total = 0u64;
    for part in rest.split(':') {
        total = total
            .checked_mul(60)?
            .checked_add(part.parse::<u64>().ok()?)?;
    }
    days.checked_mul(86_400)?.checked_add(total)
}

/// Processes adopted by launchd (ppid 1), owned by `uid`, younger than `max_age` seconds: the
/// only ones a just-finished check can have detached. From `ps -A -o pid=,ppid=,uid=,etime=`.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn orphan_candidates(ps: &str, uid: u32, max_age: u64, me: u32) -> Vec<u32> {
    ps.lines()
        .filter_map(|l| {
            let mut f = l.split_whitespace();
            let pid = f.next()?.parse::<u32>().ok()?;
            let ppid = f.next()?.parse::<u32>().ok()?;
            let owner = f.next()?.parse::<u32>().ok()?;
            let age = etime_secs(f.next()?)?;
            (ppid == 1 && owner == uid && age <= max_age && pid != me).then_some(pid)
        })
        .collect()
}

/// The candidates that belong to the check, from `lsof -F pfin -d cwd,0,1,2`: working folder
/// inside one of `roots`, or stdin/stdout/stderr on the check's output file.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn confirmed_detached(lsof: &str, roots: &[String], log_inode: u64) -> Vec<u32> {
    let mut out = Vec::new();
    let (mut pid, mut fd) = (None, String::new());
    let inside = |p: &str| {
        roots.iter().any(|r| {
            p == r
                || p.strip_prefix(r.as_str())
                    .is_some_and(|x| x.starts_with('/'))
        })
    };
    for line in lsof.lines() {
        let (k, v) = line.split_at(line.len().min(1));
        match k {
            "p" => pid = v.parse::<u32>().ok(),
            "f" => fd = v.to_string(),
            "i" if matches!(fd.as_str(), "0" | "1" | "2")
                && log_inode != 0
                && v.parse::<u64>().ok() == Some(log_inode) =>
            {
                out.extend(pid)
            }
            "n" if fd == "cwd" && inside(v) => out.extend(pid),
            _ => {}
        }
    }
    out.sort_unstable();
    out.dedup();
    out
}

/// Stop processes a check detached from its process group (macOS). A process that also left the
/// check folder and closed its output can still escape; the sandbox limits what it can do.
#[cfg(target_os = "macos")]
fn sweep_detached(dirs: &[&Path], log_inode: u64, ran_secs: u64) {
    use std::os::unix::fs::MetadataExt;
    let Some(uid) = dirs
        .first()
        .and_then(|d| d.metadata().ok())
        .map(|m| m.uid())
    else {
        return;
    };
    let roots: Vec<String> = dirs
        .iter()
        .filter_map(|d| d.canonicalize().ok())
        .map(|d| d.to_string_lossy().into_owned())
        .collect();
    for _ in 0..5 {
        let Ok(ps) = Command::new("/bin/ps")
            .args(["-A", "-o", "pid=,ppid=,uid=,etime="])
            .output()
        else {
            return;
        };
        let candidates = orphan_candidates(
            &String::from_utf8_lossy(&ps.stdout),
            uid,
            ran_secs.saturating_add(2),
            std::process::id(),
        );
        if candidates.is_empty() {
            return;
        }
        let list = candidates
            .iter()
            .map(u32::to_string)
            .collect::<Vec<_>>()
            .join(",");
        let Ok(lsof) = Command::new("/usr/sbin/lsof")
            .args(["-nP", "-w", "-F", "pfin", "-d", "cwd,0,1,2", "-p", &list])
            .output()
        else {
            return;
        };
        let hits = confirmed_detached(&String::from_utf8_lossy(&lsof.stdout), &roots, log_inode);
        if hits.is_empty() {
            return;
        }
        for p in hits {
            let _ = Command::new("/bin/kill")
                .args(["-9", &p.to_string()])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status();
        }
        std::thread::sleep(Duration::from_millis(50));
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
    writes: &[PathBuf],
) -> Result<CheckResult, String> {
    let (sandbox, unsandboxed) = sandbox.for_check(spec.shell, spec.network);
    let program = match shell_program(spec.shell) {
        Ok(p) => p,
        Err(why) => return Ok(not_started(spec, why)),
    };
    let log = tempfile::tempfile_in(tmp).map_err(|e| format!("temp file: {e}"))?;
    // The output file's inode: a detached process that kept the check's stdout is found by it.
    #[cfg(target_os = "macos")]
    let log_inode = {
        use std::os::unix::fs::MetadataExt;
        log.metadata().map(|m| m.ino()).unwrap_or(0)
    };
    let out = log.try_clone().map_err(|e| e.to_string())?;
    let err = log.try_clone().map_err(|e| e.to_string())?;

    let mut cmd = match sandbox {
        Sandbox::None => Command::new(&program),
        // Mapped to `RestrictedToken` or `None` per check above.
        Sandbox::RestrictedTokenPartial => Command::new(&program),
        Sandbox::Seatbelt => {
            let mut c = Command::new("/usr/bin/sandbox-exec");
            c.arg("-p")
                .arg(seatbelt_profile(workdir, tmp, spec.network, writes))
                .arg(&program);
            c
        }
        Sandbox::Landlock { namespaces } => {
            // The helper enforces the timeout itself, so it can kill everything the check
            // started (it is their subreaper) before it exits.
            let exe = std::env::current_exe().map_err(|e| format!("countercheck binary: {e}"))?;
            let mut c = Command::new(exe);
            c.arg("sandbox-exec")
                .arg("--work")
                .arg(workdir)
                .arg("--tmp")
                .arg(tmp)
                .arg("--timeout-secs")
                .arg(spec.timeout_secs.to_string());
            for w in writes {
                c.arg("--write").arg(w);
            }
            if spec.network {
                c.arg("--network");
            }
            if namespaces {
                c.arg("--namespaces");
            }
            c.arg("--").arg(&program);
            c
        }
        Sandbox::RestrictedToken | Sandbox::WindowsRunner => {
            // The helper starts the check itself (a restricted token needs its own
            // CreateProcess), with the command line `Command` would have built, and enforces
            // the timeout so it can take its grants off the `writes` locations before it exits.
            let exe = std::env::current_exe().map_err(|e| format!("countercheck binary: {e}"))?;
            #[cfg(windows)]
            let sid = crate::windows_sandbox::run_sid()?;
            #[cfg(not(windows))]
            let sid = String::new();
            let line = match windows_command_line(&program, spec.shell, &spec.run) {
                Ok(l) => l,
                Err(why) => return Ok(not_started(spec, why)),
            };
            let mut c = Command::new(exe);
            c.arg("sandbox-exec")
                .arg("--work")
                .arg(workdir)
                .arg("--tmp")
                .arg(tmp)
                .arg("--timeout-secs")
                .arg(spec.timeout_secs.to_string())
                .arg("--sid")
                .arg(sid)
                .arg("--cmdline")
                .arg(line);
            for w in writes {
                c.arg("--write").arg(w);
            }
            // The full sandbox: the helper runs the check as the runner account, through the
            // copy of countercheck `verify` made for it.
            #[cfg(windows)]
            if sandbox == Sandbox::WindowsRunner {
                let exe = crate::windows_runner::helper()
                    .ok_or("the Windows runner's copy of countercheck is missing")?;
                c.arg("--runner-exe").arg(exe);
            }
            c
        }
    };
    if !matches!(sandbox, Sandbox::RestrictedToken | Sandbox::WindowsRunner) {
        add_run(&mut cmd, spec.shell, &spec.run);
    }
    cmd.current_dir(workdir)
        .env("COUNTERCHECK", "1")
        .env("TMPDIR", tmp)
        // Python reads and writes bytecode caches here, never in `__pycache__` folders: a cache
        // planted in a linked venv (or anywhere a check reads) can't stand in for the source.
        .env("PYTHONPYCACHEPREFIX", tmp.join("pycache"));
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
    // Under the Linux sandbox the helper times the check out itself; this wait is the safety net.
    let grace = match sandbox {
        Sandbox::Landlock { .. } | Sandbox::RestrictedToken => 15,
        // The runner account's sign-in comes first.
        Sandbox::WindowsRunner => 45,
        _ => 0,
    };
    let status = child
        .wait_timeout(Duration::from_secs(spec.timeout_secs.saturating_add(grace)))
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
    // macOS has no subreaper or PID namespace: find what the check detached (`setsid`, double
    // fork) by its working folder or its hold on the check's output, and stop it.
    #[cfg(target_os = "macos")]
    sweep_detached(&[workdir, tmp], log_inode, started.elapsed().as_secs());
    let duration_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
    let metric_name = spec.budget.as_ref().map(|b| b.metric.as_str());
    let (output_sha256, output_tail, metric) = read_output(log, metric_name)?;
    // The Linux helper's own timeout: exit 124 and its line in the output. A check can only fake
    // this to fail.
    #[cfg(target_os = "linux")]
    let (exit_code, timed_out) = if matches!(sandbox, Sandbox::Landlock { .. })
        && exit_code == Some(i32::from(crate::linux_sandbox::TIMED_OUT))
        && output_tail.contains(crate::linux_sandbox::TIMEOUT_LINE)
    {
        (None, true)
    } else {
        (exit_code, timed_out)
    };
    #[cfg(windows)]
    let (exit_code, timed_out) =
        if matches!(sandbox, Sandbox::RestrictedToken | Sandbox::WindowsRunner)
            && exit_code == Some(crate::windows_sandbox::TIMED_OUT)
            && output_tail.contains(crate::windows_sandbox::TIMEOUT_LINE)
        {
            (None, true)
        } else {
            (exit_code, timed_out)
        };
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
        writes: if sandbox.confines_writes() {
            writes
                .iter()
                .map(|p| p.to_string_lossy().into_owned())
                .collect()
        } else {
            Vec::new()
        },
        sandboxed: unsandboxed.map(|_| false),
        unsandboxed_reason: unsandboxed.map(str::to_string),
        needs_network: false,
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
    // Hints only: the verdict never changes, or a worker could fail on purpose behind a blocked
    // call to turn FAILED into something softer. Computed on the output before any hint is added.
    if command_failed && sandbox.confines_writes() {
        let out = &result.output_tail;
        // Landlock refuses a write with EACCES ("Permission denied"), Seatbelt with EPERM,
        // Windows with ERROR_ACCESS_DENIED ("Access is denied", or EACCES in sh and Python).
        let eacces = matches!(
            sandbox,
            Sandbox::Landlock { .. } | Sandbox::RestrictedToken | Sandbox::WindowsRunner
        );
        let denied = denied_write_path(out, eacces);
        let network = sandbox.blocks_internet() && !spec.network && looks_like_network_error(out);
        // "Operation not permitted" with no path and no other sign: under Seatbelt a refused
        // connection to an IP address and a refused write print exactly this, so say both.
        // Windows' bare "Access is denied." (cmd) can only be a write here.
        let unclear = denied.is_none()
            && !network
            && match sandbox {
                Sandbox::Seatbelt => has_bare_denial(out, false),
                Sandbox::RestrictedToken | Sandbox::WindowsRunner => has_bare_denial(out, true),
                _ => false,
            };
        if let Some(path) = denied {
            result.output_tail.push_str(&write_denied_hint(&path));
        }
        if network {
            result.output_tail.push_str(NO_INTERNET_HINT);
        }
        if unclear {
            result
                .output_tail
                .push_str(if sandbox == Sandbox::RestrictedToken {
                    WINDOWS_DENIAL_HINT
                } else if spec.network {
                    UNCLEAR_DENIAL_HINT
                } else {
                    UNCLEAR_DENIAL_NETWORK_HINT
                });
        }
    }
    Ok(result)
}

/// Error text that usually means a blocked or failed connection (Python, Node, curl, Go, Java).
/// Only decides whether to show a hint, never the verdict, so a guess is fine here.
fn looks_like_network_error(output: &str) -> bool {
    const SIGNS: [&str; 12] = [
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
    // Not "Operation not permitted": the sandbox prints it for a refused file write too (see
    // `has_bare_denial`).
    let lower = output.to_lowercase();
    SIGNS.iter().any(|s| lower.contains(s))
}

/// A line saying "Operation not permitted" (or `EPERM`; with `eacces`, also "Access is denied")
/// with no path in it.
fn has_bare_denial(output: &str, eacces: bool) -> bool {
    output
        .lines()
        .any(|line| is_permission_denial(&line.to_lowercase(), eacces) && path_in(line).is_none())
}

/// The sandbox's refusal, in the words of the C library, Python, Node, the shells and Windows
/// (`eacces`: Landlock and Windows refuse with EACCES / ERROR_ACCESS_DENIED).
fn is_permission_denial(lower_line: &str, eacces: bool) -> bool {
    lower_line.contains("operation not permitted")
        || lower_line.contains("eperm")
        || (eacces
            && (lower_line.contains("permission denied")
                || lower_line.contains("eacces")
                || lower_line.contains("access is denied")
                || lower_line.contains("access to the path")))
}

/// The file a refused write names, if the output has one: a line with "Operation not permitted"
/// (or `EPERM`) and an absolute path, e.g. Python `PermissionError: [Errno 1] Operation not
/// permitted: '/var/tmp/x'`, sh `sh: /var/tmp/x: Operation not permitted`, Node `EPERM: operation
/// not permitted, open '/var/tmp/x'`. Node prints the path on its own `path:` line too.
fn denied_write_path(output: &str, eacces: bool) -> Option<String> {
    let lines: Vec<&str> = output.lines().collect();
    lines.iter().enumerate().find_map(|(i, line)| {
        if !is_permission_denial(&line.to_lowercase(), eacces) {
            return None;
        }
        path_in(line).or_else(|| {
            // Node: `path: '/var/tmp/x'` a few lines below the error line.
            lines
                .iter()
                .skip(i + 1)
                .take(6)
                .find_map(|l| l.trim_start().strip_prefix("path:").and_then(path_in))
        })
    })
}

/// The first absolute path in a line (quoted or followed by `:`/`,`), not part of a URL: Unix
/// (`/var/tmp/x`) or Windows with a drive letter (`C:\x`, `C:/x`; not a URL scheme).
fn path_in(line: &str) -> Option<String> {
    line.split(|c: char| c.is_whitespace() || matches!(c, '\'' | '"' | ',' | '(' | ')'))
        .map(|t| t.trim_end_matches(':'))
        .find(|t| {
            let b = t.as_bytes();
            let unix = t.len() > 1 && t.starts_with('/') && !t.starts_with("//");
            let drive = b.len() > 3
                && b[0].is_ascii_alphabetic()
                && b[1] == b':'
                && (b[2] == b'\\' || b[2] == b'/')
                && b[3] != b'/';
            unix || drive
        })
        .map(|t| t.chars().take(200).collect())
}

fn write_denied_hint(path: &str) -> String {
    format!(
        "\n[countercheck] this check tried to write outside the check folder ({path}); the sandbox allows \
         writes only in the check folder and the temp dir. Point the tool at $TMPDIR if it can be \
         changed, or allow that location for this check with `writes = [\"...\"]` in \
         .countercheck/checks.toml"
    )
}

/// Added when the sandbox refused something and the output doesn't say what.
pub const UNCLEAR_DENIAL_NETWORK_HINT: &str = "\n[countercheck] the sandbox refused something this check \
    did (\"Operation not permitted\"): a connection to the internet, or a write outside the check folder and the \
    temp dir. If it needs the internet, set `network = true` on it in .countercheck/checks.toml; if it writes to a \
    fixed path, point it at $TMPDIR or allow it with `writes = [\"...\"]` on the check";
/// The same, for a check that already has `network = true` (so only a write can be refused).
pub const UNCLEAR_DENIAL_HINT: &str = "\n[countercheck] the sandbox refused something this check did \
    (\"Operation not permitted\"), probably a write outside the check folder and the temp dir; point it at \
    $TMPDIR or allow the location with `writes = [\"...\"]` on the check";

/// The same under the Windows sandbox, which refuses only writes.
pub const WINDOWS_DENIAL_HINT: &str = "\n[countercheck] the sandbox refused something this check did \
    (\"Access is denied\"), probably a write outside the check folder and the temp dir; point it at \
    %TEMP% or allow the location with `writes = [\"...\"]` on the check";

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
    fn detached_processes_are_found_by_folder_or_output_only() {
        assert_eq!(etime_secs("00:07"), Some(7));
        assert_eq!(etime_secs("01:02:03"), Some(3723));
        assert_eq!(etime_secs("2-00:00:01"), Some(172_801));
        assert_eq!(etime_secs("x"), None);
        let ps = "  10     1   501      00:03\n  11     1   501      05:00\n  12    99   501      00:01\n  13     1     0      00:01\n  14     1   501      00:01\n";
        // Young orphans of this user only, never this process.
        assert_eq!(orphan_candidates(ps, 501, 4, 14), vec![10]);
        let roots = vec![
            "/private/var/x/work".to_string(),
            "/private/var/x/tmp".to_string(),
        ];
        let lsof = "p10\nfcwd\nn/private/var/x/work/sub\np11\nfcwd\nn/Users/me\nf1\ni777\np12\nfcwd\nn/private/var/x/workshop\np13\nfcwd\nn/\nf2\ni555\n";
        // 10: cwd inside; 11: holds the output (inode 777); 12: a sibling folder, not inside;
        // 13: another file.
        assert_eq!(confirmed_detached(lsof, &roots, 777), vec![10, 11]);
        assert_eq!(
            confirmed_detached(lsof, &roots, 0),
            vec![10],
            "no inode, no match by it"
        );
    }

    #[test]
    fn a_refused_write_is_told_apart_from_a_refused_connection() {
        // Real output from macOS's sandbox (countercheck 0.5.3, macOS 27).
        let writes = [
            "PermissionError: [Errno 1] Operation not permitted: '/var/tmp/cc-probe-x'",
            "sh: /var/tmp/cc-probe-y: Operation not permitted",
            "mkdir: /var/tmp/cc-probe-dir: Operation not permitted",
            "Error: EPERM: operation not permitted, open '/var/tmp/cc-probe-z'\n  errno: -1,\n  code: 'EPERM',\n  syscall: 'open',\n  path: '/var/tmp/cc-probe-z'\n}",
        ];
        let expected = [
            Some("/var/tmp/cc-probe-x"),
            Some("/var/tmp/cc-probe-y"),
            Some("/var/tmp/cc-probe-dir"),
            Some("/var/tmp/cc-probe-z"),
        ];
        for (out, want) in writes.iter().zip(expected) {
            assert_eq!(denied_write_path(out, false).as_deref(), want, "{out}");
        }
        for out in writes {
            assert!(
                !looks_like_network_error(out),
                "a write is not a network error: {out}"
            );
        }
        // No path, no other sign: a refused connection to an IP and a refused write (issue #16's
        // runner) print the same, so neither the write nor the network hint, only "unclear".
        for out in [
            "runner crashed: PermissionError(1, 'Operation not permitted')",
            "PermissionError: [Errno 1] Operation not permitted",
        ] {
            assert_eq!(denied_write_path(out, false), None, "{out}");
            assert!(!looks_like_network_error(out), "{out}");
            assert!(has_bare_denial(out, false), "{out}");
        }
        // Refused connections by name: the network hint, and no write hint.
        let connects = [
            "urllib.error.URLError: <urlopen error [Errno 8] nodename nor servname provided, or not known>",
            "curl: (6) Could not resolve host: example.com",
            "Error: getaddrinfo ENOTFOUND example.com",
        ];
        for out in connects {
            assert!(looks_like_network_error(out), "{out}");
            assert_eq!(denied_write_path(out, false), None, "{out}");
        }
        // A URL is not a path.
        assert_eq!(path_in("fetch https://example.com/a failed"), None);
    }

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
            writes: Vec::new(),
        };
        let r = not_started(&spec, NO_SH.to_string());
        assert!(!r.passed && !r.timed_out && r.exit_code.is_none());
        assert_eq!(r.output_tail, NO_SH);
    }

    #[test]
    fn the_sandbox_labels_say_what_is_enforced() {
        assert_eq!(Sandbox::None.label(), "none");
        let w = Sandbox::RestrictedToken;
        assert!(
            w.label()
                .starts_with("windows-restricted-token (writes only in the check folder")
        );
        assert!(w.label().contains("internet NOT blocked"), "{}", w.label());
        assert!(w.confines_writes() && !w.blocks_internet());
        // A partial run: sh checks outside the sandbox (and say why), the others in it.
        let p = Sandbox::RestrictedTokenPartial;
        assert!(
            p.label().contains("partial: sh checks ran unsandboxed"),
            "{}",
            p.label()
        );
        assert!(p.label().contains("internet NOT blocked"));
        assert_eq!(
            p.for_check(Shell::Sh, false),
            (Sandbox::None, Some(SH_UNSANDBOXED))
        );
        for shell in [Shell::Pwsh, Shell::Cmd] {
            assert_eq!(p.for_check(shell, false), (Sandbox::RestrictedToken, None));
        }
        assert_eq!(w.for_check(Shell::Sh, false), (w, None));
        assert_eq!(
            Sandbox::None.for_check(Shell::Sh, false),
            (Sandbox::None, None)
        );
        // The full sandbox (ADR-0004): every shell runs as the runner account, internet blocked;
        // a network = true check runs as the developer, restricted unless it uses sh.
        let r = Sandbox::WindowsRunner;
        assert!(
            r.label()
                .starts_with("windows-runner (no internet, localhost only")
        );
        assert!(r.confines_writes() && r.blocks_internet());
        for shell in [Shell::Sh, Shell::Pwsh, Shell::Cmd] {
            assert_eq!(r.for_check(shell, false), (r, None));
        }
        assert_eq!(
            r.for_check(Shell::Sh, true),
            (Sandbox::None, Some(SH_NETWORK_UNSANDBOXED))
        );
        assert_eq!(
            r.for_check(Shell::Pwsh, true),
            (Sandbox::RestrictedToken, None)
        );
        if cfg!(windows) {
            assert!(!seatbelt_works());
        }
    }

    #[test]
    fn windows_command_lines_quote_like_the_standard_library() {
        let sh = Path::new(r"C:\Program Files\Git\bin\sh.exe");
        assert_eq!(
            windows_command_line(sh, Shell::Sh, "sh tests/check.sh").unwrap(),
            r#""C:\Program Files\Git\bin\sh.exe" -c "sh tests/check.sh""#
        );
        // Quotes are escaped, backslashes before a quote or the closing quote doubled, others kept.
        assert_eq!(
            windows_command_line(sh, Shell::Sh, r#"echo "a b" C:\x\ end\"#).unwrap(),
            r#""C:\Program Files\Git\bin\sh.exe" -c "echo \"a b\" C:\x\ end\\""#
        );
        assert_eq!(
            windows_command_line(sh, Shell::Sh, r#"a\"b"#).unwrap(),
            r#""C:\Program Files\Git\bin\sh.exe" -c a\\\"b"#
        );
        assert_eq!(
            windows_command_line(sh, Shell::Sh, "").unwrap(),
            r#""C:\Program Files\Git\bin\sh.exe" -c """#
        );
        let ps = Path::new(r"C:\pwsh.exe");
        assert_eq!(
            windows_command_line(ps, Shell::Pwsh, "exit 3").unwrap(),
            r#""C:\pwsh.exe" -NoProfile -NonInteractive -Command "exit 3""#
        );
        // cmd gets the line as written.
        let cmd = Path::new(r"C:\Windows\System32\cmd.exe");
        assert_eq!(
            windows_command_line(cmd, Shell::Cmd, r#"findstr /C:"a b" x >NUL && echo ok"#).unwrap(),
            r#""C:\Windows\System32\cmd.exe" /D /S /C "findstr /C:"a b" x >NUL && echo ok""#
        );
        assert!(windows_command_line(Path::new("a\"b"), Shell::Sh, "x").is_err());
    }

    #[test]
    fn refused_writes_on_windows_name_the_path() {
        // sh (MSYS), Python and PowerShell under the Windows sandbox; cmd says no path.
        let cases = [
            (
                "sh: C:/Users/me/leak.txt: Permission denied",
                Some("C:/Users/me/leak.txt"),
            ),
            (
                r"PermissionError: [Errno 13] Permission denied: 'C:\\Users\\me\\leak.txt'",
                Some(r"C:\\Users\\me\\leak.txt"),
            ),
            (
                r"Out-File: Access to the path 'C:\Users\me\leak.txt' is denied.",
                Some(r"C:\Users\me\leak.txt"),
            ),
            ("Access is denied.", None),
        ];
        for (out, want) in cases {
            assert_eq!(denied_write_path(out, true).as_deref(), want, "{out}");
        }
        assert!(has_bare_denial("Access is denied.", true));
        assert!(!has_bare_denial("Access is denied.", false));
        // A URL is still not a path, and neither is a lone drive.
        assert_eq!(path_in("see https://example.com/x"), None);
        assert_eq!(path_in("drive C: is full"), None);
    }
}
