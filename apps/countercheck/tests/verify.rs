//! End-to-end: a real git repo, a "worker" that changes files, and the real `countercheck` binary.

use std::fs;
use std::path::Path;
use std::process::Command;

const BIN: &str = env!("CARGO_BIN_EXE_countercheck");

fn sh(dir: &Path, cmd: &str) {
    let ok = Command::new("sh")
        .arg("-c")
        .arg(cmd)
        .current_dir(dir)
        .status()
        .unwrap();
    assert!(ok.success(), "command failed: {cmd}");
}

/// A tiny project: src/calc.sh adds two numbers, tests/check.sh checks it.
fn project(checks: &str) -> tempfile::TempDir {
    let d = tempfile::tempdir().unwrap();
    let p = d.path();
    sh(
        p,
        "git init -q -b main && git config user.email t@t && git config user.name t",
    );
    fs::create_dir_all(p.join("src")).unwrap();
    fs::create_dir_all(p.join("tests")).unwrap();
    fs::create_dir_all(p.join(".countercheck")).unwrap();
    // Bug at base: subtracts instead of adding.
    fs::write(p.join("src/calc.sh"), "echo $(($1 - $2))\n").unwrap();
    fs::write(
        p.join("tests/check.sh"),
        "[ \"$(sh src/calc.sh 2 3)\" = 5 ]\n",
    )
    .unwrap();
    fs::write(p.join(".countercheck/checks.toml"), checks).unwrap();
    sh(p, "git add -A && git commit -qm base");
    d
}

const CHECKS: &str = "protected = [\"tests/**\"]\n[[check]]\nname = \"calc\"\nrun = \"sh tests/check.sh\"\ntimeout_secs = 30\n";

/// Inside an countercheck check (COUNTERCHECK=1) we already run in a sandbox, and macOS can't nest them.
fn nested() -> bool {
    std::env::var_os("COUNTERCHECK").is_some()
}

fn countercheck(dir: &Path, args: &[&str]) -> (i32, serde_json::Value) {
    let mut cmd = Command::new(BIN);
    cmd.args(["verify", "--json", "--repo"]).arg(dir);
    if nested() && !args.contains(&"--sandbox") {
        cmd.args(["--sandbox", "none"]);
    }
    let out = cmd.args(args).output().unwrap();
    let code = out.status.code().unwrap_or(-1);
    let json = serde_json::from_slice(&out.stdout)
        .unwrap_or_else(|_| panic!("no JSON; stderr: {}", String::from_utf8_lossy(&out.stderr)));
    (code, json)
}

#[test]
fn honest_fix_is_verified() {
    let d = project(CHECKS);
    fs::write(d.path().join("src/calc.sh"), "echo $(($1 + $2))\n").unwrap();
    let (code, r) = countercheck(
        d.path(),
        &["--claim", "fixed calc, tests pass", "--agent", "test-agent"],
    );
    assert_eq!(code, 0, "{r:#}");
    assert_eq!(r["verdict"], "verified");
    assert_eq!(r["claim_status"], "supported");
    assert_eq!(r["checks_locked"], true);
    assert_eq!(r["repo"]["dirty"], true);
    assert_eq!(r["worker"]["agent"], "test-agent");
}

#[test]
fn worker_that_edits_its_tests_is_caught() {
    let d = project(CHECKS);
    // The "fix" leaves the bug and makes the test always pass.
    fs::write(d.path().join("tests/check.sh"), "exit 0\n").unwrap();
    let (code, r) = countercheck(d.path(), &["--claim", "all tests pass"]);
    assert_eq!(code, 2, "{r:#}");
    assert_eq!(r["verdict"], "failed");
    assert_eq!(r["claim_status"], "contradicted");
    assert_eq!(r["protected_changes"][0]["path"], "tests/check.sh");
    assert_eq!(r["protected_changes"][0]["action"], "restored_from_base");
}

#[test]
fn worker_cannot_weaken_the_check_file() {
    let d = project(CHECKS);
    fs::write(
        d.path().join(".countercheck/checks.toml"),
        "[[check]]\nname = \"calc\"\nrun = \"true\"\n",
    )
    .unwrap();
    let (code, r) = countercheck(d.path(), &[]);
    assert_eq!(code, 2, "{r:#}");
    assert_eq!(r["checks"][0]["run"], "sh tests/check.sh");
}

