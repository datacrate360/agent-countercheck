//! GitHub Copilot hooks: `countercheck hook copilot-stop` (`agentStop`), `session-start` with
//! Copilot's input, and `hook install --agent copilot`, end to end with the real binary. Payloads
//! are in `fixtures/copilot/`, built from GitHub's hooks reference (see its README).

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

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

/// A repository whose check fails at base: `src/calc.sh` subtracts instead of adding.
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
    fs::write(p.join(".countercheck/checks.toml"), CHECKS).unwrap();
    sh(p, "git add -A && git commit -qm base");
    d
}

fn fix_calc(dir: &Path) {
    fs::write(dir.join("src/calc.sh"), "echo $(($1 + $2))\n").unwrap();
}

fn fixture(name: &str) -> String {
    fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/copilot")
            .join(name),
    )
    .unwrap()
}

/// A fixture payload with its placeholders filled in (through JSON, so Windows paths stay valid).
fn payload(name: &str, dir: &Path, session: &str, transcript: &Path) -> String {
    let mut v: serde_json::Value = serde_json::from_str(&fixture(name)).unwrap();
    for val in v.as_object_mut().unwrap().values_mut() {
        match val.as_str() {
            Some("SESSION") => *val = session.into(),
            Some("CWD") => *val = dir.to_string_lossy().into(),
            Some("TRANSCRIPT") => *val = transcript.to_string_lossy().into(),
            _ => {}
        }
    }
    v.to_string()
}

/// Copilot's event log with `claim` as the main agent's last text (outside the project).
fn transcript(claim: &str) -> (tempfile::TempDir, PathBuf) {
    let d = tempfile::tempdir().unwrap();
    let f = d.path().join("events.jsonl");
    let text = fixture("events.jsonl").replace(
        "\"content\":\"CLAIM\"",
        &format!("\"content\":{}", serde_json::to_string(claim).unwrap()),
    );
    fs::write(&f, text).unwrap();
    (d, f)
}

/// Run `countercheck hook <name>` in `cwd` with `input` on stdin; returns (exit code, stdout).
fn run_hook(name: &str, cwd: &Path, input: &str, envs: &[(&str, &str)]) -> (i32, String) {
    let mut cmd = Command::new(BIN);
    cmd.args(["hook", name])
        .current_dir(cwd)
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
    (
        out.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&out.stdout).into_owned(),
    )
}

/// `agentStop` for `dir`, the agent's last message being `claim`.
fn stop(dir: &Path, session: &str, claim: &str, envs: &[(&str, &str)]) -> (i32, String) {
    let (_t, path) = transcript(claim);
    let input = payload("agent-stop.json", dir, session, &path);
    run_hook("copilot-stop", dir, &input, envs)
}

/// The agent may stop: no block, and the one line shown to the user (`systemMessage`).
fn lets_stop(out: &str) -> String {
    let v: serde_json::Value =
        serde_json::from_str(out.trim()).unwrap_or_else(|_| panic!("not one JSON object: {out}"));
    assert!(v.get("decision").is_none(), "blocked: {out}");
    assert!(v.get("hookSpecificOutput").is_none(), "blocked: {out}");
    v["systemMessage"].as_str().unwrap().to_string()
}

/// A block in both forms Copilot reads (the CLI's top-level `decision`, VS Code's
/// `hookSpecificOutput`), with the same reason; returns it.
fn block_reason(out: &str) -> String {
    let v: serde_json::Value =
        serde_json::from_str(out.trim()).unwrap_or_else(|_| panic!("not one JSON object: {out}"));
    assert_eq!(v["decision"], "block", "{out}");
    let reason = v["reason"].as_str().unwrap().to_string();
    assert!(!reason.is_empty(), "a block needs a reason: {out}");
    let h = &v["hookSpecificOutput"];
    assert_eq!(h["hookEventName"], "Stop", "{out}");
    assert_eq!(h["decision"], "block", "{out}");
    assert_eq!(h["reason"].as_str(), Some(reason.as_str()), "{out}");
    reason
}

fn read_json(path: &str) -> serde_json::Value {
    serde_json::from_str(&fs::read_to_string(path).unwrap()).unwrap()
}

