//! Devin CLI: `countercheck hook devin-stop`, `hook session-start` with Devin's input, `hook install
//! --agent devin` and the tamper check on `.devin/hooks.v1.json`, end to end with the real binary.
//! Inputs are the fixtures in `tests/fixtures/devin/` (built from Devin's docs and binary; see the
//! README there).

use std::fs;
use std::path::Path;
use std::process::{Command, Output, Stdio};

mod common;

const BIN: &str = env!("CARGO_BIN_EXE_countercheck");

/// The documented `Stop` fields only: no `cwd`, no final message.
const STOP: &str = include_str!("fixtures/devin/stop.json");
/// `STOP` plus the fields the binary's hook input type names (`cwd`, `model`,
/// `last_assistant_message`).
const STOP_FULL: &str = include_str!("fixtures/devin/stop-full.json");
const SESSION_START: &str = include_str!("fixtures/devin/session-start.json");

const SESSION: &str = "3f8d1c2a-6b4e-4c1d-9a2f-7e5b0c9d8a14";
const FILE: &str = ".devin/hooks.v1.json";

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

/// `STOP_FULL` (or another fixture) with the project folder and the final message filled in.
fn payload(fixture: &str, cwd: &Path, message: Option<&str>) -> String {
    let mut v: serde_json::Value = serde_json::from_str(fixture).unwrap();
    if v.get("cwd").is_some() {
        v["cwd"] = cwd.to_string_lossy().into_owned().into();
    }
    if v.get("last_assistant_message").is_some() {
        v["last_assistant_message"] = message.map_or(serde_json::Value::Null, Into::into);
    }
    v.to_string()
}

fn with_session(input: &str, session: &str) -> String {
    let mut v: serde_json::Value = serde_json::from_str(input).unwrap();
    v["session_id"] = session.into();
    v.to_string()
}