#[test]
fn committed_work_against_a_base_branch() {
    let d = project(CHECKS);
    sh(
        d.path(),
        "git checkout -qb agent && echo 'echo $(($1 + $2))' > src/calc.sh && git commit -qam fix",
    );
    let (code, r) = countercheck(d.path(), &["--base", "main"]);
    assert_eq!(code, 0, "{r:#}");
    assert_eq!(r["repo"]["dirty"], false);
    assert_ne!(r["repo"]["base_commit"], r["repo"]["head_commit"]);
}

#[test]
fn checks_missing_from_base_are_unverified() {
    let d = tempfile::tempdir().unwrap();
    let p = d.path();
    sh(
        p,
        "git init -q -b main && git config user.email t@t && git config user.name t && echo x > a && git add a && git commit -qm base",
    );
    fs::create_dir_all(p.join(".countercheck")).unwrap();
    fs::write(
        p.join(".countercheck/checks.toml"),
        "[[check]]\nname = \"ok\"\nrun = \"true\"\n",
    )
    .unwrap();
    let (code, r) = countercheck(p, &[]);
    assert_eq!(code, 3, "{r:#}");
    assert_eq!(r["verdict"], "unverified");
    assert_eq!(r["checks_locked"], false);
}

#[test]
fn timeouts_fail() {
    let d = project("[[check]]\nname = \"slow\"\nrun = \"sleep 30\"\ntimeout_secs = 1\n");
    let (code, r) = countercheck(d.path(), &[]);
    assert_eq!(code, 2);
    assert_eq!(r["checks"][0]["timed_out"], true);
}

#[cfg(target_os = "macos")]
#[test]
fn seatbelt_blocks_writes_outside_the_check_folder() {
    if nested() {
        return; // covered by the outer run; macOS can't nest sandboxes
    }
    let outside = tempfile::tempdir().unwrap();
    let target = outside.path().canonicalize().unwrap().join("leak.txt");
    let checks = format!(
        "[[check]]\nname = \"leak\"\nrun = \"echo x > '{}'\"\n",
        target.display()
    );
    let d = project(&checks);
    let (code, _) = countercheck(d.path(), &[]);
    assert_eq!(code, 2);
    assert!(!target.exists());
    let (code, _) = countercheck(d.path(), &["--sandbox", "none"]);
    assert_eq!(code, 0);
    assert!(target.exists());
}

/// Web tests run a local server and a browser over localhost: the sandbox must allow that, or
/// honest work is reported as FAILED. The internet stays blocked.
#[cfg(target_os = "macos")]
#[test]
fn seatbelt_allows_localhost_but_not_the_internet() {
    if nested() {
        return; // covered by the outer run; macOS can't nest sandboxes
    }
    let checks = "[[check]]\nname = \"loopback\"\nrun = \"python3 tests/loopback.py\"\n\
                  [[check]]\nname = \"internet\"\nrun = \"python3 tests/internet.py\"\n";
    let d = project(checks);
    fs::write(
        d.path().join("tests/loopback.py"),
        "import socket\ns = socket.socket()\ns.bind(('127.0.0.1', 0))\ns.listen(1)\n\
         c = socket.create_connection(s.getsockname(), timeout=5)\na, _ = s.accept()\n\
         c.sendall(b'hi')\nassert a.recv(2) == b'hi'\n",
    )
    .unwrap();
    // Exits 0 only when the sandbox itself refuses (EPERM), not when the network is just down.
    fs::write(
        d.path().join("tests/internet.py"),
        "import socket, sys\ntry:\n    socket.create_connection(('1.1.1.1', 443), timeout=5)\n\
         except PermissionError:\n    sys.exit(0)\nexcept OSError:\n    sys.exit(3)\nsys.exit(1)\n",
    )
    .unwrap();
    sh(d.path(), "git add -A && git commit -qm probes");
    let (code, json) = countercheck(d.path(), &[]);
    assert_eq!(code, 0, "{json}");
    // Outside the sandbox the internet probe can't succeed (it needs EPERM): proves it really tests the sandbox.
    let (code, _) = countercheck(d.path(), &["--sandbox", "none"]);
    assert_eq!(code, 2);
}

