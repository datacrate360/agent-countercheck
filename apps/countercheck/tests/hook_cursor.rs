//! Cursor: `countercheck hook cursor-stop` and `hook install --agent cursor`, end to end with the
//! real binary. Payloads come from `tests/fixtures/cursor/` (see its README for where each is from).

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

mod common;

const BIN: &str = env!("CARGO_BIN_EXE_countercheck");
const SESSION: &str = "668320d2-2fd3-4c4d-91f4-1d0c6e1f3c2b";

fn sh(dir: &Path, cmd: &str) {
    let ok = Command::new(common::sh_program())
        .arg("-c")
        .arg(cmd)
        .current_dir(dir)
        .status()
        .unwrap();
    assert!(ok.success(), "command failed: {cmd}");
}

/// A tiny project: src/calc.sh should add two numbers but subtracts; tests/check.sh is protected.
fn project() -> tempfile::TempDir {
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
    fs::write(
        p.join(".countercheck/checks.toml"),
        "protected = [\"tests/**\"]\n[[check]]\nname = \"calc\"\nrun = \"sh tests/check.sh\"\ntimeout_secs = 30\n",
    )
    .unwrap();
    sh(p, "git add -A && git commit -qm base");
    d
}

fn fix(p: &Path) {
    fs::write(p.join("src/calc.sh"), "echo $(($1 + $2))\n").unwrap();
}

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/cursor")
        .join(name)
}

/// A fixture payload with the workspace root and transcript filled in (JSON-escaped, for Windows).
fn payload(name: &str, root: &Path, transcript: &str) -> String {
    let esc = |s: &str| {
        let q = serde_json::to_string(s).unwrap();
        q[1..q.len() - 1].to_string()
    };
    fs::read_to_string(fixture(name))
        .unwrap()
        .replace("{{ROOT}}", &esc(&root.to_string_lossy()))
        .replace(
            "{{TRANSCRIPT}}",
            &esc(&fixture(transcript).to_string_lossy()),
        )
}

struct Run {
    code: i32,
    stdout: String,
    stderr: String,
}

