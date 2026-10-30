//! OpenClaw: `countercheck hook openclaw-stop` with the event Countercheck's OpenClaw plugin
//! relays, the plugin itself run under Node with a stand-in for OpenClaw's plugin API, and
//! `hook install --agent openclaw` (manual setup). Inputs are the fixtures in
//! `tests/fixtures/openclaw/` (built from OpenClaw's types; see the README there).

use std::fs;
use std::path::Path;
use std::process::{Command, Output, Stdio};

mod common;

const BIN: &str = env!("CARGO_BIN_EXE_countercheck");

const FINALIZE: &str = include_str!("fixtures/openclaw/before-agent-finalize.json");
const PLUGIN: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/src/agents/openclaw/index.ts");

fn sh(dir: &Path, cmd: &str) {
    let ok = Command::new(common::sh_program())
        .arg("-c")
        .arg(cmd)
        .current_dir(dir)
        .status()
        .unwrap();
    assert!(ok.success(), "command failed: {cmd}");
}

/// A repository whose one locked check fails at base: `src/calc.sh` subtracts instead of adding.
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

fn fix(dir: &Path) {
    fs::write(dir.join("src/calc.sh"), "echo $(($1 + $2))\n").unwrap();
}

/// The fixture with the project folder and the final message filled in (`None`: no message).
fn payload(cwd: &Path, message: Option<&str>) -> String {
    let mut v: serde_json::Value = serde_json::from_str(FINALIZE).unwrap();
    v["cwd"] = cwd.to_string_lossy().into_owned().into();
    match message {
        Some(m) => v["lastAssistantMessage"] = m.into(),
        None => {
            v.as_object_mut().unwrap().remove("lastAssistantMessage");
        }
    }
    v.to_string()
}

fn with_session(input: &str, session: &str) -> String {
    let mut v: serde_json::Value = serde_json::from_str(input).unwrap();
    v["sessionId"] = session.into();
    v.to_string()
}

/// Run `countercheck hook openclaw-stop` in `dir` with `input` on stdin; (exit code, stdout).
fn openclaw_stop(dir: &Path, input: &str, envs: &[(&str, &str)]) -> (i32, String) {
    use std::io::Write;
    let mut cmd = Command::new(BIN);
    cmd.args(["hook", "openclaw-stop"])
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
    (
        out.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&out.stdout).into_owned(),
    )
}

/// OpenClaw may finish: `{"action":"continue","message":...}`; the message.
fn lets_finish(out: &str) -> String {
    let v: serde_json::Value =
        serde_json::from_str(out.trim()).unwrap_or_else(|_| panic!("not one JSON object: {out}"));
    assert_eq!(v["action"], "continue", "{out}");
    assert_eq!(v.as_object().unwrap().len(), 2, "{out}");
    v["message"].as_str().unwrap().to_string()
}

/// The reason in `{"action":"revise","reason":...}`, or panic.
fn revise_reason(out: &str) -> String {
    let v: serde_json::Value = serde_json::from_str(out.trim()).unwrap();
    assert_eq!(v["action"], "revise", "{out}");
    assert_eq!(v.as_object().unwrap().len(), 2, "{out}");
    v["reason"].as_str().unwrap().to_string()
}

fn last_log_receipt(log: &Path) -> (serde_json::Value, serde_json::Value) {
    let line: serde_json::Value =
        serde_json::from_str(fs::read_to_string(log).unwrap().lines().last().unwrap()).unwrap();
    let receipt =
        serde_json::from_str(&fs::read_to_string(line["receipt"].as_str().unwrap()).unwrap())
            .unwrap();
    (line, receipt)
}

#[test]
fn verified_work_may_finish_and_the_receipt_says_openclaw() {
    let d = project();
    fix(d.path());
    let log = d.path().join("hook.log");
    let (code, out) = openclaw_stop(
        d.path(),
        &payload(d.path(), Some("Done: calc adds now.")),
        &[("COUNTERCHECK_HOOK_LOG", log.to_str().unwrap())],
    );
    assert_eq!(code, 0);
    let msg = lets_finish(&out);
    assert!(
        msg.starts_with("✓ Countercheck: VERIFIED, 1 check(s) passed"),
        "{msg}"
    );
    let (_, receipt) = last_log_receipt(&log);
    assert_eq!(receipt["verdict"], "verified");
    assert_eq!(receipt["worker"]["agent"], "openclaw");
    assert_eq!(receipt["worker"]["model"], "claude-sonnet-4-6");
    assert_eq!(
        receipt["worker"]["session"],
        "8f3c2a1e-5b7d-4e9f-a0c1-2d3e4f5a6b7c"
    );
}

