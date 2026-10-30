//! `countercheck hook install / uninstall / status` (CC-05), end to end with the real binary.

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

/// A repository with committed checks that pass: `src/ok.sh` exits 0.
fn project() -> tempfile::TempDir {
    let d = tempfile::tempdir().unwrap();
    let p = d.path();
    sh(p, common::GIT_INIT);
    fs::create_dir_all(p.join(".countercheck")).unwrap();
    fs::write(
        p.join(".countercheck/checks.toml"),
        "[[check]]\nname = \"ok\"\nrun = \"true\"\n",
    )
    .unwrap();
    sh(p, "git add -A && git commit -qm base");
    d
}

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

fn verify(dir: &Path) -> serde_json::Value {
    let mut c = Command::new(BIN);
    c.args(["verify", "--json", "--repo"]).arg(dir);
    if std::env::var_os("COUNTERCHECK").is_some() {
        c.args(["--sandbox", "none"]);
    }
    serde_json::from_slice(&c.output().unwrap().stdout).unwrap()
}

#[test]
fn installed_hooks_are_the_ones_the_tamper_check_guards() {
    let d = project();
    let p = d.path();
    let o = hook(p, &["install", "--agent", "claude-code", "--yes"], &[]);
    assert!(o.status.success(), "{}", text(&o));
    let s = fs::read_to_string(p.join(".claude/settings.json")).unwrap();
    assert!(s.contains("\"countercheck hook claude-stop\""), "{s}");
    assert!(s.contains("\"countercheck hook session-start\""), "{s}");
    assert!(text(&o).contains("commit it"), "{}", text(&o));
    // Not committed yet: status says so.
    let st = text(&hook(p, &["status"], &[]));
    assert!(st.contains("installed, not committed"), "{st}");
    sh(p, "git add -A && git commit -qm hooks");
    let st = text(&hook(p, &["status"], &[]));
    assert!(
        st.lines().any(|l| l.starts_with("claude-code")
            && l.contains("project")
            && l.contains("installed  ")),
        "{st}"
    );
    assert_eq!(verify(p)["verdict"], "verified");
    // Taken out again without committing: exactly what an agent switching its referee off does.
    let o = hook(p, &["uninstall", "--agent", "claude-code", "--yes"], &[]);
    assert!(o.status.success(), "{}", text(&o));
    let r = verify(p);
    assert_eq!(r["verdict"], "failed", "{r:#}");
    assert!(
        r["hook_settings"]
            .to_string()
            .contains(".claude/settings.json"),
        "{r:#}"
    );
}

#[test]
fn nothing_is_written_without_consent_or_on_a_dry_run() {
    let d = project();
    let p = d.path();
    let o = hook(p, &["install", "--agent", "claude-code"], &[]);
    assert!(!o.status.success());
    assert!(text(&o).contains("re-run with --yes"), "{}", text(&o));
    assert!(
        text(&o).contains("+      {\"hooks\""),
        "the change is shown: {}",
        text(&o)
    );
    let o = hook(p, &["install", "--agent", "claude-code", "--dry-run"], &[]);
    assert!(o.status.success());
    assert!(!p.join(".claude/settings.json").exists());
}