/// Run `countercheck hook <name>` in `dir` with `input` on stdin; (exit code, stdout).
fn run_hook(name: &str, dir: &Path, input: &str, envs: &[(&str, &str)]) -> (i32, String) {
    use std::io::Write;
    let mut cmd = Command::new(BIN);
    cmd.args(["hook", name])
        .current_dir(dir)
        .env_remove("DEVIN_PROJECT_DIR")
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

fn devin_stop(dir: &Path, input: &str, envs: &[(&str, &str)]) -> (i32, String) {
    run_hook("devin-stop", dir, input, envs)
}

/// Devin may finish: no block, and the line for the user (`systemMessage`).
fn lets_stop(out: &str) -> String {
    let v: serde_json::Value =
        serde_json::from_str(out.trim()).unwrap_or_else(|_| panic!("not one JSON object: {out}"));
    assert!(v.get("decision").is_none(), "blocked: {out}");
    assert_eq!(v.as_object().unwrap().len(), 1, "only systemMessage: {out}");
    v["systemMessage"].as_str().unwrap().to_string()
}

/// The reason in Devin's `{"decision":"block","reason":...}`, or panic.
fn block_reason(out: &str) -> String {
    let v: serde_json::Value = serde_json::from_str(out.trim()).unwrap();
    assert_eq!(v["decision"], "block", "{out}");
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

/// Only the documented fields: the project comes from `DEVIN_PROJECT_DIR`, though the hook runs in
/// another folder.
#[test]
fn verified_work_may_finish_and_the_receipt_says_devin() {
    let d = project();
    fix(d.path());
    let elsewhere = tempfile::tempdir().unwrap();
    let log = d.path().join("hook.log");
    let (code, out) = devin_stop(
        elsewhere.path(),
        STOP,
        &[
            ("DEVIN_PROJECT_DIR", d.path().to_str().unwrap()),
            ("COUNTERCHECK_HOOK_LOG", log.to_str().unwrap()),
        ],
    );
    assert_eq!(code, 0);
    let msg = lets_stop(&out);
    assert!(
        msg.starts_with("✓ Countercheck: VERIFIED, 1 check(s) passed"),
        "{msg}"
    );
    let (_, receipt) = last_log_receipt(&log);
    assert_eq!(receipt["verdict"], "verified");
    assert_eq!(receipt["worker"]["agent"], "devin");
    assert_eq!(receipt["worker"]["session"], SESSION);
}

/// `cwd` and `model` in the input win, when Devin sends them.
#[test]
fn the_inputs_cwd_and_model_are_used_when_present() {
    let d = project();
    fix(d.path());
    let not_git = tempfile::tempdir().unwrap();
    let log = d.path().join("hook.log");
    let (code, out) = devin_stop(
        not_git.path(),
        &payload(STOP_FULL, d.path(), Some("Done: calc adds now.")),
        &[
            ("DEVIN_PROJECT_DIR", not_git.path().to_str().unwrap()),
            ("COUNTERCHECK_HOOK_LOG", log.to_str().unwrap()),
        ],
    );
    assert_eq!(code, 0);
    assert!(lets_stop(&out).contains("VERIFIED"), "{out}");
    let (_, receipt) = last_log_receipt(&log);
    assert_eq!(receipt["worker"]["agent"], "devin");
    assert_eq!(receipt["worker"]["model"], "swe-1.5");
}

#[test]
fn a_false_success_claim_is_sent_back_with_the_failing_check() {
    let d = project();
    let log = d.path().join("hook.log");
    let (code, out) = devin_stop(
        d.path(),
        &payload(STOP_FULL, d.path(), Some("Fixed it, all tests pass.")),
        &[("COUNTERCHECK_HOOK_LOG", log.to_str().unwrap())],
    );
    assert_eq!(code, 0);
    let reason = block_reason(&out);
    assert!(
        reason.contains("FAILED") && reason.contains("Failed check \"calc\""),
        "{reason}"
    );
    let (line, receipt) = last_log_receipt(&log);
    assert_eq!(line["nudged"], true);
    assert_eq!(line["gave_up"], false);
    assert_eq!(receipt["verdict"], "failed");
    assert_eq!(receipt["worker"]["agent"], "devin");
}

#[test]
fn editing_the_locked_test_is_still_sent_back() {
    let d = project();
    fs::write(d.path().join("tests/check.sh"), "exit 0\n").unwrap();
    let (code, out) = devin_stop(
        d.path(),
        &payload(STOP_FULL, d.path(), Some("All tests pass.")),
        &[],
    );
    assert_eq!(code, 0);
    assert!(block_reason(&out).contains("original versions"), "{out}");
}

#[test]
fn an_honest_blocker_may_finish() {
    let d = project();
    let said = "I fixed what I could, but test_export can't pass: it contradicts test_display.";
    let (code, out) = devin_stop(d.path(), &payload(STOP_FULL, d.path(), Some(said)), &[]);
    assert_eq!(code, 0);
    let msg = lets_stop(&out);
    assert!(
        msg.contains("FAILED") && msg.contains("says it is blocked"),
        "{msg}"
    );
}

/// The documented input has no final message: no claim, never a pass.
#[test]
fn no_final_message_and_failing_checks_is_sent_back() {
    let d = project();
    let (code, out) = devin_stop(
        d.path(),
        STOP,
        &[("DEVIN_PROJECT_DIR", d.path().to_str().unwrap())],
    );
    assert_eq!(code, 0);
    assert!(block_reason(&out).contains("\"calc\""), "{out}");
}

#[test]
fn after_the_push_back_limit_devin_may_finish_with_a_warning() {
    let d = project();
    let log = d.path().join("hook.log");
    let envs = [("COUNTERCHECK_HOOK_LOG", log.to_str().unwrap())];
    let input = payload(STOP_FULL, d.path(), Some("All tests pass."));
    for _ in 0..3 {
        let (code, out) = devin_stop(d.path(), &input, &envs);
        assert_eq!(code, 0);
        block_reason(&out);
    }
    let (code, out) = devin_stop(d.path(), &input, &envs);
    assert_eq!(code, 0);
    let msg = lets_stop(&out);
    assert!(
        msg.starts_with("⚠ Countercheck: FAILED") && msg.contains("did NOT pass"),
        "{msg}"
    );
    let (line, _) = last_log_receipt(&log);
    assert_eq!(line["gave_up"], true);
    // COUNTERCHECK_MAX_NUDGES changes the limit; another session has its own count.
    let other = with_session(&input, "s2");
    let (_, out) = devin_stop(d.path(), &other, &[("COUNTERCHECK_MAX_NUDGES", "0")]);
    assert!(lets_stop(&out).contains("did NOT pass"), "{out}");
}

#[test]
fn the_stop_hook_fails_open() {
    let d = project();
    let not_git = tempfile::tempdir().unwrap();
    // Garbage on stdin.
    assert_eq!(devin_stop(d.path(), "not json", &[]), (0, String::new()));
    // A folder that isn't a git repository.
    let input = payload(STOP_FULL, not_git.path(), Some("Done."));
    assert_eq!(devin_stop(not_git.path(), &input, &[]), (0, String::new()));
    // No cwd and no DEVIN_PROJECT_DIR: the hook's own folder, here not a repository.
    assert_eq!(devin_stop(not_git.path(), STOP, &[]), (0, String::new()));
    // Switched off.
    let input = payload(STOP_FULL, d.path(), Some("Done."));
    assert_eq!(
        devin_stop(d.path(), &input, &[("COUNTERCHECK_HOOK_DISABLED", "1")]),
        (0, String::new())
    );
}

/// Devin's SessionStart input (`session_id`, `cwd`) locks the checks to the starting commit: an
/// agent that commits weaker checks doesn't change what is checked.
#[test]
fn session_start_locks_the_checks_against_a_committed_edit() {
    let d = project();
    let p = d.path();
    let (code, out) = run_hook("session-start", p, &payload(SESSION_START, p, None), &[]);
    assert_eq!((code, out.trim()), (0, ""));
    let record = p.join(format!(".git/countercheck/sessions/{SESSION}.json"));
    let rec: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&record).unwrap()).unwrap();
    assert_eq!(rec["base"].as_str().unwrap().len(), 40, "{rec}");
    // Same shape as Claude Code's: without DEVIN_PROJECT_DIR it can't be told apart.
    assert_eq!(rec["agent"], "claude-code", "{rec}");
    fs::remove_file(&record).unwrap();
    let pd = p.to_str().unwrap();
    run_hook(
        "session-start",
        p,
        &payload(SESSION_START, p, None),
        &[("DEVIN_PROJECT_DIR", pd)],
    );
    let rec: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&record).unwrap()).unwrap();
    assert_eq!(rec["agent"], "devin", "{rec}");
    fs::write(
        p.join(".countercheck/checks.toml"),
        "[[check]]\nname = \"calc\"\nrun = \"true\"\n",
    )
    .unwrap();
    sh(p, "git commit -qam 'simpler checks'");
    let (_, out) = devin_stop(p, &payload(STOP_FULL, p, Some("All tests pass.")), &[]);
    assert!(block_reason(&out).contains("\"calc\""), "{out}");
}