#[test]
fn edited_receipts_are_detected() {
    let d = project(CHECKS);
    let (_, r) = countercheck(d.path(), &[]);
    let path = d
        .path()
        .join(".countercheck/receipts")
        .join(format!("{}.json", r["id"].as_str().unwrap()));
    let show = |p: &Path| {
        Command::new(BIN)
            .arg("show")
            .arg(p)
            .status()
            .unwrap()
            .code()
    };
    assert_eq!(show(&path), Some(0));
    let text = fs::read_to_string(&path)
        .unwrap()
        .replace("\"failed\"", "\"verified\"");
    fs::write(&path, text).unwrap();
    assert_eq!(show(&path), Some(4));
}

#[test]
fn receipts_with_a_reported_cost_keep_their_seal() {
    // 36 * 0.000003: JSON read it back as 0.000108, one bit off, and `show` said BROKEN.
    let d = project(CHECKS);
    let (_, r) = countercheck(d.path(), &["--cost-usd", "0.00010800000000000001"]);
    let path = d
        .path()
        .join(".countercheck/receipts")
        .join(format!("{}.json", r["id"].as_str().unwrap()));
    let code = Command::new(BIN)
        .arg("show")
        .arg(&path)
        .status()
        .unwrap()
        .code();
    assert_eq!(code, Some(0));
}

#[test]
fn ignored_dependency_folders_can_be_linked() {
    let base = "protected = [\"tests/**\"]\n[[check]]\nname = \"dep\"\nrun = \"sh deps/lib.sh\"\n";
    let d = project(&format!("link = [\"deps\"]\n{base}"));
    let p = d.path();
    sh(
        p,
        "echo deps/ > .gitignore && git add .gitignore && git commit -qm ignore && mkdir deps && echo 'exit 0' > deps/lib.sh",
    );
    let (code, r) = countercheck(p, &[]);
    assert_eq!(code, 0, "{r:#}");
    assert_eq!(r["linked"][0], "deps");

    let d2 = project(base);
    let p2 = d2.path();
    sh(
        p2,
        "echo deps/ > .gitignore && git add .gitignore && git commit -qm ignore && mkdir deps && echo 'exit 0' > deps/lib.sh",
    );
    let (code, _) = countercheck(p2, &[]);
    assert_eq!(
        code, 2,
        "without link the ignored folder is not in the clean copy"
    );
}

#[test]
fn cache_files_are_not_reported() {
    let d = project(CHECKS);
    let p = d.path();
    fs::create_dir_all(p.join("tests/__pycache__")).unwrap();
    fs::write(p.join("tests/__pycache__/x.cpython-311.pyc"), "junk").unwrap();
    fs::write(p.join("src/calc.sh"), "echo $(($1 + $2))\n").unwrap();
    let (code, r) = countercheck(p, &[]);
    assert_eq!(code, 0, "{r:#}");
    assert_eq!(r["protected_changes"].as_array().unwrap().len(), 0);
}