#[test]
fn other_settings_stay_as_written_and_the_original_is_kept() {
    let d = project();
    let p = d.path();
    fs::create_dir_all(p.join(".claude")).unwrap();
    let original = "{\n  \"model\": \"opus\",\n  \"permissions\": {\"allow\": [\"Bash(ls)\"]},\n  \"hooks\": {\"Stop\": [{\"hooks\": [{\"type\": \"command\", \"command\": \"say done\"}]}]}\n}\n";
    fs::write(p.join(".claude/settings.json"), original).unwrap();
    let o = hook(p, &["install", "--agent", "claude-code", "-y"], &[]);
    assert!(o.status.success(), "{}", text(&o));
    let s = fs::read_to_string(p.join(".claude/settings.json")).unwrap();
    assert!(
        s.starts_with(
            "{\n  \"model\": \"opus\",\n  \"permissions\": {\"allow\": [\"Bash(ls)\"]},\n"
        ),
        "{s}"
    );
    assert!(s.contains("say done"), "{s}");
    assert_eq!(
        fs::read_to_string(p.join(".claude/settings.json.countercheck-backup")).unwrap(),
        original
    );
    // A second install adds nothing; neither does one where the `hook stop` alias is in place.
    let o = hook(p, &["install", "--agent", "claude-code", "-y"], &[]);
    assert!(text(&o).contains("already"), "{}", text(&o));
    let alias = s.replace("hook claude-stop", "hook stop");
    fs::write(p.join(".claude/settings.json"), &alias).unwrap();
    let o = hook(p, &["install", "--agent", "claude-code", "-y"], &[]);
    assert!(text(&o).contains("already"), "{}", text(&o));
    // Uninstall takes out only ours.
    hook(p, &["uninstall", "--agent", "claude-code", "-y"], &[]);
    let s = fs::read_to_string(p.join(".claude/settings.json")).unwrap();
    assert!(s.contains("say done") && !s.contains("countercheck"), "{s}");
}

#[test]
fn a_broken_or_linked_settings_file_is_left_alone() {
    let d = project();
    let p = d.path();
    fs::create_dir_all(p.join(".claude")).unwrap();
    fs::write(p.join(".claude/settings.json"), "{\"a\": 1,}").unwrap();
    let o = hook(p, &["install", "--agent", "claude-code", "-y"], &[]);
    assert!(!o.status.success());
    assert!(text(&o).contains("not valid JSON"), "{}", text(&o));
    assert_eq!(
        fs::read_to_string(p.join(".claude/settings.json")).unwrap(),
        "{\"a\": 1,}"
    );
    #[cfg(unix)]
    {
        fs::remove_file(p.join(".claude/settings.json")).unwrap();
        let elsewhere = tempfile::tempdir().unwrap();
        fs::write(elsewhere.path().join("s.json"), "{}").unwrap();
        std::os::unix::fs::symlink(
            elsewhere.path().join("s.json"),
            p.join(".claude/settings.json"),
        )
        .unwrap();
        let o = hook(p, &["install", "--agent", "claude-code", "-y"], &[]);
        assert!(text(&o).contains("is a symlink"), "{}", text(&o));
        assert_eq!(
            fs::read_to_string(elsewhere.path().join("s.json")).unwrap(),
            "{}"
        );
    }
}

#[test]
fn user_scope_writes_in_the_home_folder() {
    let home = tempfile::tempdir().unwrap();
    let anywhere = tempfile::tempdir().unwrap();
    let envs = [("HOME", home.path()), ("USERPROFILE", home.path())];
    let o = hook(
        anywhere.path(),
        &["install", "--agent", "cortex-code", "--scope", "user", "-y"],
        &envs,
    );
    assert!(o.status.success(), "{}", text(&o));
    let s = fs::read_to_string(home.path().join(".snowflake/cortex/settings.json")).unwrap();
    assert!(s.contains("hook claude-stop"), "{s}");
}

#[test]
fn cortex_code_warns_when_claude_settings_already_run_the_check() {
    let d = project();
    let p = d.path();
    hook(p, &["install", "--agent", "claude-code", "-y"], &[]);
    let o = hook(p, &["install", "--agent", "cortex-code", "--dry-run"], &[]);
    assert!(text(&o).contains("checked twice"), "{}", text(&o));
}

#[test]
fn unknown_and_manual_agents_are_plain_errors() {
    let d = project();
    let o = hook(d.path(), &["install", "--agent", "nope", "-y"], &[]);
    assert!(!o.status.success());
    assert!(
        text(&o).contains("known: claude-code, cortex-code"),
        "{}",
        text(&o)
    );
    let o = hook(d.path(), &["install", "--agent", "hermes", "-y"], &[]);
    assert!(text(&o).contains("docs/hooks/hermes.md"), "{}", text(&o));
}

