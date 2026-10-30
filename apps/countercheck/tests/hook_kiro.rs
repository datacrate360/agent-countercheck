//! Kiro: `countercheck hook kiro-stop` and `hook install --agent kiro`, end to end with the real
//! binary. Payloads are Kiro's documented examples (`tests/fixtures/kiro/`).

use std::fs;
use std::path::Path;
use std::process::{Command, Output, Stdio};

mod common;

const BIN: &str = env!("CARGO_BIN_EXE_countercheck");

fn sh(dir: &Path, cmd: &str) {
    let ok = Command::new(common::sh_program())
        .arg("-c")
        .arg(cmd)
        .current_dir(dir)
        .status()
        .unwrap();
    assert!(ok.success(), "command failed: {cmd}");
}

/// A tiny project: src/calc.sh adds two numbers (wrongly at base), tests/check.sh checks it.
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

/// A documented payload from `tests/fixtures/kiro/`, with `cwd` set to `dir` and extra fields.
fn fixture(name: &str, dir: &Path, extra: &[(&str, &str)]) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/kiro")
        .join(name);
    let mut v: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(path).unwrap()).unwrap();
    v["cwd"] = serde_json::json!(dir);
    for (k, val) in extra {
        v[*k] = serde_json::json!(val);
    }
    v.to_string()
}

/// Run `countercheck hook <name>` in `cwd` with `input` on stdin; returns (exit code, stdout).
fn run_hook(name: &str, cwd: &Path, input: &str, envs: &[(&str, &str)]) -> (i32, String) {
    use std::io::Write;
    let mut cmd = Command::new(BIN);
    cmd.args(["hook", name])
        .current_dir(cwd)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    // Inside a Countercheck check we already run in a sandbox, and macOS can't nest them.
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

fn kiro_stop(dir: &Path, input: &str, envs: &[(&str, &str)]) -> (i32, String) {
    run_hook("kiro-stop", dir, input, envs)
}

/// Kiro's "keep working" answer: the documented `{"decision":"block","reason":...}`, nothing else.
fn block_reason(out: &str) -> String {
    let v: serde_json::Value =
        serde_json::from_str(out.trim()).unwrap_or_else(|_| panic!("not one JSON object: {out}"));
    let doc: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(
            Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/kiro/block.json"),
        )
        .unwrap(),
    )
    .unwrap();
    let keys =
        |v: &serde_json::Value| -> Vec<String> { v.as_object().unwrap().keys().cloned().collect() };
    assert_eq!(keys(&v), keys(&doc), "{out}");
    assert_eq!(v["decision"], "block", "{out}");
    v["reason"].as_str().unwrap().to_string()
}

/// The agent may stop: one plain line for the user, never a block.
fn lets_stop(out: &str) -> String {
    assert!(!out.contains("\"decision\""), "blocked: {out}");
    assert_eq!(out.trim().lines().count(), 1, "{out}");
    out.trim().to_string()
}

#[test]
fn verified_work_may_finish_with_a_line_for_the_user() {
    let d = project();
    fix(d.path());
    let (code, out) = kiro_stop(d.path(), &fixture("stop.json", d.path(), &[]), &[]);
    assert_eq!(code, 0);
    let msg = lets_stop(&out);
    assert!(
        msg.starts_with("✓ Countercheck: VERIFIED, 1 check(s) passed"),
        "{msg}"
    );
}

#[test]
fn failed_work_is_sent_back_with_the_failing_check_and_the_receipt_names_kiro() {
    let d = project();
    let log = d.path().join("hook.log");
    let (code, out) = kiro_stop(
        d.path(),
        &fixture("stop.json", d.path(), &[]),
        &[("COUNTERCHECK_HOOK_LOG", log.to_str().unwrap())],
    );
    assert_eq!(
        code, 0,
        "exit 1 would also start a turn; the hook only exits 0"
    );
    let reason = block_reason(&out);
    assert!(
        reason.contains("FAILED") && reason.contains("\"calc\""),
        "{reason}"
    );
    let line: serde_json::Value =
        serde_json::from_str(fs::read_to_string(&log).unwrap().trim()).unwrap();
    assert_eq!(line["nudged"], true);
    assert_eq!(line["gave_up"], false);
    assert_eq!(line["session_id"], "abc123-def456-789");
    let receipt: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(line["receipt"].as_str().unwrap()).unwrap())
            .unwrap();
    assert_eq!(receipt["worker"]["agent"], "kiro");
    assert_eq!(receipt["worker"]["session"], "abc123-def456-789");
}

