//! Goose: `countercheck hook goose-stop`, its session start, and `hook install --agent goose`, end
//! to end with the real binary. Payloads in `tests/fixtures/goose/` (where each comes from is in
//! its README there).

use std::fs;
use std::io::Write;
use std::path::Path;
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

/// A repository whose committed check fails: `src/calc.sh` subtracts instead of adding.
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

fn fix(p: &Path) {
    fs::write(p.join("src/calc.sh"), "echo $(($1 + $2))\n").unwrap();
}

/// A fixture, with `working_dir` set to `dir` (`None`: as recorded) and `last_assistant_message`
/// replaced when given.
fn payload(name: &str, dir: Option<&Path>, said: Option<&str>) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/goose")
        .join(name);
    let mut v: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(path).unwrap()).unwrap();
    let obj = v.as_object_mut().unwrap();
    if let Some(dir) = dir {
        obj.insert("working_dir".into(), dir.to_string_lossy().into());
    }
    if let Some(said) = said {
        obj.insert("last_assistant_message".into(), said.into());
    }
    v.to_string()
}

struct Run {
    code: i32,
    stdout: String,
    stderr: String,
}

/// Run `countercheck hook <name>` in `cwd` (Goose runs hooks in its own working directory).
fn run(name: &str, input: &str, cwd: &Path, envs: &[(&str, &str)]) -> Run {
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
    Run {
        code: out.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
    }
}

fn stop(input: &str, cwd: &Path, envs: &[(&str, &str)]) -> Run {
    run("goose-stop", input, cwd, envs)
}

/// The turn may end: exactly Goose's clean allow, and the user's line on stderr.
fn allows(r: &Run) -> &str {
    assert_eq!(r.code, 0, "{}", r.stderr);
    assert_eq!(r.stdout.trim(), r#"{"decision":"allow"}"#, "{}", r.stderr);
    r.stderr.trim()
}

/// The reason in a Goose block, or panic.
fn block_reason(r: &Run) -> String {
    assert_eq!(r.code, 0, "{}", r.stderr);
    let v: serde_json::Value = serde_json::from_str(r.stdout.trim())
        .unwrap_or_else(|_| panic!("not one JSON object: {}", r.stdout));
    assert_eq!(v["decision"], "block", "{}", r.stdout);
    assert_eq!(v.as_object().unwrap().len(), 2, "{}", r.stdout);
    v["reason"].as_str().unwrap().to_string()
}

#[test]
fn verified_work_may_stop_with_the_users_line() {
    let d = project();
    fix(d.path());
    let r = stop(
        &payload("stop-source.json", Some(d.path()), None),
        d.path(),
        &[],
    );
    let msg = allows(&r);
    assert!(
        msg.starts_with("✓ Countercheck: VERIFIED, 1 check(s) passed"),
        "{msg}"
    );
}

#[test]
fn a_false_success_claim_is_sent_back_with_the_failing_check() {
    let d = project();
    let log = d.path().join("hook.log");
    let anywhere = tempfile::tempdir().unwrap();
    // The folder comes from `working_dir`, not from where the hook runs.
    let r = stop(
        &payload("stop-source.json", Some(d.path()), None),
        anywhere.path(),
        &[("COUNTERCHECK_HOOK_LOG", log.to_str().unwrap())],
    );
    let reason = block_reason(&r);
    assert!(
        reason.contains("FAILED") && reason.contains("\"calc\""),
        "{reason}"
    );
    let line: serde_json::Value =
        serde_json::from_str(fs::read_to_string(&log).unwrap().trim()).unwrap();
    assert_eq!(line["nudged"], true);
    assert_eq!(line["gave_up"], false);
    assert_eq!(line["session_id"], "20261030_7");
    let receipt: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(line["receipt"].as_str().unwrap()).unwrap())
            .unwrap();
    assert_eq!(receipt["worker"]["agent"], "goose");
    assert_eq!(receipt["claim"], "Fixed it, all tests pass.");
}

