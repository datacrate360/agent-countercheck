//! End-to-end tests for the decisions in `hook.rs` that every agent adapter shares, written to kill
//! the mutants `cargo mutants -f apps/countercheck/src/hook.rs` found no test for.

use std::fs;
use std::path::Path;
use std::process::Command;

mod common;

const BIN: &str = env!("CARGO_BIN_EXE_countercheck");

const CHECKS: &str = "protected = [\"tests/**\"]\n[[check]]\nname = \"calc\"\nrun = \"sh tests/check.sh\"\ntimeout_secs = 30\n";

fn sh(dir: &Path, cmd: &str) {
    let ok = Command::new(common::sh_program())
        .arg("-c")
        .arg(cmd)
        .current_dir(dir)
        .status()
        .unwrap();
    assert!(ok.success(), "command failed: {cmd}");
}

/// A git repo with `checks` committed as the base, and a buggy `src/calc.sh` its test catches.
fn project(checks: &str) -> tempfile::TempDir {
    let d = tempfile::tempdir().unwrap();
    let p = d.path();
    sh(p, common::GIT_INIT);
    fs::create_dir_all(p.join("src")).unwrap();
    fs::create_dir_all(p.join("tests")).unwrap();
    fs::create_dir_all(p.join(".countercheck")).unwrap();
    fs::write(p.join("src/calc.sh"), "echo $(($1 - $2))\n").unwrap();
    fs::write(
        p.join("tests/check.sh"),
        "[ \"$(sh src/calc.sh 2 3)\" = 5 ]\n",
    )
    .unwrap();
    fs::write(p.join(".countercheck/checks.toml"), checks).unwrap();
    sh(p, "git add -A && git commit -qm base");
    d
}

/// Inside a countercheck check we already run in a sandbox, and macOS can't nest them.
fn nested() -> bool {
    std::env::var_os("COUNTERCHECK").is_some()
}

