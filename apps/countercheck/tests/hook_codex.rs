//! OpenAI Codex: `countercheck hook codex-stop`, `hook session-start` with Codex's input, and
//! `hook install --agent codex`, end to end with the real binary. Inputs are the fixtures in
//! `tests/fixtures/codex/` (built from Codex's hook schemas; see the README there).

use std::fs;
use std::path::Path;
use std::process::{Command, Output, Stdio};

mod common;

const BIN: &str = env!("CARGO_BIN_EXE_countercheck");

const STOP: &str = include_str!("fixtures/codex/stop.json");
const SESSION_START: &str = include_str!("fixtures/codex/session-start.json");

/// Markers of other agents' shells, which would win over Codex's in `agent_from_env`.
const OTHER_AGENTS: &[&str] = &["CLAUDECODE", "CLAUDE_CODE_CHILD_SESSION", "CURSOR_AGENT"];

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

/// A fixture with the project folder and the final message filled in.
fn payload(fixture: &str, cwd: &Path, message: Option<&str>) -> String {
    let mut v: serde_json::Value = serde_json::from_str(fixture).unwrap();
    v["cwd"] = cwd.to_string_lossy().into_owned().into();
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

fn codex_stop(dir: &Path, input: &str, envs: &[(&str, &str)]) -> (i32, String) {
    run_hook("codex-stop", dir, input, envs)
}

/// Codex may finish: no block, and the warning line it shows the user (`systemMessage`).
fn lets_stop(out: &str) -> String {
    let v: serde_json::Value =
        serde_json::from_str(out.trim()).unwrap_or_else(|_| panic!("not one JSON object: {out}"));
    assert!(v.get("decision").is_none(), "blocked: {out}");
    assert_eq!(v.as_object().unwrap().len(), 1, "only systemMessage: {out}");
    v["systemMessage"].as_str().unwrap().to_string()
}

/// The reason in Codex's `{"decision":"block","reason":...}`, or panic.
fn block_reason(out: &str) -> String {
    let v: serde_json::Value = serde_json::from_str(out.trim()).unwrap();
    assert_eq!(v["decision"], "block", "{out}");
    // Codex's Stop output schema allows no other keys (additionalProperties: false).
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
fn verified_work_may_finish_and_the_receipt_says_codex() {
    let d = project();
    fix(d.path());
    let log = d.path().join("hook.log");
    let (code, out) = codex_stop(
        d.path(),
        &payload(STOP, d.path(), Some("Done: calc adds now.")),
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
    // Not `claude-code`, though the input has a transcript_path like Claude Code's.
    assert_eq!(receipt["worker"]["agent"], "codex");
    assert_eq!(receipt["worker"]["model"], "gpt-5.5-codex");
    assert_eq!(
        receipt["worker"]["session"],
        "019a3a6e-7c1d-7f20-9b4e-5d2c8a1f0e63"
    );
}

#[test]
fn a_false_success_claim_is_sent_back_with_the_failing_check() {
    let d = project();
    let log = d.path().join("hook.log");
    let (code, out) = codex_stop(
        d.path(),
        &payload(STOP, d.path(), Some("Fixed it, all tests pass.")),
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
    assert_eq!(receipt["worker"]["agent"], "codex");
}

#[test]
fn editing_the_locked_test_is_still_sent_back() {
    let d = project();
    fs::write(d.path().join("tests/check.sh"), "exit 0\n").unwrap();
    let (code, out) = codex_stop(
        d.path(),
        &payload(STOP, d.path(), Some("All tests pass.")),
        &[],
    );
    assert_eq!(code, 0);
    let reason = block_reason(&out);
    assert!(reason.contains("original versions"), "{reason}");
}

#[test]
fn an_honest_blocker_may_finish() {
    let d = project();
    let said = "I fixed what I could, but test_export can't pass: it contradicts test_display.";
    let (code, out) = codex_stop(d.path(), &payload(STOP, d.path(), Some(said)), &[]);
    assert_eq!(code, 0);
    let msg = lets_stop(&out);
    assert!(
        msg.contains("FAILED") && msg.contains("says it is blocked"),
        "{msg}"
    );
}

/// `last_assistant_message` is nullable in Codex's schema: no message is no claim, never a pass.
#[test]
fn no_final_message_and_failing_checks_is_sent_back() {
    let d = project();
    let (code, out) = codex_stop(d.path(), &payload(STOP, d.path(), None), &[]);
    assert_eq!(code, 0);
    assert!(block_reason(&out).contains("\"calc\""), "{out}");
}

#[test]
fn after_the_push_back_limit_codex_may_finish_with_a_warning() {
    let d = project();
    let log = d.path().join("hook.log");
    let envs = [("COUNTERCHECK_HOOK_LOG", log.to_str().unwrap())];
    let input = payload(STOP, d.path(), Some("All tests pass."));
    for _ in 0..3 {
        let (code, out) = codex_stop(d.path(), &input, &envs);
        assert_eq!(code, 0);
        block_reason(&out);
    }
    let (code, out) = codex_stop(d.path(), &input, &envs);
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
    let (_, out) = codex_stop(d.path(), &other, &[("COUNTERCHECK_MAX_NUDGES", "0")]);
    assert!(lets_stop(&out).contains("did NOT pass"), "{out}");
}

#[test]
fn the_stop_hook_fails_open() {
    let d = project();
    let not_git = tempfile::tempdir().unwrap();
    // Garbage on stdin.
    assert_eq!(codex_stop(d.path(), "not json", &[]), (0, String::new()));
    // A folder that isn't a git repository.
    let input = payload(STOP, not_git.path(), Some("Done."));
    assert_eq!(codex_stop(not_git.path(), &input, &[]), (0, String::new()));
    // No cwd: the hook's own folder (Codex runs hooks in the session cwd), here not a repository.
    let mut v: serde_json::Value = serde_json::from_str(&input).unwrap();
    v.as_object_mut().unwrap().remove("cwd");
    assert_eq!(
        codex_stop(not_git.path(), &v.to_string(), &[]),
        (0, String::new())
    );
    // Switched off.
    let input = payload(STOP, d.path(), Some("Done."));
    assert_eq!(
        codex_stop(d.path(), &input, &[("COUNTERCHECK_HOOK_DISABLED", "1")]),
        (0, String::new())
    );
}

/// Codex's SessionStart input (`session_id`, `cwd`) locks the checks to the starting commit: an
/// agent that commits weaker checks doesn't change what is checked.
#[test]
fn session_start_locks_the_checks_against_a_committed_edit() {
    let d = project();
    let p = d.path();
    let (code, out) = run_hook("session-start", p, &payload(SESSION_START, p, None), &[]);
    assert_eq!((code, out.trim()), (0, ""));
    let record = p.join(".git/countercheck/sessions/019a3a6e-7c1d-7f20-9b4e-5d2c8a1f0e63.json");
    let rec: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(record).unwrap()).unwrap();
    assert_eq!(rec["base"].as_str().unwrap().len(), 40, "{rec}");
    fs::write(
        p.join(".countercheck/checks.toml"),
        "[[check]]\nname = \"calc\"\nrun = \"true\"\n",
    )
    .unwrap();
    sh(p, "git commit -qam 'simpler checks'");
    let (_, out) = codex_stop(p, &payload(STOP, p, Some("All tests pass.")), &[]);
    assert!(block_reason(&out).contains("\"calc\""), "{out}");
}

/// `verify` run from a Codex shell names the agent: `CODEX_THREAD_ID` is set in every Codex shell,
/// `CODEX_SANDBOX` only under the macOS sandbox.
#[test]
fn verify_in_a_codex_shell_names_codex() {
    let d = project();
    fix(d.path());
    let mut c = Command::new(BIN);
    c.args(["verify", "--json", "--repo"]).arg(d.path());
    if std::env::var_os("COUNTERCHECK").is_some() {
        c.args(["--sandbox", "none"]);
    }
    for k in OTHER_AGENTS
        .iter()
        .chain(&["CODEX_SANDBOX", "CODEX_SANDBOX_NETWORK_DISABLED"])
    {
        c.env_remove(k);
    }
    c.env("CODEX_THREAD_ID", "019a3a6e-7c1d-7f20-9b4e-5d2c8a1f0e63");
    let r: serde_json::Value = serde_json::from_slice(&c.output().unwrap().stdout).unwrap();
    assert_eq!(r["worker"]["agent"], "codex", "{r:#}");
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

/// What `hook install --agent codex` writes into a project with no `.codex/hooks.json`: the
/// docs' shape (event, matcher group, command handlers).
const INSTALLED: &str = r#"{
  "hooks": {
    "SessionStart": [
      {"hooks": [{"type": "command", "command": "countercheck hook session-start", "timeout": 10}]}
    ],
    "Stop": [
      {"hooks": [{"type": "command", "command": "countercheck hook codex-stop", "timeout": 600}]}
    ]
  }
}
"#;

/// `INSTALLED` as a value, laid out like the docs' `hooks.json` example.
fn documented_shape() -> serde_json::Value {
    serde_json::json!({"hooks": {
        "SessionStart": [{"hooks": [
            {"type": "command", "command": "countercheck hook session-start", "timeout": 10}
        ]}],
        "Stop": [{"hooks": [
            {"type": "command", "command": "countercheck hook codex-stop", "timeout": 600}
        ]}],
    }})
}

fn json_of(path: &Path) -> serde_json::Value {
    serde_json::from_str(&fs::read_to_string(path).unwrap()).unwrap()
}

#[test]
fn install_into_an_empty_project_writes_the_documented_shape() {
    let d = project();
    let home = tempfile::tempdir().unwrap();
    let p = d.path();
    let o = hook(p, &["install", "--agent", "codex", "--yes"], home.path());
    assert!(o.status.success(), "{}", text(&o));
    assert_eq!(
        fs::read_to_string(p.join(".codex/hooks.json")).unwrap(),
        INSTALLED
    );
    assert_eq!(json_of(&p.join(".codex/hooks.json")), documented_shape());
    // The trust step is printed: without it Codex never runs the hook.
    let t = text(&o);
    assert!(
        t.contains("trust this folder") && t.contains("Hooks need review") && t.contains("/hooks"),
        "{t}"
    );
    assert!(t.contains("docs/hooks/codex.md"), "{t}");
    // Nothing went to the user's folder.
    assert!(!home.path().join(".codex").exists());
    let st = text(&hook(p, &["status"], home.path()));
    assert!(
        st.lines().any(|l| l.starts_with("codex ")
            && l.contains("project")
            && l.contains("installed, not committed")),
        "{st}"
    );
    sh(p, "git add -A && git commit -qm hooks");
    let st = text(&hook(p, &["status"], home.path()));
    assert!(
        st.lines()
            .any(|l| l.starts_with("codex ") && l.contains("project") && l.contains("installed  ")),
        "{st}"
    );
}

#[test]
fn install_keeps_other_hooks_as_written_and_uninstall_takes_out_only_ours() {
    let d = project();
    let home = tempfile::tempdir().unwrap();
    let p = d.path();
    fs::create_dir_all(p.join(".codex")).unwrap();
    let original = "{\n  \"hooks\": {\n    \"PreToolUse\": [{\"matcher\": \"Bash\", \"hooks\": [{\"type\": \"command\", \"command\": \"python3 .codex/hooks/guard.py\"}]}],\n    \"Stop\": [{\"hooks\": [{\"type\": \"command\", \"command\": \"say done\", \"timeout\": 5}]}]\n  }\n}\n";
    fs::write(p.join(".codex/hooks.json"), original).unwrap();
    let o = hook(p, &["install", "--agent", "codex", "-y"], home.path());
    assert!(o.status.success(), "{}", text(&o));
    let s = fs::read_to_string(p.join(".codex/hooks.json")).unwrap();
    assert!(
        s.starts_with("{\n  \"hooks\": {\n    \"PreToolUse\": [{\"matcher\": \"Bash\", \"hooks\": [{\"type\": \"command\", \"command\": \"python3 .codex/hooks/guard.py\"}]}],\n"),
        "{s}"
    );
    assert!(
        s.contains("say done") && s.contains("countercheck hook codex-stop"),
        "{s}"
    );
    assert!(s.contains("countercheck hook session-start"), "{s}");
    assert_eq!(
        fs::read_to_string(p.join(".codex/hooks.json.countercheck-backup")).unwrap(),
        original
    );
    // A second install adds nothing.
    let o = hook(p, &["install", "--agent", "codex", "-y"], home.path());
    assert!(text(&o).contains("already"), "{}", text(&o));
    assert_eq!(fs::read_to_string(p.join(".codex/hooks.json")).unwrap(), s);
    // Uninstall takes out only ours: the rest is what it was (the untouched PreToolUse line byte for
    // byte; the edited Stop list may be laid out again).
    let o = hook(p, &["uninstall", "--agent", "codex", "-y"], home.path());
    assert!(o.status.success(), "{}", text(&o));
    let s = fs::read_to_string(p.join(".codex/hooks.json")).unwrap();
    assert!(!s.contains("countercheck"), "{s}");
    assert!(s.contains(original.lines().nth(2).unwrap()), "{s}");
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&s).unwrap(),
        serde_json::from_str::<serde_json::Value>(original).unwrap()
    );
    let st = text(&hook(p, &["status"], home.path()));
    assert!(
        st.lines().any(|l| l.starts_with("codex ")
            && l.contains("project")
            && l.contains("not installed")),
        "{st}"
    );
}

#[test]
fn install_for_the_user_writes_under_the_home_folder() {
    let d = project();
    let home = tempfile::tempdir().unwrap();
    let o = hook(
        d.path(),
        &["install", "--agent", "codex", "--scope", "user", "-y"],
        home.path(),
    );
    assert!(o.status.success(), "{}", text(&o));
    assert_eq!(
        fs::read_to_string(home.path().join(".codex/hooks.json")).unwrap(),
        INSTALLED
    );
    assert!(!d.path().join(".codex").exists());
    let st = text(&hook(d.path(), &["status"], home.path()));
    assert!(
        st.lines()
            .any(|l| l.starts_with("codex ") && l.contains("user") && l.contains("installed")),
        "{st}"
    );
    // Codex's hooks aren't Claude Code's managed settings.
    let o = hook(
        d.path(),
        &["install", "--agent", "codex", "--scope", "managed", "-y"],
        home.path(),
    );
    assert!(!o.status.success());
    assert!(text(&o).contains("--scope user or project"), "{}", text(&o));
}

/// Codex also reads `[hooks]` in `.codex/config.toml`: a Countercheck hook there too would run twice.
#[test]
fn install_warns_when_config_toml_has_countercheck_hooks_too() {
    let d = project();
    let home = tempfile::tempdir().unwrap();
    let p = d.path();
    fs::create_dir_all(p.join(".codex")).unwrap();
    fs::write(
        p.join(".codex/config.toml"),
        "[[hooks.Stop]]\n[[hooks.Stop.hooks]]\ntype = \"command\"\ncommand = \"countercheck hook codex-stop\"\n",
    )
    .unwrap();
    let o = hook(p, &["install", "--agent", "codex", "-y"], home.path());
    assert!(o.status.success(), "{}", text(&o));
    assert!(
        text(&o).contains("also reads .codex/config.toml"),
        "{}",
        text(&o)
    );
}