/// Inferred field: `assistant_response`. A success claim on failed work is sent back.
#[test]
fn a_false_success_claim_is_sent_back() {
    let d = project();
    let input = fixture(
        "stop.json",
        d.path(),
        &[("assistant_response", "Fixed it, all tests pass.")],
    );
    let (code, out) = kiro_stop(d.path(), &input, &[]);
    assert_eq!(code, 0);
    assert!(block_reason(&out).contains("\"calc\""), "{out}");
}

/// Inferred field: `assistant_response`. An agent that says it is blocked may stop; the FAILED
/// receipt is kept and the user is told.
#[test]
fn an_honest_blocker_may_finish() {
    let d = project();
    let said = "I fixed what I could, but test_export can't pass: it contradicts test_display.";
    let input = fixture("stop.json", d.path(), &[("assistant_response", said)]);
    let (code, out) = kiro_stop(d.path(), &input, &[]);
    assert_eq!(code, 0);
    let msg = lets_stop(&out);
    assert!(
        msg.contains("FAILED") && msg.contains("says it is blocked"),
        "{msg}"
    );
}

#[test]
fn after_the_nudge_limit_the_agent_may_finish_with_a_warning() {
    let d = project();
    let input = fixture("stop.json", d.path(), &[]);
    let envs = [("COUNTERCHECK_MAX_NUDGES", "2")];
    for _ in 0..2 {
        let (code, out) = kiro_stop(d.path(), &input, &envs);
        assert_eq!(code, 0);
        block_reason(&out);
    }
    let (code, out) = kiro_stop(d.path(), &input, &envs);
    assert_eq!(code, 0);
    let msg = lets_stop(&out);
    assert!(
        msg.starts_with("⚠ Countercheck: FAILED") && msg.contains("did NOT pass"),
        "{msg}"
    );
}

#[test]
fn the_stop_hook_fails_open() {
    let d = project();
    let elsewhere = tempfile::tempdir().unwrap(); // not a git repository
    let cases = [
        ("not json".to_string(), d.path()),
        // no cwd: Kiro runs the command in the project root, so that is used
        (
            r#"{"hook_event_name":"Stop","session_id":"x"}"#.to_string(),
            elsewhere.path(),
        ),
        (fixture("stop.json", elsewhere.path(), &[]), d.path()),
    ];
    for (input, cwd) in &cases {
        let (code, out) = kiro_stop(cwd, input, &[]);
        assert_eq!((code, out.trim()), (0, ""), "{input}");
    }
    // Switched off.
    let (code, out) = kiro_stop(
        d.path(),
        &fixture("stop.json", d.path(), &[]),
        &[("COUNTERCHECK_HOOK_DISABLED", "1")],
    );
    assert_eq!((code, out.trim()), (0, ""));
}

/// Without `cwd`, the hook checks the folder Kiro runs it in (the project root).
#[test]
fn without_cwd_the_project_root_is_checked() {
    let d = project();
    let (code, out) = kiro_stop(
        d.path(),
        r#"{"hook_event_name":"Stop","session_id":"x"}"#,
        &[],
    );
    assert_eq!(code, 0);
    block_reason(&out);
}

/// Kiro's session-start event locks the checks: committing an edited test later changes nothing.
#[test]
fn session_start_locks_the_checks_to_where_the_session_began() {
    let d = project();
    let p = d.path();
    for event in ["agentSpawn", "SessionStart"] {
        let input = fixture("agent-spawn.json", p, &[("hook_event_name", event)]);
        let (code, out) = run_hook("session-start", p, &input, &[]);
        assert_eq!((code, out.as_str()), (0, ""));
    }
    fs::write(p.join("tests/check.sh"), "exit 0\n").unwrap();
    sh(p, "git commit -qam 'relax the test'");
    let (code, out) = kiro_stop(p, &fixture("stop.json", p, &[]), &[]);
    assert_eq!(code, 0);
    assert!(block_reason(&out).contains("\"calc\""), "{out}");
}

// ---- hook install --agent kiro ----

/// Run `countercheck hook <args>` in `dir`, with stdin closed (no terminal to ask on).
fn hook(dir: &Path, args: &[&str], envs: &[(&str, &Path)]) -> Output {
    let mut c = Command::new(BIN);
    c.arg("hook")
        .args(args)
        .current_dir(dir)
        .stdin(Stdio::null());
    for (k, v) in envs {
        c.env(k, v);
    }
    c.output().unwrap()
}

fn text(o: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&o.stdout),
        String::from_utf8_lossy(&o.stderr)
    )
}

const FILE: &str = ".kiro/hooks/countercheck.json";