#[test]
fn the_docs_payload_without_a_folder_checks_the_hooks_working_directory() {
    let d = project();
    let r = stop(&payload("stop-docs.json", None, None), d.path(), &[]);
    assert!(block_reason(&r).contains("\"calc\""), "{}", r.stdout);
}

#[test]
fn an_honest_blocker_report_may_stop() {
    let d = project();
    let said = "I fixed what I could, but test_export can't pass: it contradicts test_display.";
    let r = stop(
        &payload("stop-source.json", Some(d.path()), Some(said)),
        d.path(),
        &[],
    );
    let msg = allows(&r);
    assert!(
        msg.contains("FAILED") && msg.contains("says it is blocked"),
        "{msg}"
    );
}

#[test]
fn after_the_push_back_limit_it_may_stop_with_a_warning() {
    let d = project();
    let input = payload("stop-source.json", Some(d.path()), None);
    let envs = [("COUNTERCHECK_MAX_NUDGES", "1")];
    assert!(block_reason(&stop(&input, d.path(), &envs)).contains("FAILED"));
    let r = stop(&input, d.path(), &envs);
    let msg = allows(&r);
    assert!(
        msg.starts_with("⚠ Countercheck: FAILED") && msg.contains("push-back limit"),
        "{msg}"
    );
}

/// Anything wrong lets the turn end: exit 0 with nothing on stdout (Goose's clean allow).
#[test]
fn fails_open() {
    let not_git = tempfile::tempdir().unwrap();
    let d = project();
    let cases = [
        ("not json".to_string(), d.path()),
        // no folder in the input, and the hook runs outside any repository
        (payload("stop-docs.json", None, None), not_git.path()),
        (
            payload("stop-source.json", Some(not_git.path()), None),
            d.path(),
        ),
    ];
    for (input, cwd) in cases {
        let r = stop(&input, cwd, &[]);
        assert_eq!(
            (r.code, r.stdout.as_str()),
            (0, ""),
            "{input}: {}",
            r.stderr
        );
        assert!(r.stderr.contains("countercheck hook:"), "{}", r.stderr);
    }
    let r = stop(
        &payload("stop-source.json", Some(d.path()), None),
        d.path(),
        &[("COUNTERCHECK_HOOK_DISABLED", "1")],
    );
    assert_eq!((r.code, r.stdout.as_str()), (0, ""));
}

/// Goose's session start (with `working_dir`, or without it from the folder Goose runs in) locks
/// the checks: a committed test edit doesn't move them.
#[test]
fn session_start_keeps_the_checks_locked_when_the_agent_commits() {
    for (fixture, with_dir) in [
        ("session-start-source.json", true),
        ("session-start-client.json", false),
    ] {
        let d = project();
        let anywhere = tempfile::tempdir().unwrap();
        let cwd = if with_dir { anywhere.path() } else { d.path() };
        let input = payload(fixture, with_dir.then_some(d.path()), None);
        let r = run("session-start", &input, cwd, &[]);
        assert_eq!((r.code, r.stdout.as_str()), (0, ""), "{}", r.stderr);
        sh(
            d.path(),
            "echo 'exit 0' > tests/check.sh && git commit -qam 'fix the tests'",
        );
        let r = stop(
            &payload("stop-source.json", Some(d.path()), None),
            d.path(),
            &[],
        );
        assert!(
            block_reason(&r).contains("FAILED"),
            "{fixture}: {}",
            r.stdout
        );
    }
}