#[test]
fn a_false_success_claim_is_sent_back_with_the_failing_check() {
    let d = project();
    let log = d.path().join("hook.log");
    let (code, out) = openclaw_stop(
        d.path(),
        &payload(d.path(), Some("Fixed it, all tests pass.")),
        &[("COUNTERCHECK_HOOK_LOG", log.to_str().unwrap())],
    );
    assert_eq!(code, 0);
    let reason = revise_reason(&out);
    assert!(
        reason.contains("FAILED") && reason.contains("Failed check \"calc\""),
        "{reason}"
    );
    let (line, receipt) = last_log_receipt(&log);
    assert_eq!(line["nudged"], true);
    assert_eq!(line["gave_up"], false);
    assert_eq!(receipt["verdict"], "failed");
    assert_eq!(receipt["worker"]["agent"], "openclaw");
}

#[test]
fn editing_the_locked_test_is_still_sent_back() {
    let d = project();
    fs::write(d.path().join("tests/check.sh"), "exit 0\n").unwrap();
    let (code, out) = openclaw_stop(d.path(), &payload(d.path(), Some("All tests pass.")), &[]);
    assert_eq!(code, 0);
    assert!(revise_reason(&out).contains("original versions"), "{out}");
}

#[test]
fn an_honest_blocker_may_finish() {
    let d = project();
    let said = "I fixed what I could, but test_export can't pass: it contradicts test_display.";
    let (code, out) = openclaw_stop(d.path(), &payload(d.path(), Some(said)), &[]);
    assert_eq!(code, 0);
    let msg = lets_finish(&out);
    assert!(
        msg.contains("FAILED") && msg.contains("says it is blocked"),
        "{msg}"
    );
}

/// `lastAssistantMessage` is optional in OpenClaw's event: no message is no claim, never a pass.
#[test]
fn no_final_message_and_failing_checks_is_sent_back() {
    let d = project();
    let (code, out) = openclaw_stop(d.path(), &payload(d.path(), None), &[]);
    assert_eq!(code, 0);
    assert!(revise_reason(&out).contains("\"calc\""), "{out}");
}

/// The default limit is 2, below OpenClaw's own 3 revisions: OpenClaw still calls the hook a
/// third time, so the user gets the ⚠ line instead of an unmarked answer.
#[test]
fn after_the_push_back_limit_openclaw_may_finish_with_a_warning() {
    let d = project();
    let log = d.path().join("hook.log");
    let envs = [("COUNTERCHECK_HOOK_LOG", log.to_str().unwrap())];
    let input = payload(d.path(), Some("All tests pass."));
    for _ in 0..2 {
        let (code, out) = openclaw_stop(d.path(), &input, &envs);
        assert_eq!(code, 0);
        revise_reason(&out);
    }
    let (code, out) = openclaw_stop(d.path(), &input, &envs);
    assert_eq!(code, 0);
    let msg = lets_finish(&out);
    assert!(
        msg.starts_with("⚠ Countercheck: FAILED") && msg.contains("did NOT pass"),
        "{msg}"
    );
    let (line, _) = last_log_receipt(&log);
    assert_eq!(line["gave_up"], true);
    // COUNTERCHECK_MAX_NUDGES changes the limit; another session has its own count.
    let other = with_session(&input, "s2");
    let (_, out) = openclaw_stop(d.path(), &other, &[("COUNTERCHECK_MAX_NUDGES", "0")]);
    assert!(lets_finish(&out).contains("did NOT pass"), "{out}");
    let third = with_session(&input, "s3");
    for _ in 0..3 {
        let (_, out) = openclaw_stop(d.path(), &third, &[("COUNTERCHECK_MAX_NUDGES", "3")]);
        revise_reason(&out);
    }
}

#[test]
fn the_stop_hook_fails_open() {
    let d = project();
    let not_git = tempfile::tempdir().unwrap();
    // Garbage on stdin.
    assert_eq!(openclaw_stop(d.path(), "not json", &[]), (0, String::new()));
    // A folder that isn't a git repository (OpenClaw's own workspace, when no cwd is set).
    let input = payload(not_git.path(), Some("Done."));
    assert_eq!(openclaw_stop(d.path(), &input, &[]), (0, String::new()));
    // No cwd: the plugin runs in the Gateway's folder, so that is never taken for the project,
    // even when it is a repository with failing checks.
    let mut v: serde_json::Value = serde_json::from_str(&payload(d.path(), Some("Done."))).unwrap();
    v.as_object_mut().unwrap().remove("cwd");
    assert_eq!(
        openclaw_stop(d.path(), &v.to_string(), &[]),
        (0, String::new())
    );
    // Switched off.
    let input = payload(d.path(), Some("Done."));
    assert_eq!(
        openclaw_stop(d.path(), &input, &[("COUNTERCHECK_HOOK_DISABLED", "1")]),
        (0, String::new())
    );
}

// ---- the plugin, under Node ----