/// Run `countercheck hook <name>` with `input` on stdin, in an environment cleared of everything
/// that changes what a hook does (so the suite passes inside an agent's own shell); returns
/// (exit code, stdout, stderr).
fn run_hook(name: &str, input: &str, envs: &[(&str, &str)]) -> (i32, String, String) {
    use std::io::Write;
    const MARKERS: &[&str] = &[
        "COUNTERCHECK_",
        "CLAUDE",
        "CURSOR_",
        "DEVIN_",
        "CODEX_",
        "GOOSE_",
        "JUNIE_",
        "KIRO_",
        "COPILOT_",
        "AMP_",
        "OPENCLAW_",
        "HERMES_",
        "CORTEX_",
    ];
    let mut cmd = Command::new(BIN);
    cmd.args(["hook", name])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    for (k, _) in std::env::vars_os() {
        let k = k.to_string_lossy().to_string();
        if MARKERS.iter().any(|m| k.starts_with(m)) {
            cmd.env_remove(&k);
        }
    }
    if nested() {
        cmd.env("COUNTERCHECK_SANDBOX", "none");
    }
    for (k, v) in envs {
        cmd.env(k, v);
    }
    let mut child = cmd.spawn().unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(input.as_bytes())
        .unwrap();
    let out = child.wait_with_output().unwrap();
    (
        out.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

fn claude_payload(dir: &Path, session: &str, last_message: &str) -> serde_json::Value {
    serde_json::json!({
        "session_id": session,
        "transcript_path": dir.join("no-such-transcript.jsonl"),
        "cwd": dir,
        "hook_event_name": "Stop",
        "stop_hook_active": false,
        "last_assistant_message": last_message,
    })
}

fn hermes_payload(cwd: &Path, changed: &[&Path], said: &str) -> String {
    serde_json::json!({
        "hook_event_name": "pre_verify",
        "cwd": cwd,
        "session_id": "s1",
        "extra": { "final_response": said, "changed_paths": changed, "model": "m" }
    })
    .to_string()
}

/// The `message` of a Hermes `continue`, or panic.
fn hermes_message(out: &str) -> String {
    let v: serde_json::Value =
        serde_json::from_str(out.trim()).unwrap_or_else(|_| panic!("not one JSON object: {out}"));
    assert_eq!(v["action"], "continue", "{out}");
    v["message"].as_str().unwrap().to_string()
}

fn receipts(dir: &Path) -> Vec<serde_json::Value> {
    fs::read_dir(dir.join(".countercheck/receipts"))
        .map(|rd| {
            rd.flatten()
                .filter(|e| e.path().extension().is_some_and(|x| x == "json"))
                .map(|e| serde_json::from_str(&fs::read_to_string(e.path()).unwrap()).unwrap())
                .collect()
        })
        .unwrap_or_default()
}

fn log_lines(log: &Path) -> Vec<serde_json::Value> {
    fs::read_to_string(log)
        .unwrap_or_default()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect()
}

/// The hook reads up to 8 MiB of input: a 2 MiB Stop input (a long final message) is still
/// judged, not dropped as broken JSON (which would fail open and let the agent stop).
#[test]
fn a_large_hook_input_is_still_judged() {
    let d = project(CHECKS);
    let mut payload = claude_payload(d.path(), "s1", "All tests pass.");
    payload["padding"] = serde_json::Value::String("x".repeat(2 * 1024 * 1024));
    let (code, out, err) = run_hook("claude-stop", &payload.to_string(), &[]);
    assert_eq!(code, 0, "{err}");
    let v: serde_json::Value =
        serde_json::from_str(out.trim()).unwrap_or_else(|_| panic!("{out} {err}"));
    assert_eq!(v["decision"], "block", "{out}");
}

/// The hook runs the checks in the platform's sandbox unless `COUNTERCHECK_SANDBOX=none`.
#[test]
fn the_hook_runs_in_the_sandbox_unless_told_not_to() {
    if nested() {
        return;
    }
    let d = project(CHECKS);
    // What `verify` picks on its own on this machine.
    let out = Command::new(BIN)
        .args(["verify", "--json", "--claim", "first", "--repo"])
        .arg(d.path())
        .output()
        .unwrap();
    let auto: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let auto = auto["sandbox"].as_str().unwrap().to_string();
    let hook = |said: &str, envs: &[(&str, &str)]| {
        let before: Vec<String> = receipts(d.path())
            .iter()
            .map(|r| r["id"].as_str().unwrap().to_string())
            .collect();
        run_hook(
            "claude-stop",
            &claude_payload(d.path(), said, said).to_string(),
            envs,
        );
        let new: Vec<serde_json::Value> = receipts(d.path())
            .into_iter()
            .filter(|r| !before.contains(&r["id"].as_str().unwrap().to_string()))
            .collect();
        assert_eq!(new.len(), 1);
        new[0]["sandbox"].as_str().unwrap().to_string()
    };
    assert_eq!(hook("second", &[]), auto);
    assert_eq!(hook("third", &[("COUNTERCHECK_SANDBOX", "none")]), "none");
}

/// One project found from the changed files gets its message as is, not under a `[path]` heading.
#[test]
fn hermes_names_the_project_only_when_there_are_several() {
    let d = project(CHECKS);
    let home = tempfile::tempdir().unwrap();
    let (_, out, _) = run_hook(
        "hermes-pre-verify",
        &hermes_payload(home.path(), &[&d.path().join("src/calc.sh")], "Done."),
        &[],
    );
    let msg = hermes_message(&out);
    assert!(msg.starts_with("Countercheck re-ran"), "{msg}");
}

/// A changed file in a repository without checks doesn't make a second project: the one project
/// is checked once, and its message has no `[path]` heading.
#[test]
fn hermes_skips_a_repository_without_checks() {
    let d = project(CHECKS);
    let other = tempfile::tempdir().unwrap();
    sh(
        other.path(),
        &format!(
            "{} && echo x > notes.txt && git add -A && git commit -qm x",
            common::GIT_INIT
        ),
    );
    let home = tempfile::tempdir().unwrap();
    let log = home.path().join("hook.log");
    let (_, out, _) = run_hook(
        "hermes-pre-verify",
        &hermes_payload(
            home.path(),
            &[
                &other.path().join("notes.txt"),
                &d.path().join("src/calc.sh"),
            ],
            "Done.",
        ),
        &[("COUNTERCHECK_HOOK_LOG", log.to_str().unwrap())],
    );
    let msg = hermes_message(&out);
    assert!(msg.starts_with("Countercheck re-ran"), "{msg}");
    assert_eq!(log_lines(&log).len(), 1);
    assert!(receipts(other.path()).is_empty());
}

/// The same repository reached through a symlinked path (two folders of it) is checked once.
#[cfg(unix)]
#[test]
fn hermes_checks_a_project_once_however_its_files_are_named() {
    let d = project(CHECKS);
    let home = tempfile::tempdir().unwrap();
    let link = home.path().join("link");
    std::os::unix::fs::symlink(d.path(), &link).unwrap();
    let log = home.path().join("hook.log");
    let (_, out, _) = run_hook(
        "hermes-pre-verify",
        &hermes_payload(
            home.path(),
            &[&link.join("src/calc.sh"), &link.join("tests/check.sh")],
            "Done.",
        ),
        &[("COUNTERCHECK_HOOK_LOG", log.to_str().unwrap())],
    );
    hermes_message(&out);
    assert_eq!(log_lines(&log).len(), 1, "{out}");
}

/// A project whose checks can't be read: alone, the error is the hook's own (it fails open); next
/// to another project, it is skipped and the other project's push-back still gets through.
#[test]
fn hermes_reports_a_broken_project_without_hiding_another() {
    let broken = project("not [ toml\n");
    let failing = project(CHECKS);
    let home = tempfile::tempdir().unwrap();
    let (code, out, err) = run_hook(
        "hermes-pre-verify",
        &hermes_payload(home.path(), &[&broken.path().join("src/calc.sh")], "Done."),
        &[],
    );
    assert_eq!((code, out.trim()), (0, ""));
    assert!(err.starts_with("countercheck hook: "), "{err}");
    let (code, out, err) = run_hook(
        "hermes-pre-verify",
        &hermes_payload(
            home.path(),
            &[
                &broken.path().join("src/calc.sh"),
                &failing.path().join("src/calc.sh"),
            ],
            "Done.",
        ),
        &[],
    );
    assert_eq!(code, 0);
    let msg = hermes_message(&out);
    let name = failing
        .path()
        .file_name()
        .unwrap()
        .to_string_lossy()
        .to_string();
    assert!(msg.contains(&name) && msg.contains("FAILED"), "{msg}");
    assert!(err.contains("not valid"), "{err}");
}

/// A check whose output (`out.txt`, written by the worker) is its whole failure.
const ECHO_CHECKS: &str =
    "[[check]]\nname = \"c\"\nrun = \"cat out.txt; exit 1\"\ntimeout_secs = 30\n";

/// Two projects' push-backs together are cut to the message limit (2000 characters).
#[test]
fn hermes_cuts_a_long_message_from_several_projects() {
    let a = project(ECHO_CHECKS);
    let b = project(ECHO_CHECKS);
    for p in [&a, &b] {
        fs::write(p.path().join("out.txt"), "y".repeat(1500)).unwrap();
    }
    let home = tempfile::tempdir().unwrap();
    let (_, out, _) = run_hook(
        "hermes-pre-verify",
        &hermes_payload(
            home.path(),
            &[&a.path().join("out.txt"), &b.path().join("out.txt")],
            "Done.",
        ),
        &[],
    );
    let msg = hermes_message(&out);
    assert!(msg.ends_with("\n[...truncated]"), "{msg}");
    assert_eq!(msg.chars().count(), 1980 + "\n[...truncated]".len());
}

/// A message of exactly 2000 characters is kept whole; a shorter one gets no cut mark.
#[test]
fn hermes_keeps_a_message_at_the_limit_whole() {
    let d = project(ECHO_CHECKS);
    let home = tempfile::tempdir().unwrap();
    let message = |n: usize, said: &str| {
        fs::write(d.path().join("out.txt"), "y".repeat(n)).unwrap();
        let (_, out, _) = run_hook(
            "hermes-pre-verify",
            &hermes_payload(home.path(), &[&d.path().join("out.txt")], said),
            &[],
        );
        hermes_message(&out)
    };
    let short = message(1, "Done.");
    assert!(!short.contains("[...truncated]"), "{short}");
    // Grow the output until the message is exactly 2000 characters. The rest of the message (check
    // time, notes) can differ between runs, so measure each run instead of assuming it stays the same.
    let mut n = 2000 - short.chars().count() + 1;
    let mut full = message(n, "Done.");
    for _ in 0..8 {
        let len = full.chars().count();
        if full.contains("[...truncated]") {
            n = n.saturating_sub(100); // overshot: the cut hides the length, so step back
        } else if len < 2000 {
            n += 2000 - len;
        } else {
            break;
        }
        full = message(n, "Done.");
    }
    assert_eq!(full.chars().count(), 2000, "{full}");
    assert!(!full.contains("[...truncated]"), "{full}");
}

/// A Stop with an empty `transcript_path` takes the app the session's start recorded.
#[test]
fn an_empty_transcript_path_falls_back_to_the_recorded_agent() {
    let d = project(CHECKS);
    let start = serde_json::json!({
        "session_id": "c1", "cwd": d.path(), "hook_event_name": "SessionStart",
        "transcript_path": "/Users/x/.snowflake/cortex/conversations/c1.json"
    });
    let (code, _, err) = run_hook("session-start", &start.to_string(), &[]);
    assert_eq!(code, 0, "{err}");
    let mut stop = claude_payload(d.path(), "c1", "Done.");
    stop["transcript_path"] = serde_json::Value::String(String::new());
    run_hook("claude-stop", &stop.to_string(), &[]);
    let r = receipts(d.path());
    assert_eq!(r.len(), 1);
    assert_eq!(r[0]["worker"]["agent"], "cortex-code", "{:#}", r[0]);
}

/// The agent SessionStart records for the session, read from `.git/countercheck/sessions/`.
fn recorded_agent(dir: &Path, session: &str) -> serde_json::Value {
    let file = dir
        .join(".git/countercheck/sessions")
        .join(format!("{session}.json"));
    let v: serde_json::Value = serde_json::from_str(&fs::read_to_string(file).unwrap()).unwrap();
    v["agent"].clone()
}

/// Copilot is told by `sessionId` *without* `session_id`; an input with both is not Copilot.
#[test]
fn session_start_names_copilot_only_without_session_id() {
    let d = project(CHECKS);
    let start = serde_json::json!({
        "session_id": "both", "sessionId": "both", "cwd": d.path(),
        "hook_event_name": "SessionStart",
    });
    run_hook("session-start", &start.to_string(), &[]);
    assert_eq!(recorded_agent(d.path(), "both"), "claude-code");
    let start = serde_json::json!({ "sessionId": "cp", "cwd": d.path() });
    run_hook("session-start", &start.to_string(), &[]);
    assert_eq!(recorded_agent(d.path(), "cp"), "copilot");
}

/// Only a full commit id recorded at the session's start locks the base: an abbreviated one (or a
/// ref name) is not trusted, and the stop falls back to HEAD.
#[test]
fn a_recorded_base_must_be_a_full_commit_id() {
    let d = project(CHECKS);
    let start = serde_json::json!({
        "session_id": "s1", "cwd": d.path(), "hook_event_name": "SessionStart",
    });
    run_hook("session-start", &start.to_string(), &[]);
    let file = d.path().join(".git/countercheck/sessions/s1.json");
    let mut rec: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&file).unwrap()).unwrap();
    let full = rec["base"].as_str().unwrap().to_string();
    assert_eq!(full.len(), 40);
    let stop = |said: &str| {
        run_hook(
            "claude-stop",
            &claude_payload(d.path(), "s1", said).to_string(),
            &[],
        );
        let mut r = receipts(d.path());
        r.sort_by_key(|x| x["created_at"].as_str().unwrap().to_string());
        r.into_iter()
            .find(|x| x["claim"] == said)
            .unwrap_or_else(|| panic!("no receipt for {said}"))["repo"]["base_rev"]
            .clone()
    };
    assert_eq!(stop("first"), full.as_str());
    for short in [&full[..7], "HEAD"] {
        rec["base"] = serde_json::Value::String(short.to_string());
        fs::write(&file, rec.to_string()).unwrap();
        assert_eq!(stop(&format!("with {short}")), "HEAD", "{short}");
    }
}