/// `verify` run from Goose's shell tool names Goose (it sets `GOOSE_TERMINAL=1` and `AGENT=goose`).
#[test]
fn verify_names_goose_from_its_shell() {
    let d = project();
    let agent = |set: &[(&str, &str)]| -> serde_json::Value {
        let mut cmd = Command::new(BIN);
        cmd.args(["verify", "--json", "--sandbox", "none", "--repo"])
            .arg(d.path());
        for m in [
            "CLAUDE_CODE_CHILD_SESSION",
            "CLAUDECODE",
            "CURSOR_AGENT",
            "CODEX_SANDBOX",
            "CODEX_SANDBOX_NETWORK_DISABLED",
            "HERMES_SESSION_ID",
            "GOOSE_TERMINAL",
            "AGENT",
        ] {
            cmd.env_remove(m);
        }
        for (k, v) in set {
            cmd.env(k, v);
        }
        let v: serde_json::Value = serde_json::from_slice(&cmd.output().unwrap().stdout).unwrap();
        v["worker"]["agent"].clone()
    };
    assert_eq!(
        agent(&[("GOOSE_TERMINAL", "1"), ("AGENT", "goose")]),
        "goose"
    );
    assert_eq!(agent(&[("AGENT", "goose")]), "goose");
    assert_eq!(agent(&[("AGENT", "aider")]), serde_json::Value::Null);
    // Claude Code started from Goose's shell: the inner spawner wins.
    assert_eq!(
        agent(&[("GOOSE_TERMINAL", "1"), ("CLAUDECODE", "1")]),
        "claude-code"
    );
}

// ---- hook install --agent goose ----

const HOOKS: &str = ".agents/plugins/countercheck/hooks/hooks.json";

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

#[test]
fn install_writes_the_plugin_hooks_file() {
    let d = project();
    let p = d.path();
    let o = hook(p, &["install", "--agent", "goose", "--yes"], &[]);
    assert!(o.status.success(), "{}", text(&o));
    assert_eq!(
        fs::read_to_string(p.join(HOOKS)).unwrap(),
        r#"{
  "hooks": {
    "SessionStart": [
      {"hooks": [{"type": "command", "command": "countercheck hook session-start", "timeout": 10}]}
    ],
    "Stop": [
      {"hooks": [{"type": "command", "command": "countercheck hook goose-stop", "timeout": 600}]}
    ]
  }
}
"#
    );
    assert!(text(&o).contains("goose session"), "{}", text(&o));
    assert!(
        text(&o).contains(&format!("git add {HOOKS}")),
        "{}",
        text(&o)
    );
    let st = text(&hook(p, &["status"], &[]));
    assert!(
        st.lines().any(|l| l.starts_with("goose")
            && l.contains("project")
            && l.contains("installed, not committed")),
        "{st}"
    );
    let o = hook(p, &["install", "--agent", "goose", "--yes"], &[]);
    assert!(text(&o).contains("already"), "{}", text(&o));
}

#[test]
fn install_keeps_other_hooks_and_uninstall_takes_out_only_ours() {
    let d = project();
    let p = d.path();
    let file = p.join(HOOKS);
    fs::create_dir_all(file.parent().unwrap()).unwrap();
    let original = "{\n  \"hooks\": {\n    \"Stop\": [\n      {\"hooks\": [{\"type\": \"command\", \"command\": \"${PLUGIN_ROOT}/scripts/notify.sh\"}]}\n    ]\n  }\n}\n";
    fs::write(&file, original).unwrap();
    let o = hook(p, &["install", "--agent", "goose", "-y"], &[]);
    assert!(o.status.success(), "{}", text(&o));
    let s = fs::read_to_string(&file).unwrap();
    assert!(
        s.contains("${PLUGIN_ROOT}/scripts/notify.sh") && s.contains("hook goose-stop"),
        "{s}"
    );
    assert!(s.find("notify.sh") < s.find("goose-stop"), "{s}");
    let o = hook(p, &["uninstall", "--agent", "goose", "-y"], &[]);
    assert!(o.status.success(), "{}", text(&o));
    assert_eq!(fs::read_to_string(&file).unwrap(), original);
}

#[test]
fn user_scope_writes_the_plugin_in_the_home_folder() {
    let home = tempfile::tempdir().unwrap();
    let anywhere = tempfile::tempdir().unwrap();
    let envs = [("HOME", home.path()), ("USERPROFILE", home.path())];
    let o = hook(
        anywhere.path(),
        &["install", "--agent", "goose", "--scope", "user", "-y"],
        &envs,
    );
    assert!(o.status.success(), "{}", text(&o));
    let s = fs::read_to_string(home.path().join(HOOKS)).unwrap();
    assert!(
        s.contains("hook goose-stop") && s.contains("hook session-start"),
        "{s}"
    );
}