/// Loads the plugin, registers it against a stand-in for OpenClaw's `api` (with `bin`), and calls
/// its `before_agent_finalize` handler with one event; prints what it returned, what it logged and
/// its registration options.
const DRIVER: &str = r#"
import { pathToFileURL } from "node:url";
const [plugin, bin, cwd, message] = process.argv.slice(2);
const mod = await import(pathToFileURL(plugin).href);
let handler, opts;
const logs = [];
mod.default.register({
  pluginConfig: { bin },
  logger: { info: (m) => logs.push(m), warn: (m) => logs.push("warn: " + m) },
  on: (name, h, o) => { if (name === "before_agent_finalize") { handler = h; opts = o; } },
});
const result = await handler(
  { sessionId: "node-1", cwd, stopHookActive: false, model: "m", lastAssistantMessage: message },
  { sessionId: "node-1" },
);
console.log(JSON.stringify({ result: result ?? null, logs, opts }));
"#;

/// Node that runs TypeScript by stripping types (22.6+), or `None` (the test is skipped).
fn node() -> Option<Command> {
    let ok = Command::new("node")
        .args(["--experimental-strip-types", "-e", "0"])
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|s| s.success());
    ok.then(|| {
        let mut c = Command::new("node");
        c.arg("--experimental-strip-types");
        c
    })
}

fn run_plugin(bin: &str, cwd: &Path, message: &str) -> Option<serde_json::Value> {
    let mut node = node()?;
    let driver = tempfile::tempdir().unwrap();
    let path = driver.path().join("driver.mjs");
    fs::write(&path, DRIVER).unwrap();
    if std::env::var_os("COUNTERCHECK").is_some() {
        node.env("COUNTERCHECK_SANDBOX", "none");
    }
    let out: Output = node
        .arg(&path)
        .args([PLUGIN, bin])
        .arg(cwd)
        .arg(message)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    Some(serde_json::from_str(stdout.lines().last().unwrap()).unwrap())
}

#[test]
fn the_plugin_asks_openclaw_for_another_pass_while_the_checks_fail() {
    let d = project();
    let Some(v) = run_plugin(BIN, d.path(), "All tests pass.") else {
        eprintln!("skipped: no node that can run TypeScript");
        return;
    };
    assert_eq!(v["result"]["action"], "revise", "{v}");
    assert!(
        v["result"]["reason"]
            .as_str()
            .unwrap()
            .contains("Failed check \"calc\""),
        "{v}"
    );
    assert_eq!(v["result"].as_object().unwrap().len(), 2, "{v}");
    // Above OpenClaw's 15 s default, so a real check run isn't cut off.
    assert_eq!(v["opts"]["timeoutMs"], 600_000);
    fix(d.path());
    let v = run_plugin(BIN, d.path(), "Done.").unwrap();
    assert!(v["result"].is_null(), "{v}");
    assert!(
        v["logs"][0]
            .as_str()
            .unwrap()
            .starts_with("✓ Countercheck: VERIFIED"),
        "{v}"
    );
}

#[test]
fn the_plugin_fails_open() {
    let d = project();
    // Countercheck missing: the answer goes through.
    let missing = d.path().join("no-such-countercheck");
    let Some(v) = run_plugin(missing.to_str().unwrap(), d.path(), "All tests pass.") else {
        eprintln!("skipped: no node that can run TypeScript");
        return;
    };
    assert!(v["result"].is_null(), "{v}");
    // Not a repository: Countercheck says nothing, so neither does the plugin.
    let not_git = tempfile::tempdir().unwrap();
    let v = run_plugin(BIN, not_git.path(), "All tests pass.").unwrap();
    assert!(v["result"].is_null(), "{v}");
}

// ---- hook install / status ----

fn hook(dir: &Path, args: &[&str], home: &Path) -> Output {
    Command::new(BIN)
        .arg("hook")
        .args(args)
        .current_dir(dir)
        .env("HOME", home)
        .env("USERPROFILE", home)
        .stdin(Stdio::null())
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

/// OpenClaw is set up by hand: install writes nothing, anywhere, and says where the steps are.
#[test]
fn install_points_to_the_manual_setup_and_writes_nothing() {
    let d = project();
    let home = tempfile::tempdir().unwrap();
    for scope in ["project", "user"] {
        let o = hook(
            d.path(),
            &["install", "--agent", "openclaw", "--scope", scope, "-y"],
            home.path(),
        );
        assert!(!o.status.success(), "{}", text(&o));
        assert!(text(&o).contains("docs/hooks/openclaw.md"), "{}", text(&o));
    }
    assert!(!home.path().join(".openclaw").exists());
    assert!(!d.path().join(".openclaw").exists());
    let st = text(&hook(d.path(), &["status"], home.path()));
    assert!(
        st.lines()
            .any(|l| l.starts_with("openclaw ") && l.contains("docs/hooks/openclaw.md")),
        "{st}"
    );
}