#[test]
fn a_countercheck_path_with_spaces_is_quoted() {
    let d = project();
    let p = d.path();
    let o = hook(
        p,
        &[
            "install",
            "--agent",
            "claude-code",
            "-y",
            "--bin",
            "C:/Program Files/countercheck/countercheck.exe",
        ],
        &[],
    );
    assert!(text(&o).contains("not on PATH"), "{}", text(&o));
    let s = fs::read_to_string(p.join(".claude/settings.json")).unwrap();
    assert!(
        s.contains(r#""\"C:/Program Files/countercheck/countercheck.exe\" hook claude-stop""#),
        "{s}"
    );
}

#[test]
fn managed_scope_writes_its_own_drop_in_file_with_a_full_path() {
    let d = project();
    let managed = tempfile::tempdir().unwrap();
    let envs = [("COUNTERCHECK_MANAGED_DIR", managed.path())];
    let bin = std::env::current_exe().unwrap();
    let bin = bin.to_string_lossy().to_string();
    let o = hook(
        d.path(),
        &[
            "install",
            "--agent",
            "claude-code",
            "--scope",
            "managed",
            "-y",
            "--bin",
            &bin,
        ],
        &envs,
    );
    assert!(o.status.success(), "{}", text(&o));
    let f = managed
        .path()
        .join("managed-settings.d/50-countercheck.json");
    let s = fs::read_to_string(&f).unwrap();
    assert!(
        s.contains("hook claude-stop") && s.contains("hook session-start"),
        "{s}"
    );
    assert!(text(&o).contains("can't be switched off"), "{}", text(&o));
    let st = text(&hook(d.path(), &["status"], &envs));
    assert!(
        st.lines().any(|l| l.starts_with("claude-code")
            && l.contains("managed")
            && l.contains("installed")),
        "{st}"
    );
    // Only Claude Code has managed settings.
    let o = hook(
        d.path(),
        &[
            "install",
            "--agent",
            "cortex-code",
            "--scope",
            "managed",
            "-y",
        ],
        &envs,
    );
    assert!(!o.status.success());
    assert!(
        text(&o).contains("Claude Code's managed settings"),
        "{}",
        text(&o)
    );
}

#[cfg(unix)]
#[test]
fn a_managed_folder_without_write_access_says_how_to_get_it() {
    use std::os::unix::fs::PermissionsExt;
    let d = project();
    let managed = tempfile::tempdir().unwrap();
    fs::set_permissions(managed.path(), fs::Permissions::from_mode(0o555)).unwrap();
    // Root can write anyway: nothing to test then.
    if fs::write(managed.path().join("probe"), "x").is_ok() {
        return;
    }
    let o = hook(
        d.path(),
        &[
            "install",
            "--agent",
            "claude-code",
            "--scope",
            "managed",
            "-y",
        ],
        &[("COUNTERCHECK_MANAGED_DIR", managed.path())],
    );
    fs::set_permissions(managed.path(), fs::Permissions::from_mode(0o755)).unwrap();
    assert!(!o.status.success());
    assert!(text(&o).contains("sudo"), "{}", text(&o));
}

#[test]
fn every_agents_installed_hook_is_guarded_against_removal() {
    for (agent, file) in [
        ("cursor", ".cursor/hooks.json"),
        ("codex", ".codex/hooks.json"),
        ("copilot", ".github/hooks/countercheck.json"),
        ("goose", ".agents/plugins/countercheck/hooks/hooks.json"),
        ("kiro", ".kiro/hooks/countercheck.json"),
    ] {
        let d = project();
        let p = d.path();
        let home = tempfile::tempdir().unwrap();
        let envs = [("HOME", home.path()), ("USERPROFILE", home.path())];
        let o = hook(p, &["install", "--agent", agent, "--yes"], &envs);
        assert!(o.status.success(), "{agent}: {}", text(&o));
        assert!(
            text(&o).contains("so Countercheck can tell if an agent removes the hook"),
            "{agent}: {}",
            text(&o)
        );
        sh(p, "git add -A && git commit -qm hooks");
        assert_eq!(verify(p)["verdict"], "verified", "{agent}");
        // The agent deletes its own hook file: FAILED, naming it.
        fs::remove_file(p.join(file)).unwrap();
        let r = verify(p);
        assert_eq!(r["verdict"], "failed", "{agent}: {r:#}");
        assert!(r["reasons"].to_string().contains(file), "{agent}: {r:#}");
    }
}

#[test]
fn a_goose_plugin_switched_off_in_project_settings_fails() {
    let d = project();
    let p = d.path();
    let home = tempfile::tempdir().unwrap();
    let envs = [("HOME", home.path()), ("USERPROFILE", home.path())];
    assert!(
        hook(p, &["install", "--agent", "goose", "--yes"], &envs)
            .status
            .success()
    );
    sh(p, "git add -A && git commit -qm hooks");
    fs::create_dir_all(p.join(".config/goose")).unwrap();
    fs::write(
        p.join(".config/goose/settings.local.json"),
        r#"{"disabledPlugins": ["countercheck"]}"#,
    )
    .unwrap();
    let r = verify(p);
    assert_eq!(r["verdict"], "failed", "{r:#}");
    assert!(
        r["reasons"].to_string().contains("disabledPlugins"),
        "{r:#}"
    );
}

/// The session record `hook session-start` writes names the agent that started the session, from
/// the shape of each agent's input (they all run the same command).
#[test]
fn session_start_records_which_agent_started_the_session() {
    use std::io::Write;
    let d = project();
    let p = d.path();
    let cwd = serde_json::to_string(&p.to_string_lossy()).unwrap();
    let cases = [
        (
            "claude",
            format!(
                r#"{{"session_id":"s-claude","cwd":{cwd},"hook_event_name":"SessionStart","transcript_path":"/home/u/.claude/projects/x/s.jsonl"}}"#
            ),
            "claude-code",
        ),
        (
            "cortex",
            format!(
                r#"{{"session_id":"s-cortex","cwd":{cwd},"hook_event_name":"SessionStart","transcript_path":"/home/u/.snowflake/cortex/conversations/s.jsonl"}}"#
            ),
            "cortex-code",
        ),
        (
            "codex",
            format!(
                r#"{{"session_id":"s-codex","cwd":{cwd},"hook_event_name":"SessionStart","transcript_path":"/home/u/.codex/sessions/2026/10/30/rollout-s.jsonl","source":"startup"}}"#
            ),
            "codex",
        ),
        (
            "cursor",
            format!(
                r#"{{"session_id":"s-cursor","conversation_id":"s-cursor","hook_event_name":"sessionStart","workspace_roots":[{cwd}]}}"#
            ),
            "cursor",
        ),
        (
            "copilot",
            format!(
                r#"{{"sessionId":"s-copilot","timestamp":1704614400000,"cwd":{cwd},"source":"new"}}"#
            ),
            "copilot",
        ),
        (
            "goose",
            format!(
                r#"{{"event":"SessionStart","session_id":"s-goose","matcher_context":null,"working_dir":{cwd}}}"#
            ),
            "goose",
        ),
        (
            "kiro",
            format!(r#"{{"hook_event_name":"agentSpawn","cwd":{cwd},"session_id":"s-kiro"}}"#),
            "kiro",
        ),
    ];
    for (name, payload, want) in cases {
        let mut c = Command::new(BIN)
            .args(["hook", "session-start"])
            .current_dir(p)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        c.stdin
            .take()
            .unwrap()
            .write_all(payload.as_bytes())
            .unwrap();
        let o = c.wait_with_output().unwrap();
        assert!(o.status.success(), "{name}: {}", text(&o));
        let rec = p.join(format!(".git/countercheck/sessions/s-{name}.json"));
        let v: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(&rec).unwrap_or_else(|e| panic!("{name}: {e}")),
        )
        .unwrap();
        assert_eq!(v["agent"], want, "{name}: {v}");
    }
}