fn run(args: &[&str], dir: &Path, input: &str, envs: &[(&str, &str)]) -> Run {
    use std::io::Write;
    let mut cmd = Command::new(BIN);
    cmd.args(args)
        .current_dir(dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if std::env::var_os("COUNTERCHECK").is_some() {
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
    Run {
        code: out.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
    }
}

/// `hook cursor-stop`, run from somewhere else (user hooks run from `~/.cursor/`): the project
/// comes from `workspace_roots`.
fn stop(input: &str, envs: &[(&str, &str)]) -> Run {
    let elsewhere = tempfile::tempdir().unwrap();
    run(&["hook", "cursor-stop"], elsewhere.path(), input, envs)
}

fn json(out: &str) -> serde_json::Value {
    serde_json::from_str(out.trim()).unwrap_or_else(|_| panic!("not one JSON object: {out}"))
}

/// The agent may stop: `{}` on stdout, the one-line result on stderr.
fn lets_stop(r: &Run) -> &str {
    assert_eq!(r.code, 0, "{}", r.stderr);
    assert_eq!(json(&r.stdout), serde_json::json!({}), "{}", r.stdout);
    &r.stderr
}

#[test]
fn verified_work_may_finish_with_the_result_line() {
    let d = project();
    fix(d.path());
    let r = stop(
        &payload("stop.json", d.path(), "transcript-done.jsonl"),
        &[],
    );
    let msg = lets_stop(&r);
    assert!(
        msg.contains("✓ Countercheck: VERIFIED, 1 check(s) passed"),
        "{msg}"
    );
}

#[test]
fn a_false_success_claim_gets_a_followup_message_naming_the_check() {
    let d = project();
    let log = d.path().join("hook.log");
    let r = stop(
        &payload("stop.json", d.path(), "transcript-done.jsonl"),
        &[("COUNTERCHECK_HOOK_LOG", log.to_str().unwrap())],
    );
    assert_eq!(r.code, 0, "{}", r.stderr);
    let v = json(&r.stdout);
    // Exactly Cursor's shape: one key, a non-empty string.
    let obj = v.as_object().unwrap();
    assert_eq!(obj.keys().collect::<Vec<_>>(), ["followup_message"], "{v}");
    let msg = v["followup_message"].as_str().unwrap();
    assert!(
        msg.contains("FAILED") && msg.contains("Failed check \"calc\""),
        "{msg}"
    );
    let line: serde_json::Value =
        serde_json::from_str(fs::read_to_string(&log).unwrap().trim()).unwrap();
    assert_eq!(line["nudged"], true);
    assert_eq!(line["session_id"], SESSION);
    let receipt: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(line["receipt"].as_str().unwrap()).unwrap())
            .unwrap();
    assert_eq!(receipt["worker"]["agent"], "cursor");
    assert_eq!(receipt["worker"]["model"], "claude-opus-4-7-thinking-max");
}

#[test]
fn an_honest_blocker_in_the_transcript_may_finish() {
    let d = project();
    let r = stop(
        &payload("stop.json", d.path(), "transcript-blocked.jsonl"),
        &[],
    );
    let msg = lets_stop(&r);
    assert!(
        msg.contains("FAILED") && msg.contains("says it is blocked"),
        "{msg}"
    );
}

#[test]
fn the_nudge_limit_lets_it_finish_with_a_warning() {
    let d = project();
    let input = payload("stop.json", d.path(), "transcript-done.jsonl");
    let envs = [("COUNTERCHECK_MAX_NUDGES", "1")];
    let first = stop(&input, &envs);
    assert!(json(&first.stdout).get("followup_message").is_some());
    let second = stop(&input, &envs);
    let msg = lets_stop(&second);
    assert!(
        msg.contains("⚠ Countercheck: FAILED") && msg.contains("push-back limit"),
        "{msg}"
    );
}

#[test]
fn an_aborted_or_failed_turn_is_not_checked() {
    let d = project();
    for status in ["aborted", "error"] {
        let input = payload("stop.json", d.path(), "transcript-done.jsonl")
            .replace("\"completed\"", &format!("\"{status}\""));
        let r = stop(&input, &[]);
        assert!(lets_stop(&r).contains(status), "{}", r.stderr);
    }
    assert!(!d.path().join(".countercheck/receipts").exists());
}

#[test]
fn the_stop_hook_fails_open() {
    let not_git = tempfile::tempdir().unwrap();
    for input in [
        "not json".to_string(),
        // no workspace_roots: the hook's own folder, which isn't a git repository
        serde_json::json!({ "conversation_id": "c", "status": "completed" }).to_string(),
        payload("stop.json", not_git.path(), "transcript-done.jsonl"),
    ] {
        let r = stop(&input, &[]);
        assert_eq!(r.code, 0, "{input}: {}", r.stderr);
        assert!(r.stdout.trim().is_empty(), "{input}: {}", r.stdout);
    }
}

/// `sessionStart` names the project in `workspace_roots`: the checks stay locked to the start even
/// when the agent commits a weakened test.
#[test]
fn session_start_locks_the_checks_from_workspace_roots() {
    let d = project();
    let elsewhere = tempfile::tempdir().unwrap();
    let r = run(
        &["hook", "session-start"],
        elsewhere.path(),
        &payload("session-start.json", d.path(), "transcript-done.jsonl"),
        &[],
    );
    assert_eq!((r.code, r.stdout.as_str()), (0, ""), "{}", r.stderr);
    sh(
        d.path(),
        "echo 'exit 0' > tests/check.sh && git commit -qam 'fix the tests'",
    );
    let r = stop(
        &payload("stop.json", d.path(), "transcript-done.jsonl"),
        &[],
    );
    assert!(
        json(&r.stdout)["followup_message"]
            .as_str()
            .is_some_and(|m| m.contains("FAILED")),
        "{}",
        r.stdout
    );
}

fn hook(dir: &Path, args: &[&str], envs: &[(&str, &str)]) -> Run {
    let mut all = vec!["hook"];
    all.extend_from_slice(args);
    run(&all, dir, "", envs)
}

const INSTALLED: &str = r#"{
  "version": 1,
  "hooks": {
    "sessionStart": [
      {"command": "countercheck hook session-start", "timeout": 10}
    ],
    "stop": [
      {"command": "countercheck hook cursor-stop", "timeout": 600}
    ]
  }
}
"#;