#[test]
fn install_writes_the_documented_shape_into_an_empty_project() {
    let d = project();
    let p = d.path();
    let o = hook(p, &["install", "--agent", "kiro", "--yes"], &[]);
    assert!(o.status.success(), "{}", text(&o));
    assert!(text(&o).contains("Agent Hooks panel"), "{}", text(&o));
    let v: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(p.join(FILE)).unwrap()).unwrap();
    assert_eq!(
        v,
        serde_json::json!({
            "version": "v1",
            "hooks": [
                {"name": "countercheck-session-start", "trigger": "SessionStart",
                 "action": {"type": "command", "command": "countercheck hook session-start"},
                 "timeout": 10},
                {"name": "countercheck-stop", "trigger": "Stop",
                 "action": {"type": "command", "command": "countercheck hook kiro-stop"},
                 "timeout": 600}
            ]
        })
    );
    // Keys in the order Kiro's docs show them.
    let s = fs::read_to_string(p.join(FILE)).unwrap();
    let at = |k: &str| s.find(k).unwrap();
    assert!(at("\"version\"") < at("\"hooks\""), "{s}");
    assert!(
        at("\"name\"") < at("\"trigger\"")
            && at("\"trigger\"") < at("\"action\"")
            && at("\"action\"") < at("\"timeout\""),
        "{s}"
    );
    let st = text(&hook(p, &["status"], &[]));
    assert!(
        st.lines()
            .any(|l| l.starts_with("kiro") && l.contains("project") && l.contains("installed")),
        "{st}"
    );
}

#[test]
fn other_hooks_stay_as_written_a_second_install_adds_nothing_uninstall_takes_only_ours() {
    let d = project();
    let p = d.path();
    fs::create_dir_all(p.join(".kiro/hooks")).unwrap();
    let lint = "{\"version\": \"v1\", \"hooks\": [{\"name\": \"lint\", \"trigger\": \"Stop\", \"action\": {\"type\": \"command\", \"command\": \"npm run lint\"}}]}\n";
    fs::write(p.join(".kiro/hooks/lint.json"), lint).unwrap();
    let original = "{\n  \"version\": \"v1\",\n  \"hooks\": [\n    {\"name\": \"say\", \"trigger\": \"Stop\", \"action\": {\"type\": \"command\", \"command\": \"say done\"}, \"enabled\": false}\n  ]\n}\n";
    fs::write(p.join(FILE), original).unwrap();
    let o = hook(p, &["install", "--agent", "kiro", "-y"], &[]);
    assert!(o.status.success(), "{}", text(&o));
    let s = fs::read_to_string(p.join(FILE)).unwrap();
    assert!(
        s.starts_with("{\n  \"version\": \"v1\",\n  \"hooks\": [\n    {\"name\": \"say\", \"trigger\": \"Stop\", \"action\": {\"type\": \"command\", \"command\": \"say done\"}, \"enabled\": false},\n"),
        "{s}"
    );
    assert!(
        s.contains("hook kiro-stop") && s.contains("hook session-start"),
        "{s}"
    );
    assert_eq!(
        fs::read_to_string(p.join(".kiro/hooks/countercheck.json.countercheck-backup")).unwrap(),
        original
    );
    let o = hook(p, &["install", "--agent", "kiro", "-y"], &[]);
    assert!(text(&o).contains("already"), "{}", text(&o));
    let o = hook(p, &["uninstall", "--agent", "kiro", "-y"], &[]);
    assert!(o.status.success(), "{}", text(&o));
    assert_eq!(fs::read_to_string(p.join(FILE)).unwrap(), original);
    assert_eq!(
        fs::read_to_string(p.join(".kiro/hooks/lint.json")).unwrap(),
        lint
    );
}

#[test]
fn user_scope_writes_in_the_home_folder() {
    let home = tempfile::tempdir().unwrap();
    let anywhere = tempfile::tempdir().unwrap();
    let envs = [("HOME", home.path()), ("USERPROFILE", home.path())];
    let o = hook(
        anywhere.path(),
        &["install", "--agent", "kiro", "--scope", "user", "-y"],
        &envs,
    );
    assert!(o.status.success(), "{}", text(&o));
    let s = fs::read_to_string(home.path().join(FILE)).unwrap();
    assert!(s.contains("hook kiro-stop"), "{s}");
}

#[test]
fn managed_scope_is_claude_code_only() {
    let d = project();
    let o = hook(
        d.path(),
        &["install", "--agent", "kiro", "--scope", "managed", "-y"],
        &[],
    );
    assert!(!o.status.success());
    assert!(
        text(&o).contains("Claude Code's managed settings"),
        "{}",
        text(&o)
    );
}