// ---- hook install / uninstall / status ----

/// Run `countercheck hook <args>` in `dir`, with stdin closed and HOME set to `home`.
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

/// What `hook install --agent devin` writes into a project with no hooks file: the whole file is
/// the event map, each group with the empty matcher of the docs' examples.
const INSTALLED: &str = r#"{
  "SessionStart": [
    {"matcher": "", "hooks": [{"type": "command", "command": "countercheck hook session-start", "timeout": 10}]}
  ],
  "Stop": [
    {"matcher": "", "hooks": [{"type": "command", "command": "countercheck hook devin-stop", "timeout": 600}]}
  ]
}
"#;

fn documented_shape() -> serde_json::Value {
    serde_json::json!({
        "SessionStart": [{"matcher": "", "hooks": [
            {"type": "command", "command": "countercheck hook session-start", "timeout": 10}
        ]}],
        "Stop": [{"matcher": "", "hooks": [
            {"type": "command", "command": "countercheck hook devin-stop", "timeout": 600}
        ]}],
    })
}

fn json_of(path: &Path) -> serde_json::Value {
    serde_json::from_str(&fs::read_to_string(path).unwrap()).unwrap()
}

#[test]
fn install_into_an_empty_project_writes_the_documented_shape() {
    let d = project();
    let home = tempfile::tempdir().unwrap();
    let p = d.path();
    let o = hook(p, &["install", "--agent", "devin", "--yes"], home.path());
    assert!(o.status.success(), "{}", text(&o));
    assert_eq!(fs::read_to_string(p.join(FILE)).unwrap(), INSTALLED);
    assert_eq!(json_of(&p.join(FILE)), documented_shape());
    let t = text(&o);
    // The steps after install: a new session, the trust prompt, and /hooks to confirm.
    assert!(
        t.contains("new `devin` session") && t.contains("trust") && t.contains("/hooks"),
        "{t}"
    );
    assert!(t.contains("docs/hooks/devin.md"), "{t}");
    // The file is guarded by the tamper check, so install says to commit it for that.
    assert!(t.contains("tell if an agent removes the hook"), "{t}");
    assert!(!home.path().join(".config").exists());
    let st = text(&hook(p, &["status"], home.path()));
    assert!(
        st.lines().any(|l| l.starts_with("devin ")
            && l.contains("project")
            && l.contains("installed, not committed")),
        "{st}"
    );
    sh(p, "git add -A && git commit -qm hooks");
    let st = text(&hook(p, &["status"], home.path()));
    assert!(
        st.lines()
            .any(|l| l.starts_with("devin ") && l.contains("project") && l.contains("installed  ")),
        "{st}"
    );
}