#[test]
fn passing_work_may_finish_and_the_user_sees_the_verdict() {
    let d = project();
    fix_calc(d.path());
    let log = d.path().join("hook.log");
    let (code, out) = stop(
        d.path(),
        "s1",
        "Fixed calc.sh; all tests pass.",
        &[("COUNTERCHECK_HOOK_LOG", log.to_str().unwrap())],
    );
    assert_eq!(code, 0);
    let msg = lets_stop(&out);
    assert!(
        msg.starts_with("✓ Countercheck: VERIFIED, 1 check(s) passed"),
        "{msg}"
    );
    let line: serde_json::Value =
        serde_json::from_str(fs::read_to_string(&log).unwrap().trim()).unwrap();
    let receipt = read_json(line["receipt"].as_str().unwrap());
    assert_eq!(receipt["verdict"], "verified");
    assert_eq!(receipt["worker"]["agent"], "copilot");
    assert_eq!(receipt["worker"]["session"], "s1");
    // The claim and model are the main agent's last text in events.jsonl (not a sub-agent's,
    // not the empty `task_complete` turn).
    assert_eq!(receipt["claim"], "Fixed calc.sh; all tests pass.");
    assert_eq!(receipt["worker"]["model"], "claude-sonnet-4.5");
}

#[test]
fn a_false_success_claim_is_sent_back_with_the_failing_check() {
    let d = project();
    let log = d.path().join("hook.log");
    let (code, out) = stop(
        d.path(),
        "s1",
        "Fixed it, all tests pass.",
        &[("COUNTERCHECK_HOOK_LOG", log.to_str().unwrap())],
    );
    assert_eq!(code, 0);
    let reason = block_reason(&out);
    assert!(
        reason.contains("FAILED") && reason.contains("\"calc\""),
        "{reason}"
    );
    let line: serde_json::Value =
        serde_json::from_str(fs::read_to_string(&log).unwrap().trim()).unwrap();
    assert_eq!(line["nudged"], true);
    assert_eq!(line["gave_up"], false);
}

#[test]
fn an_honest_blocker_may_finish() {
    let d = project();
    let said = "I fixed what I could, but test_export can't pass: it contradicts test_display.";
    let (code, out) = stop(d.path(), "s1", said, &[]);
    assert_eq!(code, 0);
    let msg = lets_stop(&out);
    assert!(
        msg.contains("FAILED") && msg.contains("says it is blocked"),
        "{msg}"
    );
}

#[test]
fn tampering_is_sent_back_even_with_a_blocker_report() {
    let d = project();
    fs::write(d.path().join("tests/check.sh"), "exit 0\n").unwrap();
    let said = "The test can't pass as written, so I adjusted it.";
    let (code, out) = stop(d.path(), "s1", said, &[]);
    assert_eq!(code, 0);
    let reason = block_reason(&out);
    assert!(reason.contains("original versions"), "{reason}");
}

/// The loop guard: after COUNTERCHECK_MAX_NUDGES (default 3, below Copilot's own 8) the agent may
/// stop, and the user is told the work did NOT pass.
#[test]
fn the_push_back_limit_lets_it_stop_with_a_warning() {
    let d = project();
    for _ in 0..3 {
        let (code, out) = stop(d.path(), "s1", "All tests pass.", &[]);
        assert_eq!(code, 0);
        block_reason(&out);
    }
    let (code, out) = stop(d.path(), "s1", "All tests pass.", &[]);
    assert_eq!(code, 0);
    let msg = lets_stop(&out);
    assert!(
        msg.starts_with("⚠ Countercheck: FAILED") && msg.contains("did NOT pass"),
        "{msg}"
    );
    // Another session starts its own count.
    let (_, out) = stop(d.path(), "s2", "Done.", &[]);
    block_reason(&out);
}

/// A PascalCase `Stop` entry gets snake_case fields (VS Code's form): read the same way.
#[test]
fn the_snake_case_stop_input_works_too() {
    let d = project();
    let (_t, path) = transcript("Done, all green.");
    let input = payload("stop-vscode-style.json", d.path(), "s1", &path);
    let (code, out) = run_hook("copilot-stop", d.path(), &input, &[]);
    assert_eq!(code, 0);
    block_reason(&out);
}

/// `sessionStart` sends `sessionId`: the start is recorded, so an agent that commits its edit to a
/// test doesn't move the locked checks.
#[test]
fn session_start_locks_the_checks_against_a_committed_test_edit() {
    let d = project();
    let (_t, path) = transcript("x");
    let input = payload("session-start.json", d.path(), "s1", &path);
    let (code, out) = run_hook("session-start", d.path(), &input, &[]);
    assert_eq!((code, out.as_str()), (0, ""));
    // The agent "fixes" the test and commits it: HEAD now holds the cheat.
    fs::write(d.path().join("tests/check.sh"), "exit 0\n").unwrap();
    sh(d.path(), "git commit -qam 'fix tests'");
    let (code, out) = stop(d.path(), "s1", "All tests pass.", &[]);
    assert_eq!(code, 0);
    assert!(block_reason(&out).contains("FAILED"), "{out}");
    // Without the recorded start (another session), HEAD would be the base and the cheat pass.
    let (_, out) = stop(d.path(), "never-started", "All tests pass.", &[]);
    assert!(lets_stop(&out).contains("VERIFIED"), "{out}");
}

