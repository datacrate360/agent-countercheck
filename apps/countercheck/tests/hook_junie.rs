//! JetBrains Junie CLI: `countercheck hook junie-stop`, `hook session-start` with Junie's input, and
//! `hook install --agent junie` (user file only: Junie ignores project hooks), end to end with the
//! real binary. Inputs are the fixtures in `tests/fixtures/junie/` (see the README there).

use std::fs;
use std::path::Path;
use std::process::{Command, Output, Stdio};

mod common;

const BIN: &str = env!("CARGO_BIN_EXE_countercheck");

/// The docs' Stop input: no folder, no session id.
const STOP: &str = include_str!("fixtures/junie/stop.json");
/// What Junie 26.11.3 sends: also `session_id`, `cwd`, `project_path`.
const STOP_WITH_SESSION: &str = include_str!("fixtures/junie/stop-with-session.json");
const SESSION_START: &str = include_str!("fixtures/junie/session-start.json");
const SESSION: &str = "0f6c2b1e-4d7a-4e2b-9a51-3c8e7d1f2a90";

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

/// A fixture with the project folder (where it has one), the final message and `stop_hook_active`
/// filled in.
fn payload(fixture: &str, cwd: &Path, message: &str, retry: bool) -> String {
    let mut v: serde_json::Value = serde_json::from_str(fixture).unwrap();
    for key in ["cwd", "project_path"] {
        if v.get(key).is_some() {
            v[key] = cwd.to_string_lossy().into_owned().into();
        }
    }
    if v.get("last_assistant_message").is_some() {
        v["last_assistant_message"] = message.into();
        v["stop_hook_active"] = retry.into();
    }
    v.to_string()
}