#[test]
fn install_keeps_other_hooks_as_written_and_uninstall_takes_out_only_ours() {
    let d = project();
    let home = tempfile::tempdir().unwrap();
    let p = d.path();
    fs::create_dir_all(p.join(".devin")).unwrap();
    let original = "{\n  \"PreToolUse\": [{\"matcher\": \"exec\", \"hooks\": [{\"type\": \"command\", \"command\": \"./scripts/validate.sh\", \"timeout\": 10}]}],\n  \"Stop\": [{\"matcher\": \"\", \"hooks\": [{\"type\": \"command\", \"command\": \"say done\"}]}]\n}\n";
    fs::write(p.join(FILE), original).unwrap();
    let o = hook(p, &["install", "--agent", "devin", "-y"], home.path());
    assert!(o.status.success(), "{}", text(&o));
    let s = fs::read_to_string(p.join(FILE)).unwrap();
    assert!(
        s.starts_with(&original[..original.find("\n  \"Stop\"").unwrap()]),
        "{s}"
    );
    assert!(
        s.contains("say done")
            && s.contains("countercheck hook devin-stop")
            && s.contains("countercheck hook session-start"),
        "{s}"
    );
    assert_eq!(
        fs::read_to_string(p.join(".devin/hooks.v1.json.countercheck-backup")).unwrap(),
        original
    );
    // A second install adds nothing.
    let o = hook(p, &["install", "--agent", "devin", "-y"], home.path());
    assert!(text(&o).contains("already"), "{}", text(&o));
    assert_eq!(fs::read_to_string(p.join(FILE)).unwrap(), s);
    // Uninstall takes out only ours.
    let o = hook(p, &["uninstall", "--agent", "devin", "-y"], home.path());
    assert!(o.status.success(), "{}", text(&o));
    let s = fs::read_to_string(p.join(FILE)).unwrap();
    assert!(!s.contains("countercheck"), "{s}");
    assert!(s.contains(original.lines().nth(1).unwrap()), "{s}");
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&s).unwrap(),
        serde_json::from_str::<serde_json::Value>(original).unwrap()
    );
    let st = text(&hook(p, &["status"], home.path()));
    assert!(
        st.lines().any(|l| l.starts_with("devin ")
            && l.contains("project")
            && l.contains("not installed")),
        "{st}"
    );
}

/// The user-level hooks sit under a `"hooks"` key in another file: install doesn't write them.
#[test]
fn user_and_managed_scopes_are_refused() {
    let d = project();
    let home = tempfile::tempdir().unwrap();
    for scope in ["user", "managed"] {
        let o = hook(
            d.path(),
            &["install", "--agent", "devin", "--scope", scope, "-y"],
            home.path(),
        );
        assert!(!o.status.success(), "{scope}: {}", text(&o));
    }
    assert!(!home.path().join(".config").exists());
    assert!(!d.path().join(".devin").exists());
}

/// Devin CLI also runs the hooks in `.claude/settings.json`: Countercheck there too would run twice.
#[test]
fn install_warns_when_claude_settings_have_countercheck_hooks_too() {
    let d = project();
    let home = tempfile::tempdir().unwrap();
    let p = d.path();
    fs::create_dir_all(p.join(".claude")).unwrap();
    fs::write(
        p.join(".claude/settings.json"),
        r#"{"hooks":{"Stop":[{"hooks":[{"type":"command","command":"countercheck hook claude-stop"}]}]}}"#,
    )
    .unwrap();
    let o = hook(p, &["install", "--agent", "devin", "-y"], home.path());
    assert!(o.status.success(), "{}", text(&o));
    assert!(
        text(&o).contains("also reads .claude/settings.json"),
        "{}",
        text(&o)
    );
}

// ---- the tamper check ----

/// `countercheck verify --json` in `dir`; (exit code, receipt).
fn verify(dir: &Path) -> (i32, serde_json::Value) {
    let mut c = Command::new(BIN);
    c.args(["verify", "--json", "--claim", "fixed", "--repo"])
        .arg(dir);
    if std::env::var_os("COUNTERCHECK").is_some() {
        c.args(["--sandbox", "none"]);
    }
    let o = c.output().unwrap();
    (
        o.status.code().unwrap_or(-1),
        serde_json::from_slice(&o.stdout).unwrap(),
    )
}

/// The worker deletes the committed `.devin/hooks.v1.json`: the next stop wouldn't be checked, so
/// the run is FAILED even though the checks pass, and the receipt names the file.
#[test]
fn deleting_the_committed_hooks_file_fails_the_run() {
    let d = project();
    let p = d.path();
    let home = tempfile::tempdir().unwrap();
    let o = hook(p, &["install", "--agent", "devin", "-y"], home.path());
    assert!(o.status.success(), "{}", text(&o));
    sh(p, "git add -A && git commit -qm hooks");
    fix(p);
    let (code, r) = verify(p);
    assert_eq!(r["verdict"], "verified", "{r:#}");
    assert_eq!(code, 0);
    fs::remove_file(p.join(FILE)).unwrap();
    let (code, r) = verify(p);
    assert_ne!(code, 0, "{r:#}");
    assert_eq!(r["verdict"], "failed", "{r:#}");
    assert_eq!(r["hook_settings"][0]["path"], FILE, "{r:#}");
    let reasons = r["reasons"].to_string();
    assert!(
        reasons.contains(".devin/hooks.v1.json removes the Countercheck"),
        "{reasons}"
    );
}
