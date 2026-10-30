//! Amp: `countercheck hook amp-stop`, `hook session-start` with the plugin's input, `hook install
//! --agent amp` (a whole plugin file), and the tamper check on that file, end to end with the real
//! binary. Inputs are the fixtures in `tests/fixtures/amp/` (what the plugin sends; see the README
//! there). When `bun` is on PATH, the plugin itself is run too, with a stub of Amp's API.

use std::fs;
use std::path::Path;
use std::process::{Command, Output, Stdio};

mod common;

const BIN: &str = env!("CARGO_BIN_EXE_countercheck");

const STOP: &str = include_str!("fixtures/amp/stop.json");
const SESSION_START: &str = include_str!("fixtures/amp/session-start.json");
const TEMPLATE: &str = include_str!("../src/agents/amp/plugin.ts");
const PLUGIN: &str = ".amp/plugins/countercheck.ts";
const THREAD: &str = "T-019a3a6e-7c1d-7f20-9b4e-5d2c8a1f0e63";

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

/// A fixture with the project folder and the final message filled in (`None`: the turn had no text).
fn payload(fixture: &str, cwd: &Path, message: Option<&str>) -> String {
    let mut v: serde_json::Value = serde_json::from_str(fixture).unwrap();
    v["cwd"] = cwd.to_string_lossy().into_owned().into();
    if v.get("last_assistant_message").is_some() {
        v["last_assistant_message"] = message.map_or(serde_json::Value::Null, Into::into);
    }
    v.to_string()
}