#[test]
fn install_writes_the_documented_shape_into_an_empty_project() {
    let d = project();
    let p = d.path();
    let r = hook(p, &["install", "--agent", "cursor", "--yes"], &[]);
    assert_eq!(r.code, 0, "{}{}", r.stdout, r.stderr);
    assert_eq!(
        fs::read_to_string(p.join(".cursor/hooks.json")).unwrap(),
        INSTALLED
    );
    let again = hook(p, &["install", "--agent", "cursor", "--yes"], &[]);
    assert!(again.stdout.contains("already"), "{}", again.stdout);
    let st = hook(p, &["status"], &[]);
    assert!(
        st.stdout.lines().any(|l| l.starts_with("cursor")
            && l.contains("project")
            && l.contains("installed, not committed")),
        "{}",
        st.stdout
    );
    let r = hook(p, &["uninstall", "--agent", "cursor", "--yes"], &[]);
    assert_eq!(r.code, 0, "{}", r.stderr);
    assert_eq!(
        fs::read_to_string(p.join(".cursor/hooks.json")).unwrap(),
        "{\n  \"version\": 1\n}\n"
    );
}

#[test]
fn install_keeps_other_hooks_as_written() {
    let d = project();
    let p = d.path();
    fs::create_dir_all(p.join(".cursor")).unwrap();
    let original = "{\n  \"version\": 1,\n  \"hooks\": {\n    \"afterFileEdit\": [{ \"command\": \".cursor/hooks/format.sh\" }],\n    \"stop\": [{ \"command\": \".cursor/hooks/audit.sh\", \"loop_limit\": 10 }]\n  }\n}\n";
    fs::write(p.join(".cursor/hooks.json"), original).unwrap();
    let r = hook(p, &["install", "--agent", "cursor", "-y"], &[]);
    assert_eq!(r.code, 0, "{}", r.stderr);
    let s = fs::read_to_string(p.join(".cursor/hooks.json")).unwrap();
    assert!(
        s.contains("[{ \"command\": \".cursor/hooks/format.sh\" }]") && s.contains("audit.sh"),
        "{s}"
    );
    assert!(s.contains("countercheck hook cursor-stop"), "{s}");
    assert!(
        s.find("audit.sh").unwrap() < s.find("cursor-stop").unwrap(),
        "{s}"
    );
    hook(p, &["uninstall", "--agent", "cursor", "-y"], &[]);
    let s = fs::read_to_string(p.join(".cursor/hooks.json")).unwrap();
    assert!(
        s.contains("format.sh") && s.contains("audit.sh") && !s.contains("countercheck"),
        "{s}"
    );
}

#[test]
fn install_warns_when_claude_settings_already_run_the_check() {
    let d = project();
    let p = d.path();
    let r = hook(p, &["install", "--agent", "claude-code", "-y"], &[]);
    assert_eq!(r.code, 0, "{}", r.stderr);
    let r = hook(p, &["install", "--agent", "cursor", "-y"], &[]);
    assert!(
        r.stderr.contains("also reads .claude/settings.json"),
        "{}",
        r.stderr
    );
}

#[test]
fn user_scope_writes_in_the_home_folder() {
    let home = tempfile::tempdir().unwrap();
    let anywhere = tempfile::tempdir().unwrap();
    let h = home.path().to_str().unwrap();
    let r = hook(
        anywhere.path(),
        &["install", "--agent", "cursor", "--scope", "user", "-y"],
        &[("HOME", h), ("USERPROFILE", h)],
    );
    assert_eq!(r.code, 0, "{}", r.stderr);
    assert_eq!(
        fs::read_to_string(home.path().join(".cursor/hooks.json")).unwrap(),
        INSTALLED
    );
}