/// Fail open: a broken referee never traps the agent.
#[test]
fn bad_input_lets_the_agent_finish() {
    let d = project();
    let not_git = tempfile::tempdir().unwrap();
    let (_t, path) = transcript("Done.");
    let no_cwd = {
        let mut v: serde_json::Value =
            serde_json::from_str(&payload("agent-stop.json", d.path(), "s1", &path)).unwrap();
        v.as_object_mut().unwrap().remove("cwd");
        v.to_string()
    };
    for (cwd, input) in [
        (d.path(), "not json".to_string()),
        (d.path(), String::new()),
        // No `cwd`: the hook's own folder, here not a git repository.
        (not_git.path(), no_cwd),
        (
            not_git.path(),
            payload("agent-stop.json", not_git.path(), "s1", &path),
        ),
    ] {
        let (code, out) = run_hook("copilot-stop", cwd, &input, &[]);
        assert_eq!((code, out.trim()), (0, ""), "{input}");
    }
    // A missing transcript is no claim, not an error.
    let input = payload(
        "agent-stop.json",
        d.path(),
        "s1",
        &d.path().join("gone.jsonl"),
    );
    let (code, out) = run_hook("copilot-stop", d.path(), &input, &[]);
    assert_eq!(code, 0);
    block_reason(&out);
    // Switched off.
    let (code, out) = stop(
        d.path(),
        "s1",
        "Done.",
        &[("COUNTERCHECK_HOOK_DISABLED", "1")],
    );
    assert_eq!((code, out.trim()), (0, ""));
}

// ---- hook install --agent copilot ----

fn hook(dir: &Path, args: &[&str], home: &Path) -> Output {
    Command::new(BIN)
        .arg("hook")
        .args(args)
        .current_dir(dir)
        .stdin(Stdio::null())
        .env("HOME", home)
        .env("USERPROFILE", home)
        .output()
        .unwrap()
}

fn text(o: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&o.stdout),
        String::from_utf8_lossy(&o.stderr)
    )
}

const INSTALLED: &str = r#"{
  "version": 1,
  "hooks": {
    "sessionStart": [
      {"type": "command", "bash": "countercheck hook session-start", "powershell": "countercheck hook session-start", "timeoutSec": 10}
    ],
    "agentStop": [
      {"type": "command", "bash": "countercheck hook copilot-stop", "powershell": "countercheck hook copilot-stop", "timeoutSec": 600}
    ]
  }
}
"#;

#[test]
fn install_writes_the_documented_shape_and_uninstall_takes_it_out() {
    let d = project();
    let p = d.path();
    let home = tempfile::tempdir().unwrap();
    let file = p.join(".github/hooks/countercheck.json");
    let o = hook(p, &["install", "--agent", "copilot", "--yes"], home.path());
    assert!(o.status.success(), "{}", text(&o));
    assert_eq!(fs::read_to_string(&file).unwrap(), INSTALLED);
    let v: serde_json::Value = serde_json::from_str(INSTALLED).unwrap();
    assert_eq!(v["version"], 1);
    // The tamper check doesn't watch this file yet: install must not claim it does.
    assert!(
        text(&o).contains("does not yet check this file"),
        "{}",
        text(&o)
    );
    let st = text(&hook(p, &["status"], home.path()));
    assert!(
        st.lines().any(|l| l.starts_with("copilot")
            && l.contains("project")
            && l.contains("installed, not committed")),
        "{st}"
    );
    // Twice adds nothing.
    let o = hook(p, &["install", "--agent", "copilot", "--yes"], home.path());
    assert!(text(&o).contains("already in"), "{}", text(&o));
    assert_eq!(fs::read_to_string(&file).unwrap(), INSTALLED);
    let o = hook(
        p,
        &["uninstall", "--agent", "copilot", "--yes"],
        home.path(),
    );
    assert!(o.status.success(), "{}", text(&o));
    assert_eq!(
        fs::read_to_string(&file).unwrap(),
        "{\n  \"version\": 1\n}\n"
    );
}