fn with(input: &str, key: &str, value: &str) -> String {
    let mut v: serde_json::Value = serde_json::from_str(input).unwrap();
    v[key] = value.into();
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

fn amp_stop(dir: &Path, input: &str, envs: &[(&str, &str)]) -> (i32, String) {
    run_hook("amp-stop", dir, input, envs)
}

/// Amp may stop: no `action`, only the line the plugin shows the user (`message`).
fn lets_stop(out: &str) -> String {
    let v: serde_json::Value =
        serde_json::from_str(out.trim()).unwrap_or_else(|_| panic!("not one JSON object: {out}"));
    assert!(v.get("action").is_none(), "sent back: {out}");
    assert_eq!(v.as_object().unwrap().len(), 1, "only message: {out}");
    v["message"].as_str().unwrap().to_string()
}

/// The `userMessage` of `{"action":"continue","userMessage":...}` (Amp's `AgentEndResult`), or panic.
fn sent_back(out: &str) -> String {
    let v: serde_json::Value = serde_json::from_str(out.trim()).unwrap();
    assert_eq!(v["action"], "continue", "{out}");
    assert_eq!(v.as_object().unwrap().len(), 2, "{out}");
    v["userMessage"].as_str().unwrap().to_string()
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
fn verified_work_may_finish_and_the_receipt_says_amp() {
    let d = project();
    fix(d.path());
    let log = d.path().join("hook.log");
    let (code, out) = amp_stop(
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
    assert_eq!(receipt["worker"]["agent"], "amp");
    assert_eq!(receipt["worker"]["model"], "claude-opus-4-6");
    assert_eq!(receipt["worker"]["session"], THREAD);
}

#[test]
fn a_false_success_claim_is_sent_back_with_the_failing_check() {
    let d = project();
    let log = d.path().join("hook.log");
    let (code, out) = amp_stop(
        d.path(),
        &payload(STOP, d.path(), Some("Fixed it, all tests pass.")),
        &[("COUNTERCHECK_HOOK_LOG", log.to_str().unwrap())],
    );
    assert_eq!(code, 0);
    let reason = sent_back(&out);
    assert!(
        reason.contains("FAILED") && reason.contains("Failed check \"calc\""),
        "{reason}"
    );
    let (line, receipt) = last_log_receipt(&log);
    assert_eq!(line["nudged"], true);
    assert_eq!(line["gave_up"], false);
    assert_eq!(receipt["verdict"], "failed");
    assert_eq!(receipt["worker"]["agent"], "amp");
}

#[test]
fn editing_the_locked_test_is_still_sent_back() {
    let d = project();
    fs::write(d.path().join("tests/check.sh"), "exit 0\n").unwrap();
    let (code, out) = amp_stop(
        d.path(),
        &payload(STOP, d.path(), Some("All tests pass.")),
        &[],
    );
    assert_eq!(code, 0);
    assert!(sent_back(&out).contains("original versions"), "{out}");
}

#[test]
fn an_honest_blocker_may_finish() {
    let d = project();
    let said = "I fixed what I could, but test_export can't pass: it contradicts test_display.";
    let (code, out) = amp_stop(d.path(), &payload(STOP, d.path(), Some(said)), &[]);
    assert_eq!(code, 0);
    let msg = lets_stop(&out);
    assert!(
        msg.contains("FAILED") && msg.contains("says it is blocked"),
        "{msg}"
    );
}

/// A turn with no text (only tool calls) is no claim, never a pass.
#[test]
fn no_final_message_and_failing_checks_is_sent_back() {
    let d = project();
    let (code, out) = amp_stop(d.path(), &payload(STOP, d.path(), None), &[]);
    assert_eq!(code, 0);
    assert!(sent_back(&out).contains("\"calc\""), "{out}");
}

/// A turn the user cancelled, or that ended in an error, isn't the agent saying it is done: nothing
/// is checked and nothing is said.
#[test]
fn a_cancelled_or_failed_turn_is_not_checked() {
    let d = project();
    let log = d.path().join("hook.log");
    for status in ["cancelled", "error"] {
        let input = with(
            &payload(STOP, d.path(), Some("All tests pass.")),
            "status",
            status,
        );
        let out = amp_stop(
            d.path(),
            &input,
            &[("COUNTERCHECK_HOOK_LOG", log.to_str().unwrap())],
        );
        assert_eq!(out, (0, String::new()), "{status}");
    }
    assert!(!log.exists());
}

#[test]
fn after_the_push_back_limit_amp_may_finish_with_a_warning() {
    let d = project();
    let log = d.path().join("hook.log");
    let envs = [("COUNTERCHECK_HOOK_LOG", log.to_str().unwrap())];
    let input = payload(STOP, d.path(), Some("All tests pass."));
    for _ in 0..3 {
        let (code, out) = amp_stop(d.path(), &input, &envs);
        assert_eq!(code, 0);
        sent_back(&out);
    }
    let (code, out) = amp_stop(d.path(), &input, &envs);
    assert_eq!(code, 0);
    let msg = lets_stop(&out);
    assert!(
        msg.starts_with("⚠ Countercheck: FAILED") && msg.contains("did NOT pass"),
        "{msg}"
    );
    let (line, _) = last_log_receipt(&log);
    assert_eq!(line["gave_up"], true);
    // COUNTERCHECK_MAX_NUDGES changes the limit; another thread has its own count.
    let other = with(&input, "thread_id", "T-other");
    let (_, out) = amp_stop(d.path(), &other, &[("COUNTERCHECK_MAX_NUDGES", "0")]);
    assert!(lets_stop(&out).contains("did NOT pass"), "{out}");
}

#[test]
fn the_stop_hook_fails_open() {
    let d = project();
    let not_git = tempfile::tempdir().unwrap();
    // Garbage on stdin.
    assert_eq!(amp_stop(d.path(), "not json", &[]), (0, String::new()));
    // A folder that isn't a git repository.
    let input = payload(STOP, not_git.path(), Some("Done."));
    assert_eq!(amp_stop(not_git.path(), &input, &[]), (0, String::new()));
    // No cwd: the folder the plugin runs the hook in, here not a repository.
    let mut v: serde_json::Value = serde_json::from_str(&input).unwrap();
    v.as_object_mut().unwrap().remove("cwd");
    assert_eq!(
        amp_stop(not_git.path(), &v.to_string(), &[]),
        (0, String::new())
    );
    // Switched off.
    let input = payload(STOP, d.path(), Some("Done."));
    assert_eq!(
        amp_stop(d.path(), &input, &[("COUNTERCHECK_HOOK_DISABLED", "1")]),
        (0, String::new())
    );
}

/// Without a workspace, the plugin sends the folder it runs in: `.amp/plugins/`, inside the project.
#[test]
fn a_cwd_inside_the_project_checks_the_project() {
    let d = project();
    fs::create_dir_all(d.path().join(".amp/plugins")).unwrap();
    let inside = d.path().join(".amp/plugins");
    let (_, out) = amp_stop(&inside, &payload(STOP, &inside, Some("Done.")), &[]);
    assert!(sent_back(&out).contains("\"calc\""), "{out}");
}

/// The plugin's `session.start` input (`session_id` = the thread id, `cwd`) locks the checks to the
/// starting commit, and the record names Amp.
#[test]
fn session_start_locks_the_checks_against_a_committed_edit() {
    let d = project();
    let p = d.path();
    let (code, out) = run_hook("session-start", p, &payload(SESSION_START, p, None), &[]);
    assert_eq!((code, out.trim()), (0, ""));
    let record = p.join(format!(".git/countercheck/sessions/{THREAD}.json"));
    let rec: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(record).unwrap()).unwrap();
    assert_eq!(rec["base"].as_str().unwrap().len(), 40, "{rec}");
    assert_eq!(rec["agent"], "amp", "{rec}");
    fs::write(
        p.join(".countercheck/checks.toml"),
        "[[check]]\nname = \"calc\"\nrun = \"true\"\n",
    )
    .unwrap();
    sh(p, "git commit -qam 'simpler checks'");
    let (_, out) = amp_stop(p, &payload(STOP, p, Some("All tests pass.")), &[]);
    assert!(sent_back(&out).contains("\"calc\""), "{out}");
}

// ---- the plugin file ----

/// The plugin `hook install` writes with `--bin <bin>`.
fn plugin_for(bin: &str) -> String {
    TEMPLATE.replace(
        "\"__COUNTERCHECK_BIN__\"",
        &serde_json::to_string(bin).unwrap(),
    )
}

/// Agent text reaches Countercheck only as JSON on stdin: the plugin runs no shell.
#[test]
fn the_plugin_passes_data_on_stdin_and_runs_no_shell() {
    assert!(TEMPLATE.contains("spawn(BIN, ['hook', sub]"), "{TEMPLATE}");
    assert!(TEMPLATE.contains("JSON.stringify(input)"));
    for shell in ["amp.$", "ctx.$", "exec(", "execSync", "shell:", "Bun.$"] {
        assert!(!TEMPLATE.contains(shell), "the plugin uses {shell}");
    }
    // `hook install` and `hook uninstall` recognize it as Countercheck's by this.
    assert!(TEMPLATE.contains("countercheck hook amp-stop"));
    assert_eq!(TEMPLATE.matches("\"__COUNTERCHECK_BIN__\"").count(), 1);
}

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

fn status_line(p: &Path, home: &Path, scope: &str) -> String {
    let st = text(&hook(p, &["status"], home));
    st.lines()
        .find(|l| l.starts_with("amp ") && l.split_whitespace().nth(1) == Some(scope))
        .unwrap_or_else(|| panic!("no amp {scope} line: {st}"))
        .to_string()
}

#[test]
fn install_writes_the_embedded_plugin_and_status_reports_it() {
    let d = project();
    let home = tempfile::tempdir().unwrap();
    let p = d.path();
    assert!(status_line(p, home.path(), "project").contains("not installed"));
    let o = hook(p, &["install", "--agent", "amp", "--yes"], home.path());
    assert!(o.status.success(), "{}", text(&o));
    assert_eq!(
        fs::read_to_string(p.join(PLUGIN)).unwrap(),
        plugin_for("countercheck")
    );
    let t = text(&o);
    assert!(
        t.contains("plugins: reload") && t.contains("docs/hooks/amp.md"),
        "{t}"
    );
    assert!(
        t.contains("so Countercheck can tell if an agent removes the hook"),
        "{t}"
    );
    // Nothing went to the user's folder.
    assert!(!home.path().join(".config").exists());
    assert!(
        status_line(p, home.path(), "project").contains("installed, not committed"),
        "{}",
        status_line(p, home.path(), "project")
    );
    sh(p, "git add -A && git commit -qm hooks");
    assert!(status_line(p, home.path(), "project").contains("installed  "));
    // A second install writes nothing.
    let o = hook(p, &["install", "--agent", "amp", "--yes"], home.path());
    assert!(text(&o).contains("already"), "{}", text(&o));
    // An edited copy is reported, and a new install puts ours back.
    fs::write(p.join(PLUGIN), TEMPLATE.replace("15 * 60", "1")).unwrap();
    assert!(status_line(p, home.path(), "project").contains("edited"));
    let o = hook(p, &["install", "--agent", "amp", "--yes"], home.path());
    assert!(o.status.success(), "{}", text(&o));
    assert_eq!(
        fs::read_to_string(p.join(PLUGIN)).unwrap(),
        plugin_for("countercheck")
    );
}

/// `--bin` lands in the plugin as a string literal: spaces and backslashes stay one argument.
#[test]
fn install_with_bin_writes_it_as_a_string() {
    let d = project();
    let home = tempfile::tempdir().unwrap();
    let bin = r"C:\Program Files\countercheck\countercheck.exe";
    let o = hook(
        d.path(),
        &["install", "--agent", "amp", "--bin", bin, "--yes"],
        home.path(),
    );
    assert!(o.status.success(), "{}", text(&o));
    let s = fs::read_to_string(d.path().join(PLUGIN)).unwrap();
    assert!(
        s.contains(r#"const BIN = "C:\\Program Files\\countercheck\\countercheck.exe""#),
        "{s}"
    );
}

#[test]
fn install_refuses_a_plugin_that_isnt_countercheck_s() {
    let d = project();
    let home = tempfile::tempdir().unwrap();
    let p = d.path();
    fs::create_dir_all(p.join(".amp/plugins")).unwrap();
    let theirs = "export default function (amp) { amp.logger.log('mine') }\n";
    fs::write(p.join(PLUGIN), theirs).unwrap();
    let o = hook(p, &["install", "--agent", "amp", "--yes"], home.path());
    assert!(!o.status.success());
    assert!(
        text(&o).contains("isn't Countercheck's plugin"),
        "{}",
        text(&o)
    );
    assert_eq!(fs::read_to_string(p.join(PLUGIN)).unwrap(), theirs);
    assert!(status_line(p, home.path(), "project").contains("another plugin"));
    // Uninstall leaves it alone too.
    let o = hook(p, &["uninstall", "--agent", "amp", "--yes"], home.path());
    assert!(o.status.success(), "{}", text(&o));
    assert_eq!(fs::read_to_string(p.join(PLUGIN)).unwrap(), theirs);
}

#[test]
fn uninstall_removes_the_plugin_only_if_unchanged() {
    let d = project();
    let home = tempfile::tempdir().unwrap();
    let p = d.path();
    assert!(
        hook(p, &["install", "--agent", "amp", "--yes"], home.path())
            .status
            .success()
    );
    // Edited: kept, with an error.
    let edited = TEMPLATE.replace("15 * 60", "1");
    fs::write(p.join(PLUGIN), &edited).unwrap();
    let o = hook(p, &["uninstall", "--agent", "amp", "--yes"], home.path());
    assert!(!o.status.success());
    assert!(text(&o).contains("differs"), "{}", text(&o));
    assert_eq!(fs::read_to_string(p.join(PLUGIN)).unwrap(), edited);
    // Exactly ours: removed.
    fs::write(p.join(PLUGIN), plugin_for("countercheck")).unwrap();
    let o = hook(p, &["uninstall", "--agent", "amp", "--yes"], home.path());
    assert!(o.status.success(), "{}", text(&o));
    assert!(!p.join(PLUGIN).exists());
    assert!(status_line(p, home.path(), "project").contains("not installed"));
}

#[test]
fn install_for_the_user_writes_under_the_home_folder() {
    let d = project();
    let home = tempfile::tempdir().unwrap();
    let o = hook(
        d.path(),
        &["install", "--agent", "amp", "--scope", "user", "-y"],
        home.path(),
    );
    assert!(o.status.success(), "{}", text(&o));
    assert_eq!(
        fs::read_to_string(home.path().join(".config/amp/plugins/countercheck.ts")).unwrap(),
        plugin_for("countercheck")
    );
    assert!(!d.path().join(".amp").exists());
    assert!(status_line(d.path(), home.path(), "user").contains("installed"));
    let o = hook(
        d.path(),
        &["install", "--agent", "amp", "--scope", "managed", "-y"],
        home.path(),
    );
    assert!(!o.status.success());
    assert!(text(&o).contains("--scope user or project"), "{}", text(&o));
}

fn verify(dir: &Path) -> serde_json::Value {
    let mut c = Command::new(BIN);
    c.args(["verify", "--json", "--repo"]).arg(dir);
    if std::env::var_os("COUNTERCHECK").is_some() {
        c.args(["--sandbox", "none"]);
    }
    serde_json::from_slice(&c.output().unwrap().stdout).unwrap()
}

/// The committed plugin is compared byte for byte: deleting it or editing it makes the run FAILED,
/// naming the file, though the checks pass.
#[test]
fn deleting_or_editing_the_committed_plugin_fails_the_run() {
    let d = project();
    let home = tempfile::tempdir().unwrap();
    let p = d.path();
    fix(p);
    assert!(
        hook(p, &["install", "--agent", "amp", "--yes"], home.path())
            .status
            .success()
    );
    sh(p, "git add -A && git commit -qm 'fix, and the hook'");
    assert_eq!(verify(p)["verdict"], "verified");
    fs::remove_file(p.join(PLUGIN)).unwrap();
    let r = verify(p);
    assert_eq!(r["verdict"], "failed", "{r:#}");
    assert!(r["reasons"].to_string().contains(PLUGIN), "{r:#}");
    // One changed character in the code counts too: here the hook's own name.
    fs::write(
        p.join(PLUGIN),
        plugin_for("countercheck").replace("'amp-stop'", "'amp-stap'"),
    )
    .unwrap();
    let r = verify(p);
    assert_eq!(r["verdict"], "failed", "{r:#}");
    assert!(r["reasons"].to_string().contains(PLUGIN), "{r:#}");
    assert!(
        r["reasons"]
            .to_string()
            .contains("changes the Countercheck plugin"),
        "{r:#}"
    );
}

// ---- the plugin itself, under Bun ----

fn bun() -> Option<std::path::PathBuf> {
    let paths = std::env::var_os("PATH")?;
    let exe = if cfg!(windows) { "bun.exe" } else { "bun" };
    std::env::split_paths(&paths)
        .map(|d| d.join(exe))
        .find(|p| p.is_file())
}

/// Load the installed plugin with a stub of Amp's API and fire `agent.end` (fixtures/amp/drive.ts):
/// what it returned to Amp, and what it showed the user.
fn drive(bun: &Path, plugin: &Path, root: &Path, thread: &str, message: &str) -> serde_json::Value {
    let driver = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/amp/drive.ts");
    let mut c = Command::new(bun);
    c.arg(driver)
        .arg(plugin)
        .arg(root)
        .arg(message)
        .env("THREAD", thread)
        .current_dir(root);
    if std::env::var_os("COUNTERCHECK").is_some() {
        c.env("COUNTERCHECK_SANDBOX", "none");
    }
    let o = c.output().unwrap();
    assert!(o.status.success(), "{}", text(&o));
    serde_json::from_slice(&o.stdout).unwrap_or_else(|_| panic!("{}", text(&o)))
}

/// The real plugin, run by Bun as Amp runs it: a false claim returns Amp's `continue` answer, passing
/// work shows the VERIFIED line, and a countercheck that can't start lets Amp stop. Skipped (with a
/// note) when `bun` isn't on PATH.
#[test]
fn the_plugin_sends_amp_back_and_fails_open() {
    let Some(bun) = bun() else {
        eprintln!("skipped: bun is not on PATH");
        return;
    };
    let d = project();
    let home = tempfile::tempdir().unwrap();
    let p = d.path();
    let o = hook(
        p,
        &["install", "--agent", "amp", "--bin", BIN, "--yes"],
        home.path(),
    );
    assert!(o.status.success(), "{}", text(&o));
    sh(p, "git add -A && git commit -qm hooks");
    let plugin = p.join(PLUGIN);
    let r = drive(&bun, &plugin, p, "T-1", "All tests pass.");
    assert_eq!(r["result"]["action"], "continue", "{r:#}");
    assert!(
        r["result"]["userMessage"]
            .as_str()
            .unwrap()
            .contains("Failed check \"calc\""),
        "{r:#}"
    );
    fix(p);
    let r = drive(&bun, &plugin, p, "T-2", "Done: calc adds.");
    assert!(r.get("result").is_none(), "{r:#}");
    assert!(
        r["notes"][0]
            .as_str()
            .unwrap()
            .starts_with("✓ Countercheck: VERIFIED"),
        "{r:#}"
    );
    // A countercheck that isn't there: nothing returned, Amp stops.
    let broken = p.join("broken.ts");
    fs::write(
        &broken,
        fs::read_to_string(&plugin).unwrap().replace(
            &serde_json::to_string(BIN).unwrap(),
            "\"/nonexistent/countercheck\"",
        ),
    )
    .unwrap();
    let r = drive(&bun, &broken, p, "T-3", "All tests pass.");
    assert!(r.get("result").is_none(), "{r:#}");
    assert!(r["logs"].to_string().contains("countercheck:"), "{r:#}");
}