/// Run `countercheck hook hermes-pre-verify` with `input` on stdin; returns (exit code, stdout).
fn hermes_hook(input: &str, envs: &[(&str, &str)]) -> (i32, String) {
    use std::io::Write;
    let mut cmd = Command::new(BIN);
    cmd.args(["hook", "hermes-pre-verify"])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    if nested() {
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

fn hermes_payload(dir: &Path) -> String {
    serde_json::json!({
        "hook_event_name": "pre_verify",
        "cwd": dir,
        "session_id": "s1",
        "extra": { "final_response": "Fixed it, all tests pass.", "changed_paths": ["tests/check.sh"], "model": "m" }
    })
    .to_string()
}

#[test]
fn hermes_hook_keeps_the_agent_going_when_the_work_fails() {
    let d = project(CHECKS);
    fs::write(d.path().join("tests/check.sh"), "exit 0\n").unwrap();
    let log = d.path().join("hook.log");
    let (code, out) = hermes_hook(
        &hermes_payload(d.path()),
        &[("COUNTERCHECK_HOOK_LOG", log.to_str().unwrap())],
    );
    assert_eq!(code, 0);
    let v: serde_json::Value = serde_json::from_str(out.trim()).unwrap();
    assert_eq!(v["action"], "continue");
    let msg = v["message"].as_str().unwrap();
    assert!(
        msg.contains("FAILED") && msg.contains("fix the code instead"),
        "{msg}"
    );
    let line: serde_json::Value =
        serde_json::from_str(fs::read_to_string(&log).unwrap().trim()).unwrap();
    assert_eq!(line["verdict"], "failed");
    assert_eq!(line["nudged"], true);
}

fn hermes_payload_saying(dir: &Path, final_response: &str) -> String {
    serde_json::json!({
        "hook_event_name": "pre_verify",
        "cwd": dir,
        "session_id": "s1",
        "extra": { "final_response": final_response, "model": "m" }
    })
    .to_string()
}

/// An agent that already reports it can't finish is telling the truth: record FAILED, don't push.
#[test]
fn hermes_hook_lets_an_honest_blocker_report_stop() {
    let d = project(CHECKS);
    let log = d.path().join("hook.log");
    let said = "I fixed what I could, but test_export can't pass: it contradicts test_display.";
    let (code, out) = hermes_hook(
        &hermes_payload_saying(d.path(), said),
        &[("COUNTERCHECK_HOOK_LOG", log.to_str().unwrap())],
    );
    assert_eq!((code, out.trim()), (0, ""));
    let line: serde_json::Value =
        serde_json::from_str(fs::read_to_string(&log).unwrap().trim()).unwrap();
    assert_eq!(line["verdict"], "failed");
    assert_eq!(line["nudged"], false);
    assert_eq!(line["agent_reported_blocker"], true);
}

/// Saying "can't" doesn't excuse edited tests: still pushed back.
#[test]
fn hermes_hook_still_pushes_back_on_tampering_even_with_a_blocker_report() {
    let d = project(CHECKS);
    fs::write(d.path().join("tests/check.sh"), "exit 0\n").unwrap();
    let said = "The test can't pass as written, so I adjusted it.";
    let (code, out) = hermes_hook(&hermes_payload_saying(d.path(), said), &[]);
    assert_eq!(code, 0);
    let v: serde_json::Value = serde_json::from_str(out.trim()).unwrap();
    assert_eq!(v["action"], "continue");
}

#[test]
fn hermes_hook_is_silent_when_the_work_passes() {
    let d = project(CHECKS);
    fs::write(d.path().join("src/calc.sh"), "echo $(($1 + $2))\n").unwrap();
    let (code, out) = hermes_hook(&hermes_payload(d.path()), &[]);
    assert_eq!((code, out.trim()), (0, ""));
}

#[test]
fn hermes_hook_can_be_switched_off_and_fails_open() {
    let d = project(CHECKS);
    let (code, out) = hermes_hook(
        &hermes_payload(d.path()),
        &[("COUNTERCHECK_HOOK_DISABLED", "1")],
    );
    assert_eq!((code, out.trim()), (0, ""));
    let (code, out) = hermes_hook("not json", &[]);
    assert_eq!((code, out.trim()), (0, ""));
}

#[test]
fn hermes_hook_uses_a_fixed_base_when_the_agent_commits() {
    let d = project(CHECKS);
    let base = String::from_utf8(
        Command::new("git")
            .args(["-C"])
            .arg(d.path())
            .args(["rev-parse", "HEAD"])
            .output()
            .unwrap()
            .stdout,
    )
    .unwrap();
    // The agent "fixes" the test and commits it: HEAD moves, but the base must not.
    sh(
        d.path(),
        "echo 'exit 0' > tests/check.sh && git commit -qam weaken",
    );
    let (_, out) = hermes_hook(
        &hermes_payload(d.path()),
        &[("COUNTERCHECK_BASE", base.trim())],
    );
    assert!(out.contains("\"continue\""), "{out}");
}