/// Run `countercheck hook <name>` in `dir` with `input` on stdin; (exit code, stdout).
fn run_hook(name: &str, dir: &Path, input: &str, envs: &[(&str, &str)]) -> (i32, String) {
    use std::io::Write;
    let mut cmd = Command::new(BIN);
    cmd.args(["hook", name])
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

fn junie_stop(dir: &Path, input: &str, envs: &[(&str, &str)]) -> (i32, String) {
    run_hook("junie-stop", dir, input, envs)
}

/// Junie may finish: no block, no `additionalContext` (on a plain exit Junie would run one more
/// step with it), only the user-facing `systemMessage`.
fn lets_stop(out: &str) -> String {
    let v: serde_json::Value =
        serde_json::from_str(out.trim()).unwrap_or_else(|_| panic!("not one JSON object: {out}"));
    assert_eq!(v.as_object().unwrap().len(), 1, "only systemMessage: {out}");
    v["systemMessage"].as_str().unwrap().to_string()
}

/// The reason in Junie's `{"decision":"block","reason":...}`, or panic.
fn block_reason(out: &str) -> String {
    let v: serde_json::Value = serde_json::from_str(out.trim()).unwrap();
    assert_eq!(v["decision"], "block", "{out}");
    // Not `continue: false`: that hard-halts the task instead of sending it back.
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
fn verified_work_may_finish_and_the_receipt_says_junie() {
    let d = project();
    fix(d.path());
    let log = d.path().join("hook.log");
    // The docs' input has no folder: the hook checks the folder Junie runs it in.
    let (code, out) = junie_stop(
        d.path(),
        &payload(STOP, d.path(), "Done: calc adds now.", false),
        &[("COUNTERCHECK_HOOK_LOG", log.to_str().unwrap())],
    );
    assert_eq!(code, 0);
    let msg = lets_stop(&out);
    assert!(
        msg.starts_with("✓ Countercheck: VERIFIED, 1 check(s) passed"),
        "{msg}"
    );
    let (_, receipt) = last_log_receipt(&log);
    assert_eq!(receipt["verdict"], "verified");
    assert_eq!(receipt["worker"]["agent"], "junie");
    assert!(receipt["worker"].get("model").is_none_or(|m| m.is_null()));
}

/// When Junie sends `cwd` and `session_id` (26.11.3 does), they are used: the hook may run anywhere.
#[test]
fn the_folder_and_session_in_the_input_are_used() {
    let d = project();
    fix(d.path());
    let elsewhere = tempfile::tempdir().unwrap();
    let log = d.path().join("hook.log");
    let (code, out) = junie_stop(
        elsewhere.path(),
        &payload(STOP_WITH_SESSION, d.path(), "Done.", false),
        &[("COUNTERCHECK_HOOK_LOG", log.to_str().unwrap())],
    );
    assert_eq!(code, 0);
    assert!(lets_stop(&out).contains("VERIFIED"), "{out}");
    let (_, receipt) = last_log_receipt(&log);
    assert_eq!(receipt["worker"]["agent"], "junie");
    assert_eq!(receipt["worker"]["session"], SESSION);
}

#[test]
fn a_false_success_claim_is_sent_back_with_the_failing_check() {
    let d = project();
    let log = d.path().join("hook.log");
    let (code, out) = junie_stop(
        d.path(),
        &payload(STOP, d.path(), "Fixed it, all tests pass.", false),
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
    assert_eq!(receipt["worker"]["agent"], "junie");
}

#[test]
fn editing_the_locked_test_is_still_sent_back() {
    let d = project();
    fs::write(d.path().join("tests/check.sh"), "exit 0\n").unwrap();
    let (code, out) = junie_stop(
        d.path(),
        &payload(STOP, d.path(), "All tests pass.", false),
        &[],
    );
    assert_eq!(code, 0);
    assert!(block_reason(&out).contains("original versions"), "{out}");
}

#[test]
fn an_honest_blocker_may_finish() {
    let d = project();
    let said = "I fixed what I could, but test_export can't pass: it contradicts test_display.";
    let (code, out) = junie_stop(d.path(), &payload(STOP, d.path(), said, false), &[]);
    assert_eq!(code, 0);
    let msg = lets_stop(&out);
    assert!(
        msg.contains("FAILED") && msg.contains("says it is blocked"),
        "{msg}"
    );
}

/// Junie sends an empty submission as "": no claim, never a pass.
#[test]
fn an_empty_message_and_failing_checks_is_sent_back() {
    let d = project();
    let (code, out) = junie_stop(d.path(), &payload(STOP, d.path(), "", false), &[]);
    assert_eq!(code, 0);
    assert!(block_reason(&out).contains("\"calc\""), "{out}");
}

/// Without a session id the limit counts per task: Junie's retries (`stop_hook_active: true`) use
/// it up, and the next task (`false`) starts again.
#[test]
fn after_the_push_back_limit_junie_may_finish_and_a_new_task_counts_again() {
    let d = project();
    let log = d.path().join("hook.log");
    let envs = [("COUNTERCHECK_HOOK_LOG", log.to_str().unwrap())];
    for retry in [false, true, true] {
        let input = payload(STOP, d.path(), "All tests pass.", retry);
        let (code, out) = junie_stop(d.path(), &input, &envs);
        assert_eq!(code, 0);
        block_reason(&out);
    }
    let input = payload(STOP, d.path(), "All tests pass.", true);
    let (code, out) = junie_stop(d.path(), &input, &envs);
    assert_eq!(code, 0);
    let msg = lets_stop(&out);
    assert!(
        msg.starts_with("⚠ Countercheck: FAILED") && msg.contains("did NOT pass"),
        "{msg}"
    );
    let (line, _) = last_log_receipt(&log);
    assert_eq!(line["gave_up"], true);
    // A new task: sent back again.
    let (_, out) = junie_stop(
        d.path(),
        &payload(STOP, d.path(), "All tests pass.", false),
        &envs,
    );
    block_reason(&out);
    // COUNTERCHECK_MAX_NUDGES changes the limit.
    let (_, out) = junie_stop(
        d.path(),
        &payload(STOP, d.path(), "All tests pass.", false),
        &[("COUNTERCHECK_MAX_NUDGES", "0")],
    );
    assert!(lets_stop(&out).contains("did NOT pass"), "{out}");
}

/// With a session id the limit counts per session, like every other agent: a new task in the same
/// session doesn't reset it.
#[test]
fn with_a_session_id_the_limit_counts_per_session() {
    let d = project();
    for _ in 0..3 {
        let input = payload(STOP_WITH_SESSION, d.path(), "All tests pass.", false);
        block_reason(&junie_stop(d.path(), &input, &[]).1);
    }
    let input = payload(STOP_WITH_SESSION, d.path(), "All tests pass.", false);
    let (_, out) = junie_stop(d.path(), &input, &[]);
    assert!(lets_stop(&out).contains("did NOT pass"), "{out}");
    // The session-less count is separate.
    let (_, out) = junie_stop(
        d.path(),
        &payload(STOP, d.path(), "All tests pass.", true),
        &[],
    );
    block_reason(&out);
}

#[test]
fn the_stop_hook_fails_open() {
    let d = project();
    let not_git = tempfile::tempdir().unwrap();
    // Garbage on stdin.
    assert_eq!(junie_stop(d.path(), "not json", &[]), (0, String::new()));
    // A folder that isn't a git repository, named in the input or the one the hook runs in.
    let input = payload(STOP_WITH_SESSION, not_git.path(), "Done.", false);
    assert_eq!(junie_stop(d.path(), &input, &[]), (0, String::new()));
    let input = payload(STOP, not_git.path(), "Done.", false);
    assert_eq!(junie_stop(not_git.path(), &input, &[]), (0, String::new()));
    // Switched off.
    let input = payload(STOP, d.path(), "Done.", false);
    assert_eq!(
        junie_stop(d.path(), &input, &[("COUNTERCHECK_HOOK_DISABLED", "1")]),
        (0, String::new())
    );
}

/// Junie's SessionStart input (`session_id`, `cwd`, `project_path`) records the starting commit,
/// labelled junie; a Stop with the same session id is checked against it, so an agent that commits
/// weaker checks doesn't change what is checked.
#[test]
fn session_start_locks_the_checks_against_a_committed_edit() {
    let d = project();
    let p = d.path();
    let (code, out) = run_hook(
        "session-start",
        p,
        &payload(SESSION_START, p, "", false),
        &[],
    );
    assert_eq!((code, out.trim()), (0, ""));
    let record = p.join(format!(".git/countercheck/sessions/{SESSION}.json"));
    let rec: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(record).unwrap()).unwrap();
    assert_eq!(rec["base"].as_str().unwrap().len(), 40, "{rec}");
    assert_eq!(rec["agent"], "junie", "{rec}");
    fs::write(
        p.join(".countercheck/checks.toml"),
        "[[check]]\nname = \"calc\"\nrun = \"true\"\n",
    )
    .unwrap();
    sh(p, "git commit -qam 'simpler checks'");
    let (_, out) = junie_stop(
        p,
        &payload(STOP_WITH_SESSION, p, "All tests pass.", false),
        &[],
    );
    assert!(block_reason(&out).contains("\"calc\""), "{out}");
}

// ---- hook install / uninstall / status ----

/// Run `countercheck hook <args>` in `dir`, with stdin closed, HOME set to `home` and no JUNIE_HOME.
fn hook(dir: &Path, args: &[&str], home: &Path) -> Output {
    Command::new(BIN)
        .arg("hook")
        .args(args)
        .current_dir(dir)
        .env("HOME", home)
        .env("USERPROFILE", home)
        .env_remove("JUNIE_HOME")
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

/// What `hook install --agent junie` writes into an empty `~/.junie/config.json`.
const INSTALLED: &str = r#"{
  "hooks": {
    "SessionStart": [
      {"hooks": [{"type": "command", "command": "countercheck hook session-start", "timeout": 10}]}
    ],
    "Stop": [
      {"hooks": [{"type": "command", "command": "countercheck hook junie-stop", "timeout": 600}]}
    ]
  }
}
"#;

/// `INSTALLED` as a value, laid out like the docs' `config.json` example (no `blockOnError`).
fn documented_shape() -> serde_json::Value {
    serde_json::json!({"hooks": {
        "SessionStart": [{"hooks": [
            {"type": "command", "command": "countercheck hook session-start", "timeout": 10}
        ]}],
        "Stop": [{"hooks": [
            {"type": "command", "command": "countercheck hook junie-stop", "timeout": 600}
        ]}],
    }})
}

fn json_of(path: &Path) -> serde_json::Value {
    serde_json::from_str(&fs::read_to_string(path).unwrap()).unwrap()
}

/// Without `--scope`, junie goes to the user file: Junie runs no hooks from a project file.
#[test]
fn install_writes_the_user_file_by_default() {
    let d = project();
    let home = tempfile::tempdir().unwrap();
    let p = d.path();
    let o = hook(p, &["install", "--agent", "junie", "--yes"], home.path());
    assert!(o.status.success(), "{}", text(&o));
    let file = home.path().join(".junie/config.json");
    assert_eq!(fs::read_to_string(&file).unwrap(), INSTALLED);
    assert_eq!(json_of(&file), documented_shape());
    // Nothing in the project, and no "commit it" for a file outside it.
    assert!(!p.join(".junie").exists());
    let t = text(&o);
    assert!(!t.contains("commit it"), "{t}");
    assert!(
        t.contains("start a new `junie` session") && t.contains("install-eap.sh"),
        "{t}"
    );
    assert!(t.contains("docs/hooks/junie.md"), "{t}");
    let st = text(&hook(p, &["status"], home.path()));
    let junie: Vec<&str> = st.lines().filter(|l| l.starts_with("junie ")).collect();
    assert_eq!(junie.len(), 1, "user only, no project line: {st}");
    assert!(
        junie[0].contains("user") && junie[0].contains("installed  "),
        "{st}"
    );
}

/// `--scope project` is refused with why and what to do; nothing is written.
#[test]
fn project_scope_is_refused_and_writes_nothing() {
    let d = project();
    let home = tempfile::tempdir().unwrap();
    let p = d.path();
    for cmd in ["install", "uninstall"] {
        let o = hook(
            p,
            &[cmd, "--agent", "junie", "--scope", "project", "-y"],
            home.path(),
        );
        assert!(!o.status.success(), "{cmd}: {}", text(&o));
        let t = text(&o);
        assert!(
            t.contains("Junie ignores hooks in a project's .junie/config.json")
                && t.contains("--scope user")
                && t.contains("docs/hooks/junie.md"),
            "{t}"
        );
    }
    assert!(!p.join(".junie").exists());
    assert!(!home.path().join(".junie").exists());
    // Not Claude Code's managed settings either.
    let o = hook(
        p,
        &["install", "--agent", "junie", "--scope", "managed", "-y"],
        home.path(),
    );
    assert!(!o.status.success());
    assert!(!home.path().join(".junie").exists());
}

#[test]
fn install_keeps_other_settings_as_written_and_uninstall_takes_out_only_ours() {
    let d = project();
    let home = tempfile::tempdir().unwrap();
    let p = d.path();
    let file = home.path().join(".junie/config.json");
    fs::create_dir_all(file.parent().unwrap()).unwrap();
    let original = "{\n  \"model\": \"sonnet\",\n  \"hooks\": {\n    \"PreToolUse\": [{\"matcher\": \"Bash\", \"hooks\": [{\"type\": \"command\", \"command\": \"~/.junie/hooks/check-bash-command.sh\"}]}],\n    \"Stop\": [{\"hooks\": [{\"type\": \"command\", \"command\": \"say done\", \"timeout\": 5}]}]\n  }\n}\n";
    fs::write(&file, original).unwrap();
    let o = hook(p, &["install", "--agent", "junie", "-y"], home.path());
    assert!(o.status.success(), "{}", text(&o));
    let s = fs::read_to_string(&file).unwrap();
    assert!(
        s.starts_with("{\n  \"model\": \"sonnet\",\n  \"hooks\": {\n    \"PreToolUse\": [{\"matcher\": \"Bash\", \"hooks\": [{\"type\": \"command\", \"command\": \"~/.junie/hooks/check-bash-command.sh\"}]}],\n"),
        "{s}"
    );
    assert!(
        s.contains("say done")
            && s.contains("countercheck hook junie-stop")
            && s.contains("countercheck hook session-start"),
        "{s}"
    );
    assert_eq!(
        fs::read_to_string(home.path().join(".junie/config.json.countercheck-backup")).unwrap(),
        original
    );
    // A second install adds nothing.
    let o = hook(p, &["install", "--agent", "junie", "-y"], home.path());
    assert!(text(&o).contains("already"), "{}", text(&o));
    assert_eq!(fs::read_to_string(&file).unwrap(), s);
    // Uninstall takes out only ours.
    let o = hook(p, &["uninstall", "--agent", "junie", "-y"], home.path());
    assert!(o.status.success(), "{}", text(&o));
    let s = fs::read_to_string(&file).unwrap();
    assert!(!s.contains("countercheck"), "{s}");
    assert!(s.contains(original.lines().nth(3).unwrap()), "{s}");
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&s).unwrap(),
        serde_json::from_str::<serde_json::Value>(original).unwrap()
    );
    let st = text(&hook(p, &["status"], home.path()));
    assert!(
        st.lines()
            .any(|l| l.starts_with("junie ") && l.contains("not installed")),
        "{st}"
    );
}

/// Junie reads `$JUNIE_HOME/config.json` when JUNIE_HOME is set: install writes there.
#[test]
fn junie_home_moves_the_user_file() {
    let d = project();
    let home = tempfile::tempdir().unwrap();
    let junie_home = tempfile::tempdir().unwrap();
    let o = Command::new(BIN)
        .args(["hook", "install", "--agent", "junie", "-y"])
        .current_dir(d.path())
        .env("HOME", home.path())
        .env("USERPROFILE", home.path())
        .env("JUNIE_HOME", junie_home.path())
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert!(o.status.success(), "{}", text(&o));
    assert_eq!(
        json_of(&junie_home.path().join("config.json")),
        documented_shape()
    );
    assert!(!home.path().join(".junie").exists());
}