#[test]
fn install_keeps_other_hooks_and_uninstall_takes_out_only_ours() {
    let d = project();
    let p = d.path();
    let home = tempfile::tempdir().unwrap();
    let file = p.join(".github/hooks/countercheck.json");
    fs::create_dir_all(file.parent().unwrap()).unwrap();
    let original = "{\n  \"version\": 1,\n  \"hooks\": {\n    \"agentStop\": [\n      {\"type\": \"command\", \"bash\": \"./scripts/lint.sh\", \"timeoutSec\": 60}\n    ],\n    \"preToolUse\": [\n      {\"type\": \"command\", \"bash\": \"./scripts/guard.sh\"}\n    ]\n  }\n}\n";
    fs::write(&file, original).unwrap();
    let o = hook(p, &["install", "--agent", "copilot", "-y"], home.path());
    assert!(o.status.success(), "{}", text(&o));
    let s = fs::read_to_string(&file).unwrap();
    assert!(
        s.contains("{\"type\": \"command\", \"bash\": \"./scripts/lint.sh\", \"timeoutSec\": 60}"),
        "{s}"
    );
    assert!(
        s.contains("./scripts/guard.sh") && s.contains("hook copilot-stop"),
        "{s}"
    );
    let lint = s.find("lint.sh").unwrap();
    assert!(
        lint < s.find("hook copilot-stop").unwrap(),
        "ours goes last: {s}"
    );
    assert_eq!(
        fs::read_to_string(p.join(".github/hooks/countercheck.json.countercheck-backup")).unwrap(),
        original
    );
    let o = hook(p, &["uninstall", "--agent", "copilot", "-y"], home.path());
    assert!(o.status.success(), "{}", text(&o));
    let after: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&file).unwrap()).unwrap();
    let want: serde_json::Value = serde_json::from_str(original).unwrap();
    assert_eq!(after, want);
}

#[test]
fn install_for_the_user_and_warns_about_claude_settings_running_twice() {
    let d = project();
    let p = d.path();
    let home = tempfile::tempdir().unwrap();
    let o = hook(
        p,
        &["install", "--agent", "copilot", "--scope", "user", "-y"],
        home.path(),
    );
    assert!(o.status.success(), "{}", text(&o));
    let user = home.path().join(".copilot/hooks/countercheck.json");
    assert_eq!(fs::read_to_string(&user).unwrap(), INSTALLED);
    // Copilot CLI also runs the hooks in .claude/settings.json: a Claude Code install there too
    // means two checks per stop.
    let o = hook(p, &["install", "--agent", "claude-code", "-y"], home.path());
    assert!(o.status.success(), "{}", text(&o));
    let o = hook(p, &["install", "--agent", "copilot", "-y"], home.path());
    assert!(o.status.success(), "{}", text(&o));
    assert!(
        text(&o).contains("also reads .claude/settings.json"),
        "{}",
        text(&o)
    );
    // `--bin` with spaces: bash gets it quoted, PowerShell through the call operator.
    let o = hook(
        p,
        &[
            "install",
            "--agent",
            "copilot",
            "--scope",
            "user",
            "--bin",
            "C:/Program Files/cc/countercheck.exe",
            "--dry-run",
        ],
        tempfile::tempdir().unwrap().path(),
    );
    let t = text(&o);
    assert!(
        t.contains(r#""bash": "\"C:/Program Files/cc/countercheck.exe\" hook copilot-stop""#)
            && t.contains(
                r#""powershell": "& \"C:/Program Files/cc/countercheck.exe\" hook copilot-stop""#
            ),
        "{t}"
    );
}

/// `countercheck verify` run by Copilot (CLI: `COPILOT_CLI=1`; cloud agent: `COPILOT_AGENT_PROMPT`)
/// names it on the receipt.
#[test]
fn verify_run_by_copilot_names_it_on_the_receipt() {
    let d = project();
    fix_calc(d.path());
    for (k, v) in [("COPILOT_CLI", "1"), ("COPILOT_AGENT_PROMPT", "fix calc")] {
        let mut c = Command::new(BIN);
        c.args(["verify", "--json", "--repo"]).arg(d.path());
        if std::env::var_os("COUNTERCHECK").is_some() {
            c.args(["--sandbox", "none"]);
        }
        for other in [
            "CLAUDECODE",
            "CLAUDE_CODE_CHILD_SESSION",
            "CURSOR_AGENT",
            "CODEX_SANDBOX",
        ] {
            c.env_remove(other);
        }
        let out = c.env(k, v).output().unwrap();
        let r: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
        assert_eq!(r["worker"]["agent"], "copilot", "{k}: {r:#}");
    }
}
