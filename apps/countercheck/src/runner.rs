//! Run one check command in the snapshot, optionally inside the macOS sandbox.

use countercheck_core::budget;
use countercheck_core::config::PlannedCheck;
use countercheck_core::receipt::CheckResult;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;
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
            Sandbox::None => "none",
            Sandbox::Seatbelt => {
                "macos-seatbelt (no internet, localhost only; writes only in the check folder)"
            }
        }
    }
}

/// True when sandbox-exec can apply a profile here (it can't inside another sandbox).
pub fn seatbelt_works() -> bool {
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

fn seatbelt_profile(workdir: &Path, tmp: &Path) -> String {
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

pub fn run_check(
    spec: &PlannedCheck,
    workdir: &Path,
    tmp: &Path,
    sandbox: Sandbox,
) -> Result<CheckResult, String> {
    let log = tempfile::tempfile_in(tmp).map_err(|e| format!("temp file: {e}"))?;
    let out = log.try_clone().map_err(|e| e.to_string())?;
    let err = log.try_clone().map_err(|e| e.to_string())?;

    let mut cmd = match sandbox {
        Sandbox::None => Command::new("sh"),
        Sandbox::Seatbelt => {
            let mut c = Command::new("/usr/bin/sandbox-exec");
            c.arg("-p").arg(seatbelt_profile(workdir, tmp)).arg("sh");
            c
        }
    };
    cmd.arg("-c")
        .arg(&spec.run)
        .current_dir(workdir)
        .env("COUNTERCHECK", "1")
        .env("TMPDIR", tmp)
        .envs(spec.env.iter().map(|(k, v)| (k, v)))
        .stdin(Stdio::null())
        .stdout(Stdio::from(out))
        .stderr(Stdio::from(err));
    std::os::unix::process::CommandExt::process_group(&mut cmd, 0);

    let started = Instant::now();
    let mut child = cmd
        .spawn()
        .map_err(|e| format!("check \"{}\": {e}", spec.name))?;
    let status = child
        .wait_timeout(Duration::from_secs(spec.timeout_secs))
        .map_err(|e| e.to_string())?;
    let (exit_code, timed_out) = match status {
        Some(s) => (s.code(), false),
        None => {
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
    Ok(result)
}

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
