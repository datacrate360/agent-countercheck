//! End-to-end: a real git repo, a "worker" that changes files, and the real `countercheck` binary.

use std::fs;
use std::path::Path;
use std::process::Command;

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

/// A tiny project: src/calc.sh adds two numbers, tests/check.sh checks it.
fn project(checks: &str) -> tempfile::TempDir {
    let d = tempfile::tempdir().unwrap();
    let p = d.path();
    sh(p, common::GIT_INIT);
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

/// A test edit that makes the buggy calc pass.
const WEAK_TEST: &str = "exit 0\n";

#[test]
fn git_index_flags_do_not_hide_edited_tests() {
    // `assume-unchanged` and `skip-worktree` make git itself report the edited test as unchanged.
    for flag in ["--assume-unchanged", "--skip-worktree"] {
        let d = project(CHECKS);
        fs::write(d.path().join("tests/check.sh"), WEAK_TEST).unwrap();
        sh(
            d.path(),
            &format!("git update-index {flag} tests/check.sh src/calc.sh"),
        );
        let (code, r) = countercheck(d.path(), &[]);
        assert_eq!(code, 2, "{flag}: {r:#}");
        assert_eq!(r["protected_changes"][0]["path"], "tests/check.sh");
        assert_eq!(r["protected_changes"][0]["action"], "restored_from_base");
        assert_eq!(r["repo"]["dirty"], true, "{flag}");
    }
}

#[test]
fn a_git_clean_filter_does_not_hide_edited_tests() {
    // A clean filter that hands git the base text makes `git diff` see no change.
    let d = project(CHECKS);
    sh(
        d.path(),
        "git show HEAD:tests/check.sh > .git/base-copy && git config filter.same.clean 'cat .git/base-copy' \
         && echo 'tests/check.sh filter=same' > .git/info/attributes",
    );
    fs::write(d.path().join("tests/check.sh"), WEAK_TEST).unwrap();
    let (code, r) = countercheck(d.path(), &[]);
    assert_eq!(code, 2, "{r:#}");
    assert_eq!(r["protected_changes"][0]["path"], "tests/check.sh");
}

#[test]
fn replace_refs_do_not_change_a_pinned_base() {
    // `git replace` makes the base commit's id show the worker's commit, with its weak test and
    // weak check file.
    let d = project(CHECKS);
    let base = String::from_utf8(
        Command::new("git")
            .args(["rev-parse", "HEAD"])
            .current_dir(d.path())
            .output()
            .unwrap()
            .stdout,
    )
    .unwrap()
    .trim()
    .to_string();
    fs::write(d.path().join("tests/check.sh"), WEAK_TEST).unwrap();
    fs::write(
        d.path().join(".countercheck/checks.toml"),
        "[[check]]\nname = \"calc\"\nrun = \"true\"\n",
    )
    .unwrap();
    sh(
        d.path(),
        &format!("git commit -qam weak && git replace {base} HEAD"),
    );
    let (code, r) = countercheck(d.path(), &["--base", &base]);
    assert_eq!(code, 2, "{r:#}");
    assert_eq!(r["checks"][0]["run"], "sh tests/check.sh");
    assert_eq!(r["repo"]["base_commit"], base.as_str());
}

#[test]
fn a_rewritten_base_object_is_refused() {
    // The worker overwrites the base's own object file for the test with a weaker test's bytes.
    let d = project(CHECKS);
    let git = |args: &[&str]| {
        let out = Command::new("git")
            .args(args)
            .current_dir(d.path())
            .output()
            .unwrap();
        String::from_utf8(out.stdout).unwrap().trim().to_string()
    };
    let old = git(&["rev-parse", "HEAD:tests/check.sh"]);
    fs::write(d.path().join("tests/check.sh"), WEAK_TEST).unwrap();
    let new = git(&["hash-object", "-w", "tests/check.sh"]);
    let object = |id: &str| d.path().join(".git/objects").join(&id[..2]).join(&id[2..]);
    let mut perms = fs::metadata(object(&old)).unwrap().permissions();
    #[allow(clippy::permissions_set_readonly_false)]
    perms.set_readonly(false);
    fs::set_permissions(object(&old), perms).unwrap();
    fs::copy(object(&new), object(&old)).unwrap();
    assert_eq!(git(&["cat-file", "-p", &old]), WEAK_TEST.trim()); // git believes it
    let mut cmd = Command::new(BIN);
    cmd.args(["verify", "--json", "--repo"]).arg(d.path());
    if nested() {
        cmd.args(["--sandbox", "none"]);
    }
    let out = cmd.output().unwrap();
    assert_eq!(out.status.code(), Some(1));
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("does not match its id"), "{err}");
}

#[test]
fn protected_files_with_only_crlf_line_endings_are_not_reported() {
    // What `core.autocrlf` checkouts look like on Windows: the same test, CRLF line endings.
    let d = project(CHECKS);
    fs::write(d.path().join("src/calc.sh"), "echo $(($1 + $2))\n").unwrap();
    fs::write(
        d.path().join("tests/check.sh"),
        "[ \"$(sh src/calc.sh 2 3)\" = 5 ]\r\n",
    )
    .unwrap();
    let (code, r) = countercheck(d.path(), &[]);
    assert_eq!(code, 0, "{r:#}");
    assert_eq!(r["protected_changes"], serde_json::json!([]));
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
        &format!(
            "{} && echo x > a && git add a && git commit -qm base",
            common::GIT_INIT
        ),
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

/// A timed-out check's background children die with it (the whole process group is killed).
/// Unix only: it reads the child's state with `ps`; Windows has its own test below.
#[cfg(unix)]
#[test]
fn timeouts_kill_the_whole_process_group() {
    let outside = tempfile::tempdir().unwrap();
    let pid_file = outside.path().join("bg.pid");
    let checks = format!(
        "[[check]]\nname = \"bg\"\nrun = \"sleep 60 & echo $! > '{}'; wait\"\ntimeout_secs = 1\n",
        pid_file.display()
    );
    let d = project(&checks);
    let (code, r) = countercheck(d.path(), &["--sandbox", "none"]);
    assert_eq!(code, 2, "{r:#}");
    assert_eq!(r["checks"][0]["timed_out"], true);
    let pid = fs::read_to_string(&pid_file).unwrap();
    // Gone, or a zombie waiting to be reaped: either way it no longer runs.
    let running = || {
        let out = Command::new("ps")
            .args(["-o", "stat=", "-p", pid.trim()])
            .output()
            .unwrap();
        let stat = String::from_utf8_lossy(&out.stdout).trim().to_string();
        !stat.is_empty() && !stat.starts_with('Z')
    };
    for _ in 0..20 {
        if !running() {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    panic!("background process {} survived the timeout", pid.trim());
}

/// Unix: what a passing check leaves running in the background (a server, a `sleep &`) is
/// stopped when the check ends, not left running after verify.
#[cfg(unix)]
#[test]
fn background_processes_stop_when_a_check_ends() {
    let outside = tempfile::tempdir().unwrap();
    let pid_file = outside.path().join("bg.pid");
    let checks = format!(
        "[[check]]\nname = \"bg\"\nrun = \"(sleep 60 & echo $! > '{}'); exit 0\"\n",
        pid_file.display()
    );
    let d = project(&checks);
    let (code, r) = countercheck(d.path(), &["--sandbox", "none"]);
    assert_eq!(code, 0, "{r:#}");
    let pid = fs::read_to_string(&pid_file).unwrap();
    let running = || {
        let out = Command::new("ps")
            .args(["-o", "stat=", "-p", pid.trim()])
            .output()
            .unwrap();
        let stat = String::from_utf8_lossy(&out.stdout).trim().to_string();
        !stat.is_empty() && !stat.starts_with('Z')
    };
    for _ in 0..20 {
        if !running() {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    let _ = Command::new("kill").arg(pid.trim()).status();
    panic!(
        "background process {} is still running after verify",
        pid.trim()
    );
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
/// `network = true`: the check reaches the internet under the sandbox (EPERM would mean blocked),
/// while writes outside the check folder stay blocked; the receipt marks it.
#[cfg(target_os = "macos")]
#[test]
fn network_true_lets_a_check_use_the_internet_but_not_write_outside() {
    if nested() {
        return; // covered by the outer run; macOS can't nest sandboxes
    }
    let outside = tempfile::tempdir().unwrap();
    let target = outside.path().canonicalize().unwrap().join("leak.txt");
    let checks = format!(
        "[[check]]\nname = \"net\"\nrun = \"python3 tests/net.py\"\nnetwork = true\n\
         [[check]]\nname = \"write\"\nrun = \"echo x > {}\"\nnetwork = true\n",
        target.display()
    );
    let d = project(&checks);
    // Exits 0 when the connection isn't refused by the sandbox (EPERM); a plain network error
    // (offline machine) also counts as "not blocked by us".
    fs::write(
        d.path().join("tests/net.py"),
        "import socket, sys\ntry:\n    socket.create_connection(('1.1.1.1', 443), timeout=5)\n\
         except PermissionError:\n    sys.exit(1)\nexcept OSError:\n    pass\n",
    )
    .unwrap();
    sh(d.path(), "git add -A && git commit -qm probes");
    let (code, json) = countercheck(d.path(), &[]);
    assert_eq!(code, 2, "{json}");
    let checks = json["checks"].as_array().unwrap();
    assert_eq!(
        checks[0]["passed"], true,
        "internet blocked despite network = true: {json}"
    );
    assert_eq!(checks[0]["network"], true);
    assert_eq!(checks[1]["passed"], false, "write outside allowed: {json}");
    assert!(!target.exists());
}

/// A check that fails without internet gets a hint, but the verdict stays FAILED: a worker must
/// not be able to soften a failure by making a blocked network call on purpose.
#[cfg(target_os = "macos")]
#[test]
fn a_failed_check_without_internet_gets_a_hint_and_stays_failed() {
    if nested() {
        return;
    }
    let d = project("[[check]]\nname = \"fetch\"\nrun = \"python3 tests/fetch.py\"\n");
    fs::write(
        d.path().join("tests/fetch.py"),
        "import socket\nsocket.create_connection(('1.1.1.1', 443), timeout=5)\n",
    )
    .unwrap();
    sh(d.path(), "git add -A && git commit -qm fetch");
    let (code, json) = countercheck(d.path(), &[]);
    assert_eq!(code, 2, "{json}");
    assert_eq!(json["verdict"], "failed");
    let tail = json["checks"][0]["output_tail"].as_str().unwrap();
    assert!(tail.contains("network = true"), "{tail}");
    assert!(
        json["checks"][0].get("network").is_none(),
        "absent when false: {json}"
    );
}

/// A write the sandbox refused gets a write hint naming the path, not the internet hint
/// (issue #16: "Operation not permitted" is also what a refused write prints). Still FAILED.
#[cfg(target_os = "macos")]
#[test]
fn a_refused_write_gets_a_write_hint_not_the_internet_hint() {
    if nested() {
        return;
    }
    let outside = tempfile::tempdir().unwrap();
    let target = outside.path().canonicalize().unwrap().join("scratch.txt");
    let checks = format!(
        "[[check]]\nname = \"py\"\nrun = \"python3 -c \\\"open('{t}', 'w').write('x')\\\"\"\n\
         [[check]]\nname = \"sh\"\nrun = \"echo x > '{t}'\"\n",
        t = target.display()
    );
    let d = project(&checks);
    let (code, json) = countercheck(d.path(), &[]);
    assert_eq!(code, 2, "{json:#}");
    for c in json["checks"].as_array().unwrap() {
        let tail = c["output_tail"].as_str().unwrap();
        assert!(
            tail.contains("tried to write outside the check folder"),
            "{tail}"
        );
        assert!(tail.contains(&target.display().to_string()), "{tail}");
        assert!(!tail.contains("without internet access"), "{tail}");
        assert!(!tail.contains("network = true"), "{tail}");
    }
}

/// An ordinary failure under the sandbox gets no internet hint (it would only mislead the agent).
#[cfg(target_os = "macos")]
#[test]
fn an_ordinary_failure_gets_no_internet_hint() {
    if nested() {
        return;
    }
    let d = project("[[check]]\nname = \"bad\"\nrun = \"echo assertion failed; exit 1\"\n");
    let (_, json) = countercheck(d.path(), &[]);
    let tail = json["checks"][0]["output_tail"].as_str().unwrap();
    assert!(!tail.contains("network = true"), "{tail}");
}

/// Without the sandbox there is no hint: the network wasn't blocked by Countercheck.
#[test]
fn no_internet_hint_without_the_sandbox() {
    let d = project("[[check]]\nname = \"bad\"\nrun = \"exit 1\"\n");
    let (_, json) = countercheck(d.path(), &["--sandbox", "none"]);
    let tail = json["checks"][0]["output_tail"].as_str().unwrap();
    assert!(!tail.contains("network = true"), "{tail}");
}

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

/// Names of the checks on a receipt, in order.
fn check_names(r: &serde_json::Value) -> Vec<String> {
    r["checks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["name"].as_str().unwrap().to_string())
        .collect()
}

/// The receipt entry for the check called `name`.
fn check<'a>(r: &'a serde_json::Value, name: &str) -> &'a serde_json::Value {
    r["checks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["name"] == name)
        .unwrap_or_else(|| panic!("no check {name} in {r:#}"))
}

#[test]
fn matrix_runs_once_per_target_and_names_the_failing_one() {
    // Target 2 fails; the value is both in the command and in the environment.
    let d = project(
        "[[check]]\nname = \"target\"\nrun = \"[ \\\"$COUNTERCHECK_MATRIX_N\\\" = {n} ] && [ {n} != 2 ]\"\n\
         matrix = { n = [\"1\", \"2\", \"3\"] }\n",
    );
    let (code, r) = countercheck(d.path(), &[]);
    assert_eq!(code, 2, "{r:#}");
    assert_eq!(
        check_names(&r),
        vec!["target [n=1]", "target [n=2]", "target [n=3]"]
    );
    assert_eq!(check(&r, "target [n=1]")["passed"], true);
    assert_eq!(check(&r, "target [n=2]")["passed"], false);
    assert_eq!(
        check(&r, "target [n=2]")["run"],
        "[ \"$COUNTERCHECK_MATRIX_N\" = 2 ] && [ 2 != 2 ]"
    );
    assert_eq!(check(&r, "target [n=3]")["passed"], true);
    let reasons = r["reasons"].to_string();
    assert!(reasons.contains("target [n=2]"), "{reasons}");
    assert!(!reasons.contains("target [n=1]"), "{reasons}");
}

#[test]
fn matrix_file_edited_by_the_worker_is_ignored() {
    let d = project(
        "[[check]]\nname = \"target\"\nrun = \"[ {n} != 2 ]\"\n\
         matrix_file = { n = \".countercheck/targets.txt\" }\n",
    );
    let p = d.path();
    fs::write(p.join(".countercheck/targets.txt"), "# targets\n1\n2\n").unwrap();
    sh(p, "git add -A && git commit -qm targets");
    // The worker drops the failing target from the list.
    fs::write(p.join(".countercheck/targets.txt"), "1\n").unwrap();
    let (code, r) = countercheck(p, &[]);
    assert_eq!(code, 2, "{r:#}");
    assert_eq!(check_names(&r), vec!["target [n=1]", "target [n=2]"]);
    assert_eq!(check(&r, "target [n=2]")["passed"], false);
    assert_eq!(
        r["protected_changes"][0]["path"],
        ".countercheck/targets.txt"
    );
}

#[test]
fn matrix_values_that_are_not_plain_are_refused() {
    let d = project(
        "[[check]]\nname = \"t\"\nrun = \"echo {n}\"\nmatrix = { n = [\"a;touch pwned\"] }\n",
    );
    let out = Command::new(BIN)
        .args(["verify", "--sandbox", "none", "--repo"])
        .arg(d.path())
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("not allowed"), "{err}");
    assert!(!d.path().join("pwned").exists());
}

const FORBID: &str = "[[forbid]]\nname = \"no oracle syntax\"\npattern = '\\(\\+\\)|\\bDECODE\\s*\\('\n\
                      paths = [\"sql/**/*.sql\"]\nmessage = \"Convert these to the new syntax.\"\ncase_insensitive = true\n";

#[test]
fn forbid_hit_fails_with_path_and_line() {
    let d = project(&format!("{CHECKS}{FORBID}"));
    let p = d.path();
    fs::write(p.join("src/calc.sh"), "echo $(($1 + $2))\n").unwrap();
    fs::create_dir_all(p.join("sql/sales")).unwrap();
    fs::write(
        p.join("sql/sales/q.sql"),
        "select 1\nfrom a, b where a.id = b.id(+)\n",
    )
    .unwrap();
    fs::write(p.join("sql/ok.sql"), "select coalesce(x, 0) from t\n").unwrap();
    // Outside `paths`: not scanned.
    fs::write(p.join("src/old.sql"), "select decode(x, 1, 2) from t\n").unwrap();
    let (code, r) = countercheck(p, &[]);
    assert_eq!(code, 2, "{r:#}");
    assert_eq!(check(&r, "calc")["passed"], true);
    let f = check(&r, "forbid: no oracle syntax");
    assert_eq!(f["passed"], false);
    let out = f["output_tail"].as_str().unwrap();
    assert!(
        out.contains("sql/sales/q.sql:2: from a, b where a.id = b.id(+)"),
        "{out}"
    );
    assert!(
        out.starts_with("1 forbidden line(s) in 1 file(s); 2 file(s) scanned"),
        "{out}"
    );
    assert!(out.ends_with("Convert these to the new syntax."), "{out}");
    assert!(!out.contains("src/old.sql"), "{out}");
    assert!(
        r["reasons"]
            .to_string()
            .contains("forbid: no oracle syntax")
    );
}

#[test]
fn forbid_with_no_matching_files_passes_and_says_so() {
    let d = project(&format!("{CHECKS}{FORBID}"));
    fs::write(d.path().join("src/calc.sh"), "echo $(($1 + $2))\n").unwrap();
    let (code, r) = countercheck(d.path(), &[]);
    assert_eq!(code, 0, "{r:#}");
    let f = check(&r, "forbid: no oracle syntax");
    assert_eq!(f["passed"], true);
    assert_eq!(
        f["output_tail"],
        "no files match sql/**/*.sql; nothing to scan"
    );
}

/// A file that is only `[[forbid]]` works, and a binary file is listed as skipped, not scanned.
#[test]
fn forbid_alone_skips_binary_files() {
    let d = project(FORBID);
    let p = d.path();
    fs::create_dir_all(p.join("sql")).unwrap();
    fs::write(p.join("sql/a.sql"), "select 1\n").unwrap();
    fs::write(p.join("sql/blob.sql"), b"decode(\0\x01").unwrap();
    let (code, r) = countercheck(p, &[]);
    assert_eq!(code, 0, "{r:#}");
    let out = r["checks"][0]["output_tail"].as_str().unwrap();
    assert!(out.contains("no forbidden lines in 1 file(s)"), "{out}");
    assert!(
        out.contains("skipped 1 file(s) (binary or over 5 MiB): sql/blob.sql"),
        "{out}"
    );
}

const EXPECTED_CSV: &str = "id,region,total\n1,north,10.5\n2,south,20\n3,east,30.25\n";

/// A project whose check runs the worker's `src/export.sh` and compares `out/totals.csv` with a
/// locked reference. `golden_extra` adds keys to the golden table (mode, tolerance, sort_rows).
fn golden_project(golden_extra: &str) -> tempfile::TempDir {
    let d = project(&format!(
        "[[check]]\nname = \"totals\"\nrun = \"mkdir -p out && sh src/export.sh > out/totals.csv\"\n\
         golden = {{ output = \"out/totals.csv\", expected = \".countercheck/golden/totals.csv\"{golden_extra} }}\n"
    ));
    let p = d.path();
    fs::create_dir_all(p.join(".countercheck/golden")).unwrap();
    fs::write(p.join(".countercheck/golden/totals.csv"), EXPECTED_CSV).unwrap();
    fs::write(p.join("src/export.sh"), "echo not yet\n").unwrap();
    sh(p, "git add -A && git commit -qm golden");
    d
}

/// The worker's export prints `csv`.
fn export_prints(d: &Path, csv: &str) {
    fs::write(d.join("src/export.sh"), format!("cat <<'EOF'\n{csv}EOF\n")).unwrap();
}

#[test]
fn golden_csv_rows_in_another_order_pass_only_with_sort_rows() {
    let shuffled = "id,region,total\n3,east,30.25\n1,north,10.5\n2,south,20\n";
    let d = golden_project(", mode = \"csv\", sort_rows = true");
    export_prints(d.path(), shuffled);
    let (code, r) = countercheck(d.path(), &[]);
    assert_eq!(code, 0, "{r:#}");
    assert!(
        r["checks"][0]["output_tail"]
            .as_str()
            .unwrap()
            .ends_with("golden: out/totals.csv matches .countercheck/golden/totals.csv")
    );

    let d = golden_project(", mode = \"csv\"");
    export_prints(d.path(), shuffled);
    let (code, r) = countercheck(d.path(), &[]);
    assert_eq!(code, 2, "{r:#}");
    let c = &r["checks"][0];
    assert_eq!(c["exit_code"], 0);
    let out = c["output_tail"].as_str().unwrap();
    assert!(
        out.contains("golden: out/totals.csv does not match"),
        "{out}"
    );
    assert!(out.contains("row 1, column id: expected 1, got 3"), "{out}");
    assert!(
        r["reasons"]
            .to_string()
            .contains("failed (exit 0; its output did not pass)")
    );
}

#[test]
fn golden_csv_numbers_compare_within_the_tolerance() {
    let near = "id,region,total\n1,north,10.5000004\n2,south,20\n3,east,30.25\n";
    let far = "id,region,total\n1,north,10.51\n2,south,20\n3,east,30.25\n";
    let d = golden_project(", mode = \"csv\", tolerance = 1e-6");
    export_prints(d.path(), near);
    let (code, r) = countercheck(d.path(), &[]);
    assert_eq!(code, 0, "{r:#}");
    export_prints(d.path(), far);
    let (code, r) = countercheck(d.path(), &[]);
    assert_eq!(code, 2, "{r:#}");
    let out = r["checks"][0]["output_tail"].as_str().unwrap();
    assert!(
        out.contains("row 1, column total: expected 10.5, got 10.51"),
        "{out}"
    );
}

#[test]
fn golden_file_edited_by_the_worker_is_ignored() {
    let wrong = "id,region,total\n1,north,99\n";
    let d = golden_project(", mode = \"csv\"");
    let p = d.path();
    export_prints(p, wrong);
    // The worker "updates" the reference to match its wrong output.
    fs::write(p.join(".countercheck/golden/totals.csv"), wrong).unwrap();
    let (code, r) = countercheck(p, &[]);
    assert_eq!(code, 2, "{r:#}");
    assert_eq!(
        r["protected_changes"][0]["path"],
        ".countercheck/golden/totals.csv"
    );
    let out = r["checks"][0]["output_tail"].as_str().unwrap();
    assert!(
        out.contains("row count differs: expected 3, got 1"),
        "{out}"
    );
}

#[test]
fn golden_text_mode_reports_the_first_differing_line() {
    let d = golden_project("");
    export_prints(
        d.path(),
        "id,region,total\n1,north,10.5\n2,SOUTH,20\n3,east,30.25\n",
    );
    let (code, r) = countercheck(d.path(), &[]);
    assert_eq!(code, 2, "{r:#}");
    let out = r["checks"][0]["output_tail"].as_str().unwrap();
    assert!(
        out.contains("line 3: expected \"2,south,20\", got \"2,SOUTH,20\""),
        "{out}"
    );
    // Same content with CRLF line endings matches.
    export_prints(d.path(), &EXPECTED_CSV.replace('\n', "\r\n"));
    let (code, r) = countercheck(d.path(), &[]);
    assert_eq!(code, 0, "{r:#}");
}

/// A missing output fails plainly, and an output the worker committed doesn't count: only what
/// the command writes is compared.
#[test]
fn golden_output_must_be_written_by_the_check() {
    let d = project(
        "[[check]]\nname = \"totals\"\nrun = \"true\"\n\
         golden = { output = \"out/totals.csv\", expected = \".countercheck/golden/totals.csv\" }\n",
    );
    let p = d.path();
    fs::create_dir_all(p.join(".countercheck/golden")).unwrap();
    fs::write(p.join(".countercheck/golden/totals.csv"), EXPECTED_CSV).unwrap();
    sh(p, "git add -A && git commit -qm golden");
    let (code, r) = countercheck(p, &[]);
    assert_eq!(code, 2, "{r:#}");
    let out = r["checks"][0]["output_tail"].as_str().unwrap();
    assert!(
        out.contains("the check did not write its output file out/totals.csv"),
        "{out}"
    );
    // The worker commits a perfect copy as the "output": still not written by the check.
    fs::create_dir_all(p.join("out")).unwrap();
    fs::write(p.join("out/totals.csv"), EXPECTED_CSV).unwrap();
    let (code, _) = countercheck(p, &[]);
    assert_eq!(code, 2);
}

#[test]
fn golden_works_with_a_matrix() {
    let d = project(
        "[[check]]\nname = \"tenant\"\nrun = \"mkdir -p out && sh src/export.sh {t} > out/{t}.csv\"\n\
         matrix = { t = [\"a\", \"b\"] }\n\
         golden = { output = \"out/{t}.csv\", expected = \".countercheck/golden/{t}.csv\", mode = \"csv\" }\n",
    );
    let p = d.path();
    fs::create_dir_all(p.join(".countercheck/golden")).unwrap();
    fs::write(p.join(".countercheck/golden/a.csv"), "t,n\na,1\n").unwrap();
    fs::write(p.join(".countercheck/golden/b.csv"), "t,n\nb,2\n").unwrap();
    sh(p, "git add -A && git commit -qm golden");
    // Right for a, wrong for b.
    fs::write(p.join("src/export.sh"), "printf 't,n\\n%s,1\\n' \"$1\"\n").unwrap();
    let (code, r) = countercheck(p, &[]);
    assert_eq!(code, 2, "{r:#}");
    assert_eq!(check(&r, "tenant [t=a]")["passed"], true);
    let b = check(&r, "tenant [t=b]");
    assert_eq!(b["passed"], false);
    let out = b["output_tail"].as_str().unwrap();
    assert!(out.contains("row 1, column n: expected 2, got 1"), "{out}");
}

/// A project whose "bench" check runs the worker's `src/bench.sh` with a budget on `seconds`.
fn budget_project(budget: &str, matrix: &str) -> tempfile::TempDir {
    project(&format!(
        "[[check]]\nname = \"bench\"\nrun = \"sh src/bench.sh{}\"\nbudget = {{ {budget} }}\n{matrix}",
        if matrix.is_empty() { "" } else { " {n}" }
    ))
}

#[test]
fn budget_within_and_over_the_limit() {
    let d = budget_project("metric = \"seconds\", max = 30", "");
    let p = d.path();
    fs::write(
        p.join("src/bench.sh"),
        "echo 'METRIC seconds=41'\necho 'METRIC seconds=12.4'\n",
    )
    .unwrap();
    let (code, r) = countercheck(p, &[]);
    assert_eq!(code, 0, "{r:#}");
    let out = r["checks"][0]["output_tail"].as_str().unwrap();
    assert!(
        out.ends_with("budget: seconds = 12.4 (max 30; within the limit)"),
        "{out}"
    );

    fs::write(p.join("src/bench.sh"), "echo 'METRIC seconds=31.5'\n").unwrap();
    let (code, r) = countercheck(p, &[]);
    assert_eq!(code, 2, "{r:#}");
    let out = r["checks"][0]["output_tail"].as_str().unwrap();
    assert!(
        out.ends_with("budget: seconds = 31.5 (max 30; over the limit)"),
        "{out}"
    );
    assert!(r["reasons"].to_string().contains("its output did not pass"));
}

#[test]
fn budget_metric_missing_fails_and_early_lines_count() {
    let d = budget_project("metric = \"seconds\", min = 1", "");
    let p = d.path();
    fs::write(p.join("src/bench.sh"), "echo done\n").unwrap();
    let (code, r) = countercheck(p, &[]);
    assert_eq!(code, 2, "{r:#}");
    let out = r["checks"][0]["output_tail"].as_str().unwrap();
    assert!(
        out.contains("the output has no \"METRIC seconds=<number>\" line"),
        "{out}"
    );
    // The metric line is far before the last lines of output: still found.
    fs::write(
        p.join("src/bench.sh"),
        "echo 'METRIC seconds=2'\ni=0; while [ $i -lt 5000 ]; do echo noise $i; i=$((i+1)); done\n",
    )
    .unwrap();
    let (code, r) = countercheck(p, &[]);
    assert_eq!(code, 0, "{r:#}");
}

#[test]
fn budget_works_with_a_matrix() {
    let d = budget_project(
        "metric = \"seconds\", max = 10",
        "matrix = { n = [\"5\", \"20\"] }\n",
    );
    fs::write(
        d.path().join("src/bench.sh"),
        "echo \"METRIC seconds=$1\"\n",
    )
    .unwrap();
    let (code, r) = countercheck(d.path(), &[]);
    assert_eq!(code, 2, "{r:#}");
    assert_eq!(check(&r, "bench [n=5]")["passed"], true);
    assert_eq!(check(&r, "bench [n=20]")["passed"], false);
}

/// A project with a coverage entry over `sql/**/*.sql` for the procedures in a locked list.
fn coverage_project() -> tempfile::TempDir {
    let d = project(
        "[[coverage]]\nname = \"every procedure converted\"\nitems = \".countercheck/inventory/procs.txt\"\n\
         paths = [\"sql/**/*.sql\"]\npattern = 'CREATE\\s+OR\\s+REPLACE\\s+PROCEDURE\\s+{item}\\b'\ncase_insensitive = true\n",
    );
    let p = d.path();
    fs::create_dir_all(p.join(".countercheck/inventory")).unwrap();
    fs::write(
        p.join(".countercheck/inventory/procs.txt"),
        "# procedures to convert\nload_orders\nload_items\nclose_month\n",
    )
    .unwrap();
    fs::create_dir_all(p.join("sql")).unwrap();
    sh(p, "git add -A && git commit -qm inventory");
    d
}

#[test]
fn coverage_lists_missing_items() {
    let d = coverage_project();
    let p = d.path();
    fs::write(
        p.join("sql/orders.sql"),
        "create or replace\n  procedure load_orders() as $$ $$;\nCREATE OR REPLACE PROCEDURE load_items_v2() AS $$ $$;\n",
    )
    .unwrap();
    let (code, r) = countercheck(p, &[]);
    assert_eq!(code, 2, "{r:#}");
    let c = check(&r, "coverage: every procedure converted");
    let out = c["output_tail"].as_str().unwrap();
    assert!(
        out.starts_with("2 of 3 item(s) missing; 1 file(s) scanned"),
        "{out}"
    );
    assert!(
        out.contains("missing: load_items\nmissing: close_month"),
        "{out}"
    );

    fs::write(
        p.join("sql/rest.sql"),
        "CREATE OR REPLACE PROCEDURE load_items() AS $$ $$;\nCREATE OR REPLACE PROCEDURE close_month() AS $$ $$;\n",
    )
    .unwrap();
    let (code, r) = countercheck(p, &[]);
    assert_eq!(code, 0, "{r:#}");
    assert_eq!(
        r["checks"][0]["output_tail"],
        "all 3 item(s) found in 2 file(s)"
    );
}

#[test]
fn coverage_list_edited_by_the_worker_is_ignored() {
    let d = coverage_project();
    let p = d.path();
    fs::write(
        p.join("sql/orders.sql"),
        "CREATE OR REPLACE PROCEDURE load_orders() AS $$ $$;\n",
    )
    .unwrap();
    // The worker trims the inventory to what it converted.
    fs::write(p.join(".countercheck/inventory/procs.txt"), "load_orders\n").unwrap();
    let (code, r) = countercheck(p, &[]);
    assert_eq!(code, 2, "{r:#}");
    let out = r["checks"][0]["output_tail"].as_str().unwrap();
    assert!(out.starts_with("2 of 3 item(s) missing"), "{out}");
}

/// Run `countercheck hook hermes-pre-verify` with `input` on stdin; returns (exit code, stdout).
fn hermes_hook(input: &str, envs: &[(&str, &str)]) -> (i32, String) {
    run_hook("hermes-pre-verify", input, envs)
}

/// Run `countercheck hook <name>` with `input` on stdin; returns (exit code, stdout).
fn run_hook(name: &str, input: &str, envs: &[(&str, &str)]) -> (i32, String) {
    use std::io::Write;
    let mut cmd = Command::new(BIN);
    cmd.args(["hook", name])
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

/// Hermes runs hooks from its own working directory (the desktop app and gateway: the home
/// folder), so the project comes from the changed files, not `cwd`.
fn hermes_payload_from(cwd: &Path, changed: &[&Path]) -> String {
    serde_json::json!({
        "hook_event_name": "pre_verify",
        "cwd": cwd,
        "session_id": "s1",
        "extra": { "final_response": "Fixed it, all tests pass.", "changed_paths": changed, "model": "m" }
    })
    .to_string()
}

#[test]
fn hermes_hook_finds_the_project_from_the_changed_files_when_cwd_is_elsewhere() {
    let d = project(CHECKS);
    fs::write(d.path().join("tests/check.sh"), "exit 0\n").unwrap();
    let home = tempfile::tempdir().unwrap(); // not a git repository, like Hermes's own cwd
    let log = d.path().join("hook.log");
    let (code, out) = hermes_hook(
        &hermes_payload_from(home.path(), &[&d.path().join("tests/check.sh")]),
        &[("COUNTERCHECK_HOOK_LOG", log.to_str().unwrap())],
    );
    assert_eq!(code, 0);
    let v: serde_json::Value = serde_json::from_str(out.trim()).unwrap();
    assert_eq!(v["action"], "continue", "{out}");
    let line: serde_json::Value =
        serde_json::from_str(fs::read_to_string(&log).unwrap().trim()).unwrap();
    assert_eq!(line["verdict"], "failed");
}

#[test]
fn hermes_hook_is_silent_when_the_changed_files_are_in_no_countercheck_project() {
    let d = project(CHECKS);
    fs::write(d.path().join("tests/check.sh"), "exit 0\n").unwrap(); // would fail if checked
    let other = tempfile::tempdir().unwrap();
    sh(other.path(), "git init -q && echo x > notes.txt");
    let log = d.path().join("hook.log");
    let (code, out) = hermes_hook(
        // cwd is the failing project, but the turn only edited a file in another repository
        &hermes_payload_from(d.path(), &[&other.path().join("notes.txt")]),
        &[("COUNTERCHECK_HOOK_LOG", log.to_str().unwrap())],
    );
    assert_eq!((code, out.trim()), (0, ""));
    assert!(!log.exists(), "nothing should have been checked");
}

#[test]
fn hermes_hook_still_checks_a_project_whose_check_file_the_agent_deleted() {
    let d = project(CHECKS);
    fs::write(d.path().join("tests/check.sh"), "exit 0\n").unwrap();
    fs::remove_file(d.path().join(".countercheck/checks.toml")).unwrap();
    let home = tempfile::tempdir().unwrap();
    let (code, out) = hermes_hook(
        &hermes_payload_from(home.path(), &[&d.path().join("tests/check.sh")]),
        &[],
    );
    assert_eq!(code, 0);
    let v: serde_json::Value = serde_json::from_str(out.trim()).unwrap();
    assert_eq!(v["action"], "continue", "{out}");
}

#[test]
fn hermes_hook_checks_every_project_the_turn_touched() {
    let good = project(CHECKS);
    fs::write(good.path().join("src/calc.sh"), "echo $(($1 + $2))\n").unwrap();
    let bad = project(CHECKS);
    fs::write(bad.path().join("tests/check.sh"), "exit 0\n").unwrap();
    let home = tempfile::tempdir().unwrap();
    let log = home.path().join("hook.log");
    let (code, out) = hermes_hook(
        &hermes_payload_from(
            home.path(),
            &[
                &good.path().join("src/calc.sh"),
                &bad.path().join("tests/check.sh"),
            ],
        ),
        &[("COUNTERCHECK_HOOK_LOG", log.to_str().unwrap())],
    );
    assert_eq!(code, 0);
    let v: serde_json::Value = serde_json::from_str(out.trim()).unwrap();
    let msg = v["message"].as_str().unwrap();
    // the failing project is named (by its folder; git and the OS spell the full path differently on Windows)
    let bad_name = bad
        .path()
        .file_name()
        .unwrap()
        .to_string_lossy()
        .to_string();
    let good_name = good
        .path()
        .file_name()
        .unwrap()
        .to_string_lossy()
        .to_string();
    assert!(msg.contains(&bad_name), "{msg}");
    assert!(!msg.contains(&good_name), "{msg}");
    assert_eq!(fs::read_to_string(&log).unwrap().lines().count(), 2);
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

/// `countercheck report` writes one page with every run, newest first, escaped, with no script.
#[test]
fn report_writes_a_page_of_every_run() {
    let d = project(CHECKS);
    let (code, first) = countercheck(d.path(), &["--claim", "fixed <b>it</b>"]);
    assert_eq!(code, 2);
    fs::write(d.path().join("src/calc.sh"), "echo $(($1 + $2))\n").unwrap();
    let (code, second) = countercheck(d.path(), &[]);
    assert_eq!(code, 0);
    let out = Command::new(BIN)
        .args(["report", "--repo"])
        .arg(d.path())
        .output()
        .unwrap();
    assert_eq!(
        out.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let page = d.path().join(".countercheck/receipts/report.html");
    let html = fs::read_to_string(&page).unwrap();
    for r in [&first, &second] {
        assert!(html.contains(r["id"].as_str().unwrap()), "{html}");
    }
    assert!(html.contains("fixed &lt;b&gt;it&lt;/b&gt;"));
    assert!(html.contains("<b>2</b><span>runs</span>"));
    assert!(!html.contains("<script"));
    // The page sits with the receipts, so git ignores it.
    let status = Command::new("git")
        .arg("-C")
        .arg(d.path())
        .args(["status", "--porcelain"])
        .output()
        .unwrap();
    assert!(!String::from_utf8_lossy(&status.stdout).contains("report.html"));
}

/// Every receipt in a folder, parsed.
fn all_receipts(dir: &Path) -> Vec<serde_json::Value> {
    let mut out = Vec::new();
    for e in fs::read_dir(dir.join(".countercheck/receipts")).unwrap() {
        let p = e.unwrap().path();
        if p.extension().is_some_and(|x| x == "json") {
            out.push(serde_json::from_str(&fs::read_to_string(p).unwrap()).unwrap());
        }
    }
    out
}

/// Several agents on one project: `--role`, else `COUNTERCHECK_ROLE`, names each one on its receipt
/// (cleaned up, sealed with the rest); the report lists one row per agent and the project's path.
#[test]
fn roles_tell_agents_apart_on_the_receipt_and_the_report() {
    let d = project(CHECKS);
    let (_, by_flag) = countercheck(
        d.path(),
        &["--role", "  UI\n design ", "--agent", "claude-code"],
    );
    assert_eq!(by_flag["worker"]["role"], "UI design");
    let out = Command::new(BIN)
        .args([
            "verify",
            "--json",
            "--sandbox",
            "none",
            "--agent",
            "codex",
            "--repo",
        ])
        .arg(d.path())
        .env("COUNTERCHECK_ROLE", "Security review")
        .output()
        .unwrap();
    let by_env: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(by_env["worker"]["role"], "Security review");
    // No role: the field is left out, so older readers see the receipt they know.
    let (_, none) = countercheck(d.path(), &["--role", " \t"]);
    assert!(none["worker"].get("role").is_none(), "{none}");
    // The role is sealed like everything else.
    let id = by_flag["id"].as_str().unwrap();
    let file = d
        .path()
        .join(".countercheck/receipts")
        .join(format!("{id}.json"));
    let text = fs::read_to_string(&file).unwrap();
    fs::write(&file, text.replace("UI design", "Release")).unwrap();
    let show = Command::new(BIN)
        .args(["show"])
        .arg(&file)
        .output()
        .unwrap();
    assert_ne!(
        show.status.code(),
        Some(0),
        "an edited role must break the seal"
    );
    fs::write(&file, text).unwrap();

    let out = Command::new(BIN)
        .args(["report", "--repo"])
        .arg(d.path())
        .env("HOME", d.path().parent().unwrap())
        .output()
        .unwrap();
    assert_eq!(
        out.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let html = fs::read_to_string(d.path().join(".countercheck/receipts/report.html")).unwrap();
    assert!(html.contains("<h2>3 agents on this project</h2>"), "{html}");
    // The flow map draws one node per agent, and each run replays its steps.
    assert_eq!(html.matches("\" class=\"fn a-").count(), 3, "{html}");
    assert_eq!(html.matches("<ol class=\"steps").count(), 3, "{html}");
    for name in ["<b>UI design</b>", "<b>Security review</b>"] {
        assert!(html.contains(name), "{name}: {html}");
    }
    // The home folder is shown as ~, so a shared page doesn't carry the user's name.
    let leaf = d.path().file_name().unwrap().to_string_lossy();
    let sep = std::path::MAIN_SEPARATOR;
    assert!(
        html.contains(&format!("<code>~{sep}{leaf}</code>")),
        "{html}"
    );
}

/// Without `--agent`, `verify` names the agent whose shell it runs in, from the marker each agent
/// sets; `--agent` still wins, and with no marker the agent stays unnamed.
#[test]
fn verify_names_the_agent_from_its_shell_when_not_told() {
    let d = project(CHECKS);
    let markers = [
        "CLAUDE_CODE_CHILD_SESSION",
        "CLAUDECODE",
        "CURSOR_AGENT",
        "CODEX_SANDBOX",
        "CODEX_SANDBOX_NETWORK_DISABLED",
        "HERMES_SESSION_ID",
    ];
    let run = |set: &[(&str, &str)], args: &[&str]| -> serde_json::Value {
        let mut cmd = Command::new(BIN);
        cmd.args(["verify", "--json", "--sandbox", "none", "--repo"])
            .arg(d.path())
            .args(args);
        for m in markers {
            cmd.env_remove(m);
        }
        for (k, v) in set {
            cmd.env(k, v);
        }
        serde_json::from_slice(&cmd.output().unwrap().stdout).unwrap()
    };
    let agent = |v: serde_json::Value| v["worker"]["agent"].as_str().map(str::to_string);
    assert_eq!(agent(run(&[], &[])), None);
    assert_eq!(
        agent(run(&[("CURSOR_AGENT", "1")], &[])).as_deref(),
        Some("cursor")
    );
    assert_eq!(
        agent(run(&[("CODEX_SANDBOX", "seatbelt")], &[])).as_deref(),
        Some("codex")
    );
    assert_eq!(
        agent(run(&[("CLAUDECODE", "1")], &[])).as_deref(),
        Some("claude-code")
    );
    let h = run(&[("HERMES_SESSION_ID", "20261023_x\n")], &[]);
    assert_eq!(h["worker"]["agent"], "hermes");
    assert_eq!(h["worker"]["session"], "20261023_x");
    // Claude Code started from Hermes inherits both: the inner spawner's marker wins.
    let both = [
        ("HERMES_SESSION_ID", "s"),
        ("CLAUDE_CODE_CHILD_SESSION", "1"),
    ];
    assert_eq!(agent(run(&both, &[])).as_deref(), Some("claude-code"));
    // An empty marker is no marker, and --agent always wins.
    assert_eq!(agent(run(&[("CURSOR_AGENT", "")], &[])), None);
    assert_eq!(
        agent(run(&[("CURSOR_AGENT", "1")], &["--agent", "aider"])).as_deref(),
        Some("aider")
    );
    // The report calls the unnamed runs what they are.
    let out = Command::new(BIN)
        .args(["report", "--repo"])
        .arg(d.path())
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(0));
    let html = fs::read_to_string(d.path().join(".countercheck/receipts/report.html")).unwrap();
    assert!(html.contains("<b>unnamed agent</b>"), "{html}");
    assert!(
        html.contains("run without <code>--agent</code> or a role"),
        "{html}"
    );
    assert!(!html.contains("unknown agent"), "{html}");
}

/// Make `path` look untouched for `days` days.
fn age(path: &Path, days: u64) {
    let when =
        std::time::SystemTime::now() - std::time::Duration::from_secs(days * 24 * 3600 + 120);
    fs::File::options()
        .write(true)
        .open(path)
        .unwrap()
        .set_modified(when)
        .unwrap();
}

/// `countercheck prune` deletes receipts older than the age given (by their own date), lists them
/// first with --dry-run, and never touches anything that isn't a receipt.
#[test]
fn prune_removes_only_old_receipts_and_only_when_asked() {
    let d = project(CHECKS);
    // Different claims: two runs in the same second with the same content are one receipt (its id
    // is its content's hash), which is how a fast machine broke this test.
    let (_, old) = countercheck(d.path(), &["--claim", "first"]);
    let (_, new) = countercheck(d.path(), &["--claim", "second"]);
    assert_ne!(old["id"], new["id"]);
    let dir = d.path().join(".countercheck/receipts");
    let old_file = dir.join(format!("{}.json", old["id"].as_str().unwrap()));
    let new_file = dir.join(format!("{}.json", new["id"].as_str().unwrap()));
    let text = fs::read_to_string(&old_file).unwrap();
    let created = old["created_at"].as_str().unwrap();
    fs::write(&old_file, text.replace(created, "2020-01-02T03:04:05Z")).unwrap();
    // Old file times don't count: only the receipt's own date.
    age(&new_file, 400);
    fs::write(
        dir.join("notes.json"),
        "{\"created_at\": \"2001-01-01T00:00:00Z\"}",
    )
    .unwrap();
    age(&dir.join("notes.json"), 400);
    let prune = |args: &[&str]| {
        Command::new(BIN)
            .arg("prune")
            .args(args)
            .arg("--repo")
            .arg(d.path())
            .output()
            .unwrap()
    };
    // No age, no pruning; a bad age is a plain error.
    assert!(!prune(&[]).status.success());
    let bad = prune(&["--older-than", "soon"]);
    assert_eq!(bad.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&bad.stderr).contains("examples: 90d"));
    let dry = prune(&["--older-than", "90d", "--dry-run"]);
    let text = String::from_utf8_lossy(&dry.stdout);
    assert!(text.contains("would remove 2020-01-02T03:04:05Z"), "{text}");
    assert!(
        text.contains("would remove 1 receipt(s)") && text.contains("1 kept"),
        "{text}"
    );
    assert!(old_file.exists());
    let real = prune(&["--older-than", "90d"]);
    assert_eq!(real.status.code(), Some(0));
    assert!(String::from_utf8_lossy(&real.stdout).contains("removed 1 receipt(s)"));
    assert!(!old_file.exists());
    assert!(new_file.exists() && dir.join("notes.json").exists());
}

/// `verify` clears what killed runs left in `running/` (stale or broken progress files, half
/// writes) once they've been quiet a minute; live runs and fresh files stay.
#[test]
fn verify_sweeps_progress_files_left_by_dead_runs() {
    let d = project(CHECKS);
    let running = d.path().join(".countercheck/receipts/running");
    fs::create_dir_all(&running).unwrap();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let record = |id: &str, stale_after: u64| {
        serde_json::json!({
            "schema": "countercheck.progress/v1", "id": id, "started_at": "2026-10-23T10:00:00Z",
            "updated_unix": now, "stale_after_unix": stale_after, "worker": {},
            "stage": "check",
        })
        .to_string()
    };
    for (name, body) in [
        ("dead.json", record("dead", now - 5)),
        ("live.json", record("live", now + 3600)),
        ("junk.json", "not json".to_string()),
        ("half.tmp", "{".to_string()),
        ("fresh.json", "not json yet".to_string()),
    ] {
        fs::write(running.join(name), body).unwrap();
        if name != "fresh.json" {
            age(&running.join(name), 1);
        }
    }
    countercheck(d.path(), &[]);
    let mut left: Vec<String> = fs::read_dir(&running)
        .unwrap()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    left.sort();
    assert_eq!(left, ["fresh.json", "live.json"]);
}

/// The Claude Code hook's push-back counters of sessions unused for 30 days are removed.
#[test]
fn hook_counters_of_old_sessions_are_pruned() {
    let d = project(CHECKS);
    let state = d.path().join(".countercheck/receipts/hook-state");
    fs::create_dir_all(&state).unwrap();
    for (name, days) in [("old.json", 31), ("recent.json", 2)] {
        fs::write(state.join(name), "{\"nudges\": 1}").unwrap();
        age(&state.join(name), days);
    }
    // The work fails, so the hook pushes back and counts it.
    let (code, out) = claude_hook(&claude_payload(d.path(), "s-now", "Fixed it."), &[]);
    assert_eq!(code, 0);
    assert!(out.contains("\"decision\":\"block\""), "{out}");
    assert!(!state.join("old.json").exists());
    assert!(state.join("recent.json").exists() && state.join("s-now.json").exists());
}

/// While `verify` runs, `<receipts>/running/<id>.json` says what it is doing (for the report and
/// live UI); it is gone once the receipt is written.
#[test]
fn a_run_in_progress_is_visible_until_its_receipt_is_written() {
    let d = project(
        "[[check]]\nname = \"slow check\"\nrun = \"sh tests/slow.sh\"\ntimeout_secs = 60\n",
    );
    fs::write(d.path().join("tests/slow.sh"), "sleep 3\n").unwrap();
    sh(d.path(), "git add -A && git commit -qm slow");
    let mut cmd = Command::new(BIN);
    cmd.args([
        "verify",
        "--json",
        "--role",
        "Data pipeline",
        "--agent",
        "hermes",
    ])
    .args(["--claim", "Backfilled <beta>.", "--repo"])
    .arg(d.path())
    .stdout(std::process::Stdio::piped());
    if nested() {
        cmd.args(["--sandbox", "none"]);
    }
    let child = cmd.spawn().unwrap();
    let running = d.path().join(".countercheck/receipts/running");
    let seen = (0..100).find_map(|_| {
        std::thread::sleep(std::time::Duration::from_millis(100));
        let file = fs::read_dir(&running)
            .ok()?
            .flatten()
            .find(|e| e.path().extension().is_some_and(|x| x == "json"))?;
        let v: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(file.path()).ok()?).ok()?;
        (v["stage"] == "check").then_some(v)
    });
    let out = child.wait_with_output().unwrap();
    let v = seen.expect("a progress file while the check ran");
    assert_eq!(v["schema"], "countercheck.progress/v1");
    assert_eq!(v["current"], "slow check");
    assert_eq!(
        (v["checks_done"].as_u64(), v["checks_total"].as_u64()),
        (Some(0), Some(1))
    );
    assert_eq!(v["worker"]["role"], "Data pipeline");
    assert_eq!(v["claim"], "Backfilled <beta>.");
    assert!(v["stale_after_unix"].as_u64() > v["updated_unix"].as_u64());
    // Done: the receipt exists and the progress file is gone.
    let r: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(r["verdict"], "verified");
    let left: Vec<_> = fs::read_dir(&running)
        .map(|rd| rd.flatten().map(|e| e.file_name()).collect())
        .unwrap_or_default();
    assert!(left.is_empty(), "left behind: {left:?}");
    // The receipts folder (and with it the progress files) is ignored by git from the first run.
    let status = Command::new("git")
        .arg("-C")
        .arg(d.path())
        .args(["status", "--porcelain"])
        .output()
        .unwrap();
    assert!(!String::from_utf8_lossy(&status.stdout).contains(".countercheck/receipts"));
}

/// A stale progress file (its run was killed) or a forged one is not shown; a live one is.
#[test]
fn the_report_shows_live_runs_and_skips_stale_or_broken_ones() {
    let d = project(CHECKS);
    let (_, _) = countercheck(d.path(), &[]);
    let running = d.path().join(".countercheck/receipts/running");
    fs::create_dir_all(&running).unwrap();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let record = |id: &str, role: &str, stale_after: u64| {
        serde_json::json!({
            "schema": "countercheck.progress/v1", "id": id, "started_at": "2026-10-23T10:00:00Z",
            "updated_unix": now, "stale_after_unix": stale_after,
            "worker": {"agent": "codex", "role": role},
            "claim": "<b>done</b>", "stage": "check", "checks_total": 2, "checks_done": 1,
            "current": "unit tests",
        })
        .to_string()
    };
    fs::write(running.join("a1.json"), record("a1", "Live one", now + 600)).unwrap();
    fs::write(running.join("b2.json"), record("b2", "Dead one", now - 1)).unwrap();
    fs::write(
        running.join("c3.json"),
        "{\"schema\":\"countercheck.progress/v1\"",
    )
    .unwrap();
    fs::write(running.join("d4.json"), "x".repeat(70 * 1024)).unwrap();
    let out = Command::new(BIN)
        .args(["report", "--repo"])
        .arg(d.path())
        .output()
        .unwrap();
    assert_eq!(
        out.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let html = fs::read_to_string(d.path().join(".countercheck/receipts/report.html")).unwrap();
    assert!(html.contains("<h2>1 run in progress</h2>"), "{html}");
    assert!(
        html.contains("<b>Live one · codex</b><q>&lt;b&gt;done&lt;/b&gt;</q>"),
        "{html}"
    );
    assert!(html.contains("check 2 of 2 · unit tests"), "{html}");
    assert!(!html.contains("Dead one"), "{html}");
    // Reading never removes anything: the report is read-only.
    assert_eq!(fs::read_dir(&running).unwrap().count(), 4);
}

/// A hook records the agent's session, so one session's runs group together; the role comes from
/// the agent's environment.
#[test]
fn hooks_record_the_session_and_the_role() {
    let d = project(CHECKS);
    let (code, _) = hermes_hook(
        &hermes_payload(d.path()),
        &[("COUNTERCHECK_ROLE", "Data pipeline")],
    );
    assert_eq!(code, 0);
    let r = all_receipts(d.path());
    assert_eq!(r.len(), 1);
    assert_eq!(r[0]["worker"]["session"], "s1");
    assert_eq!(r[0]["worker"]["role"], "Data pipeline");
    assert_eq!(r[0]["worker"]["agent"], "hermes");
}

/// A receipt edited after it was written shows as a broken seal; no receipts is a plain error.
#[test]
fn report_flags_an_edited_receipt_and_needs_receipts() {
    let d = project(CHECKS);
    let out = Command::new(BIN)
        .args(["report", "--repo"])
        .arg(d.path())
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&out.stderr).contains("no receipts"));
    let (_, r) = countercheck(d.path(), &[]);
    let file = d
        .path()
        .join(".countercheck/receipts")
        .join(format!("{}.json", r["id"].as_str().unwrap()));
    let text = fs::read_to_string(&file)
        .unwrap()
        .replace("\"failed\"", "\"verified\"");
    fs::write(&file, text).unwrap();
    let out = Command::new(BIN)
        .args(["report", "--repo"])
        .arg(d.path())
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(0));
    let html = fs::read_to_string(d.path().join(".countercheck/receipts/report.html")).unwrap();
    assert!(html.contains("SEAL BROKEN"), "{html}");
}

fn session_start(dir: &Path, session: &str) -> (i32, String) {
    let input = serde_json::json!({
        "session_id": session, "cwd": dir, "hook_event_name": "SessionStart", "source": "startup"
    });
    run_hook("session-start", &input.to_string(), &[])
}

/// The agent edits the protected test AND commits it, so HEAD now holds the weakened test. With the
/// session's start recorded, both stop hooks still lock the checks to where the session began.
#[test]
fn session_start_keeps_the_checks_locked_when_the_agent_commits() {
    let d = project(CHECKS);
    assert_eq!(session_start(d.path(), "s1"), (0, String::new()));
    sh(
        d.path(),
        "echo 'exit 0' > tests/check.sh && git commit -qam 'fix the tests'",
    );
    let (_, out) = claude_hook(
        &claude_payload(d.path(), "s1", "Done, all tests pass."),
        &[],
    );
    assert!(block_reason(&out).contains("FAILED"), "{out}");
    let (_, out) = hermes_hook(&hermes_payload(d.path()), &[]);
    assert!(out.contains("\"continue\""), "{out}");
    // Nothing was written into the work tree.
    let status = Command::new("git")
        .arg("-C")
        .arg(d.path())
        .args(["status", "--porcelain"])
        .output()
        .unwrap();
    assert!(String::from_utf8_lossy(&status.stdout).trim().is_empty());
}

/// Without a recorded start, the base is HEAD: the documented weakness this hook closes.
#[test]
fn without_session_start_a_committed_test_edit_moves_the_base() {
    let d = project(CHECKS);
    sh(
        d.path(),
        "echo 'exit 0' > tests/check.sh && git commit -qam 'fix the tests'",
    );
    let (_, out) = claude_hook(
        &claude_payload(d.path(), "s2", "Done, all tests pass."),
        &[],
    );
    assert_eq!(out.trim(), "");
}

/// A resumed session keeps its first start; another session gets its own; COUNTERCHECK_BASE wins.
#[test]
fn session_start_keeps_the_first_record_and_env_wins() {
    let d = project(CHECKS);
    session_start(d.path(), "s1");
    sh(
        d.path(),
        "echo 'exit 0' > tests/check.sh && git commit -qam weaken",
    );
    session_start(d.path(), "s1"); // resume: must not move the start to the weakened commit
    let (_, out) = claude_hook(&claude_payload(d.path(), "s1", "Done."), &[]);
    assert!(block_reason(&out).contains("FAILED"), "{out}");
    session_start(d.path(), "s3"); // a new session starts from the weakened commit
    let (_, out) = claude_hook(&claude_payload(d.path(), "s3", "Done."), &[]);
    assert_eq!(out.trim(), "", "{out}");
    let (_, out) = claude_hook(
        &claude_payload(d.path(), "s3", "Done."),
        &[("COUNTERCHECK_BASE", "HEAD~1")],
    );
    assert!(block_reason(&out).contains("FAILED"), "{out}");
}

/// Session start fails open and stays silent: garbage input, no session id, not a git repo.
#[test]
fn session_start_fails_open() {
    let not_git = tempfile::tempdir().unwrap();
    for input in [
        "not json".to_string(),
        serde_json::json!({ "cwd": not_git.path(), "session_id": "s" }).to_string(),
        serde_json::json!({ "cwd": not_git.path() }).to_string(),
    ] {
        let (code, out) = run_hook("session-start", &input, &[]);
        assert_eq!((code, out.as_str()), (0, ""), "{input}");
    }
}

/// Run `countercheck hook claude-stop` with `input` on stdin; returns (exit code, stdout).
fn claude_hook(input: &str, envs: &[(&str, &str)]) -> (i32, String) {
    run_hook("claude-stop", input, envs)
}

fn claude_payload(dir: &Path, session: &str, last_message: &str) -> String {
    serde_json::json!({
        "session_id": session,
        "transcript_path": dir.join("no-such-transcript.jsonl"),
        "cwd": dir,
        "permission_mode": "default",
        "hook_event_name": "Stop",
        "stop_hook_active": false,
        "last_assistant_message": last_message,
    })
    .to_string()
}

/// The reason in a Claude Code block, or panic.
fn block_reason(out: &str) -> String {
    let v: serde_json::Value = serde_json::from_str(out.trim()).unwrap();
    assert_eq!(v["decision"], "block", "{out}");
    v["reason"].as_str().unwrap().to_string()
}

#[test]
fn claude_hook_lets_passing_work_stop() {
    let d = project(CHECKS);
    fs::write(d.path().join("src/calc.sh"), "echo $(($1 + $2))\n").unwrap();
    let (code, out) = claude_hook(&claude_payload(d.path(), "s1", "Done."), &[]);
    assert_eq!((code, out.trim()), (0, ""));
}

#[test]
fn claude_hook_blocks_a_false_success_claim() {
    let d = project(CHECKS);
    let log = d.path().join("hook.log");
    let (code, out) = claude_hook(
        &claude_payload(d.path(), "s1", "Fixed it, all tests pass."),
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
    // The receipt names the agent.
    let receipt: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(line["receipt"].as_str().unwrap()).unwrap())
            .unwrap();
    assert_eq!(receipt["worker"]["agent"], "claude-code");
}

#[test]
fn claude_hook_lets_an_honest_blocker_report_stop() {
    let d = project(CHECKS);
    let said = "I fixed what I could, but test_export can't pass: it contradicts test_display.";
    let (code, out) = claude_hook(&claude_payload(d.path(), "s1", said), &[]);
    assert_eq!((code, out.trim()), (0, ""));
}

#[test]
fn claude_hook_still_blocks_tampering_even_with_a_blocker_report() {
    let d = project(CHECKS);
    fs::write(d.path().join("tests/check.sh"), "exit 0\n").unwrap();
    let said = "The test can't pass as written, so I adjusted it.";
    let (code, out) = claude_hook(&claude_payload(d.path(), "s1", said), &[]);
    assert_eq!(code, 0);
    assert!(block_reason(&out).contains("fix the code instead"), "{out}");
}

/// Without `last_assistant_message`, the claim is the last assistant text in the transcript.
#[test]
fn claude_hook_reads_the_final_message_from_the_transcript() {
    let d = project(CHECKS);
    let transcript = d.path().join("transcript.jsonl");
    let lines = [
        serde_json::json!({"type": "user", "message": {"role": "user", "content": "fix calc"}}),
        serde_json::json!({"type": "assistant", "message": {"role": "assistant", "content": [
            {"type": "text", "text": "Fixed it, all tests pass."}]}}),
        serde_json::json!({"type": "assistant", "message": {"role": "assistant", "content": [
            {"type": "text", "text": "I can't make it pass: it contradicts the spec."}]}}),
        serde_json::json!({"type": "assistant", "message": {"role": "assistant", "content": [
            {"type": "tool_use", "id": "t1", "name": "Bash", "input": {}}]}}),
    ];
    let text: String = lines.iter().map(|l| format!("{l}\n")).collect();
    fs::write(&transcript, text + "not json\n").unwrap();
    let input = serde_json::json!({
        "session_id": "s1",
        "transcript_path": transcript,
        "cwd": d.path(),
        "hook_event_name": "Stop",
        "stop_hook_active": false,
    })
    .to_string();
    let log = d.path().join("hook.log");
    let (code, out) = claude_hook(&input, &[("COUNTERCHECK_HOOK_LOG", log.to_str().unwrap())]);
    // The last text says it is blocked, so the FAILED work may stop.
    assert_eq!((code, out.trim()), (0, ""));
    let line: serde_json::Value =
        serde_json::from_str(fs::read_to_string(&log).unwrap().trim()).unwrap();
    assert_eq!(line["agent_reported_blocker"], true);
}

/// Claude Code's Stop input has no model: the receipt takes it from the last assistant message in
/// the transcript (after a `/model` switch, the new one; Claude Code's own `<synthetic>` messages
/// are skipped).
#[test]
fn claude_hook_takes_the_model_from_the_transcript() {
    let d = project(CHECKS);
    let transcript = d.path().join("transcript.jsonl");
    let msg = |model: &str, text: &str| {
        serde_json::json!({"type": "assistant", "message": {"role": "assistant", "model": model,
            "content": [{"type": "text", "text": text}]}})
    };
    let lines = [
        msg("claude-opus-5", "Looking."),
        msg("claude-sonnet-5", "Fixed it, all tests pass."),
        msg("<synthetic>", "API Error: overloaded"),
    ];
    let text: String = lines.iter().map(|l| format!("{l}\n")).collect();
    fs::write(&transcript, text).unwrap();
    let input = serde_json::json!({
        "session_id": "s1", "transcript_path": transcript, "cwd": d.path(),
        "hook_event_name": "Stop", "stop_hook_active": false,
    })
    .to_string();
    claude_hook(&input, &[]);
    let r = all_receipts(d.path());
    assert_eq!(r.len(), 1);
    assert_eq!(r[0]["worker"]["agent"], "claude-code");
    assert_eq!(r[0]["worker"]["model"], "claude-sonnet-5");
}

/// Without a model in the transcript, the one SessionStart reported for this session is used.
#[test]
fn claude_hook_falls_back_to_the_model_from_session_start() {
    let d = project(CHECKS);
    let start = serde_json::json!({
        "session_id": "s9", "cwd": d.path(), "hook_event_name": "SessionStart",
        "source": "startup", "model": "claude-opus-5",
    })
    .to_string();
    let (code, _) = run_hook("session-start", &start, &[]);
    assert_eq!(code, 0);
    let stop = serde_json::json!({
        "session_id": "s9", "transcript_path": d.path().join("none.jsonl"), "cwd": d.path(),
        "hook_event_name": "Stop", "stop_hook_active": false,
        "last_assistant_message": "Fixed it.",
    })
    .to_string();
    claude_hook(&stop, &[]);
    let r = all_receipts(d.path());
    assert_eq!(r.len(), 1);
    assert_eq!(r[0]["worker"]["model"], "claude-opus-5");
    // Another session has no recorded model, and none is made up.
    let other = stop.replace("\"s9\"", "\"s10\"");
    claude_hook(&other, &[]);
    let r = all_receipts(d.path());
    assert!(
        r.iter()
            .any(|x| x["worker"].get("model").is_none_or(|m| m.is_null())),
        "{r:?}"
    );
}

/// A Cortex Code session in `<home>/.snowflake/cortex/conversations/`: `<id>.json` with the
/// messages either in `history` (`legacy`) or in `<id>.history.jsonl`. Returns the two paths.
/// Layout from cortex-replay's parser (github.com/dataprofessor/cortex-replay, src/parser.mjs);
/// not captured from a real install.
fn cortex_session(home: &Path, id: &str, last: &str, legacy: bool) -> (String, String) {
    let dir = home.join(".snowflake/cortex/conversations");
    fs::create_dir_all(&dir).unwrap();
    let history = [
        serde_json::json!({"role": "user", "content": [{"type": "text", "text": "fix calc"}]}),
        serde_json::json!({"role": "assistant", "content": [{"type": "text", "text": "Fixed it, all tests pass."}]}),
        serde_json::json!({"role": "assistant", "content": [{"type": "text", "text": last}],
            "user_sent_time": "2026-10-21T10:01:00Z"}),
        serde_json::json!({"role": "assistant", "content": [{"type": "tool_use",
            "tool_use": {"tool_use_id": "t1", "name": "bash", "input": {}}}]}),
    ];
    let meta = serde_json::json!({
        "session_id": id, "title": "t",
        "history": if legacy { history.to_vec() } else { vec![] },
    });
    let json = dir.join(format!("{id}.json"));
    fs::write(&json, serde_json::to_string_pretty(&meta).unwrap()).unwrap();
    let jsonl = dir.join(format!("{id}.history.jsonl"));
    if !legacy {
        let text: String = history.iter().map(|l| format!("{l}\n")).collect();
        fs::write(&jsonl, text).unwrap();
    }
    (
        json.to_string_lossy().into_owned(),
        jsonl.to_string_lossy().into_owned(),
    )
}

/// Cortex Code's Stop input has no `last_assistant_message`: the claim comes from its own
/// transcript layout, and the receipt names Cortex Code.
#[test]
fn cortex_code_stop_reads_the_claim_from_its_session_files() {
    let d = project(CHECKS);
    let home = tempfile::tempdir().unwrap();
    let said = "I can't make it pass: it contradicts the spec.";
    let (json, jsonl) = cortex_session(home.path(), "c1", said, false);
    let (legacy, _) = cortex_session(home.path(), "c2", said, true);
    for transcript in [json, jsonl, legacy] {
        let input = serde_json::json!({
            "session_id": "c1",
            "transcript_path": transcript,
            "cwd": d.path(),
            "permission_mode": "default",
            "hook_event_name": "Stop",
        })
        .to_string();
        let log = d.path().join("hook.log");
        let _ = fs::remove_file(&log);
        let (code, out) = claude_hook(&input, &[("COUNTERCHECK_HOOK_LOG", log.to_str().unwrap())]);
        // The last text reports a blocker, so the FAILED work may stop.
        assert_eq!((code, out.trim()), (0, ""), "{transcript}");
        let line: serde_json::Value =
            serde_json::from_str(fs::read_to_string(&log).unwrap().trim()).unwrap();
        assert_eq!(line["agent_reported_blocker"], true, "{transcript}");
        let receipt: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(line["receipt"].as_str().unwrap()).unwrap())
                .unwrap();
        assert_eq!(receipt["claim"], said, "{transcript}");
        assert_eq!(receipt["worker"]["agent"], "cortex-code");
    }
}

#[test]
fn claude_hook_treats_a_missing_transcript_as_no_claim() {
    let d = project(CHECKS);
    let input = serde_json::json!({
        "session_id": "s1",
        "transcript_path": d.path().join("gone.jsonl"),
        "cwd": d.path(),
        "hook_event_name": "Stop",
        "stop_hook_active": false,
    })
    .to_string();
    let (code, out) = claude_hook(&input, &[]);
    assert_eq!(code, 0);
    assert!(block_reason(&out).contains("FAILED"), "{out}");
}

/// The loop guard: after COUNTERCHECK_MAX_NUDGES (default 3) push-backs, the agent may stop.
#[test]
fn claude_hook_gives_up_after_three_push_backs() {
    let d = project(CHECKS);
    let log = d.path().join("hook.log");
    let envs = [("COUNTERCHECK_HOOK_LOG", log.to_str().unwrap())];
    let payload = claude_payload(d.path(), "../odd/session id", "All tests pass.");
    for _ in 0..3 {
        let (code, out) = claude_hook(&payload, &envs);
        assert_eq!(code, 0);
        block_reason(&out);
    }
    let (code, out) = claude_hook(&payload, &envs);
    assert_eq!((code, out.trim()), (0, ""));
    let last: serde_json::Value =
        serde_json::from_str(fs::read_to_string(&log).unwrap().lines().last().unwrap()).unwrap();
    assert_eq!(last["gave_up"], true);
    // The session id was made safe, and the state stays inside the receipts folder.
    assert!(
        d.path()
            .join(".countercheck/receipts/hook-state/oddsessionid.json")
            .is_file()
    );
    // Another session starts its own count.
    let (_, out) = claude_hook(&claude_payload(d.path(), "s2", "Done."), &envs);
    block_reason(&out);
}

#[test]
fn claude_hook_can_be_switched_off_and_fails_open() {
    let d = project(CHECKS);
    let (code, out) = claude_hook(
        &claude_payload(d.path(), "s1", "Done."),
        &[("COUNTERCHECK_HOOK_DISABLED", "1")],
    );
    assert_eq!((code, out.trim()), (0, ""));
    let (code, out) = claude_hook("not json", &[]);
    assert_eq!((code, out.trim()), (0, ""));
}

/// A path for a check file string and `sh`: forward slashes (TOML strings treat `\` as an escape).
#[cfg(windows)]
fn slash(p: &Path) -> String {
    p.to_string_lossy().replace('\\', "/")
}

/// True when `program` is on PATH (`.exe` added on Windows).
fn on_path(program: &str) -> bool {
    let name = if cfg!(windows) {
        format!("{program}.exe")
    } else {
        program.to_string()
    };
    std::env::var_os("PATH")
        .is_some_and(|p| std::env::split_paths(&p).any(|d| d.join(&name).is_file()))
}

/// Windows: a timed-out check's background children die with it (`taskkill /T` kills the tree).
/// `$!` is an MSYS process id there, so the test reads the Windows one from `/proc`.
#[cfg(windows)]
#[test]
fn timeouts_kill_the_whole_process_tree_on_windows() {
    let outside = tempfile::tempdir().unwrap();
    let pid_file = outside.path().join("bg.pid");
    let checks = format!(
        "[[check]]\nname = \"bg\"\nrun = \"sleep 60 & cat /proc/$!/winpid > '{}'; wait\"\ntimeout_secs = 5\n",
        slash(&pid_file)
    );
    let d = project(&checks);
    let started = std::time::Instant::now();
    let (code, r) = countercheck(d.path(), &["--sandbox", "none"]);
    // A survivor would hold countercheck's output open until `sleep 60` ends.
    assert!(
        started.elapsed().as_secs() < 40,
        "verify took {:?}: the background process outlived the timeout",
        started.elapsed()
    );
    assert_eq!(code, 2, "{r:#}");
    assert_eq!(r["checks"][0]["timed_out"], true);
    let pid = fs::read_to_string(&pid_file).unwrap().trim().to_string();
    assert!(pid.parse::<u32>().is_ok(), "no Windows pid: {pid:?}");
    assert!(
        !windows_pid_running(&pid),
        "background process {pid} survived the timeout"
    );
}

/// Windows: what a passing check leaves running in the background is stopped when it ends, so
/// it can't hold countercheck's output (and an agent's hook) open.
#[cfg(windows)]
#[test]
fn background_processes_stop_when_a_check_ends_on_windows() {
    let outside = tempfile::tempdir().unwrap();
    let pid_file = outside.path().join("bg.pid");
    let checks = format!(
        "[[check]]\nname = \"bg\"\nrun = \"sleep 60 & cat /proc/$!/winpid > '{}'; exit 0\"\n",
        slash(&pid_file)
    );
    let d = project(&checks);
    let started = std::time::Instant::now();
    let (code, r) = countercheck(d.path(), &["--sandbox", "none"]);
    assert!(
        started.elapsed().as_secs() < 40,
        "verify took {:?}",
        started.elapsed()
    );
    assert_eq!(code, 0, "{r:#}");
    let pid = fs::read_to_string(&pid_file).unwrap().trim().to_string();
    assert!(
        !windows_pid_running(&pid),
        "background process {pid} is still running"
    );
}

/// True while Windows process `pid` runs (polls for up to 3 seconds for it to go).
#[cfg(windows)]
fn windows_pid_running(pid: &str) -> bool {
    let running = || {
        let out = Command::new("tasklist")
            .args(["/FI", &format!("PID eq {pid}"), "/NH", "/FO", "CSV"])
            .output()
            .unwrap();
        String::from_utf8_lossy(&out.stdout).contains(&format!("\"{pid}\""))
    };
    for _ in 0..30 {
        if !running() {
            return false;
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    true
}

/// `shell = "pwsh"` runs the check in PowerShell (pwsh, or Windows PowerShell). Skipped where
/// PowerShell isn't installed.
#[test]
fn pwsh_checks_run_in_powershell() {
    if !cfg!(windows) && !on_path("pwsh") {
        return;
    }
    let checks = r#"[[check]]
name = "ps"
shell = "pwsh"
run = 'if ((Get-Content src/calc.sh -Raw) -match "\+") { Write-Output "adds"; exit 0 } else { Write-Output "still subtracts"; exit 4 }'
"#;
    let d = project(checks);
    let (code, r) = countercheck(d.path(), &[]);
    assert_eq!(code, 2, "{r:#}");
    assert_eq!(r["checks"][0]["exit_code"], 4, "{r:#}");
    let out = r["checks"][0]["output_tail"].as_str().unwrap();
    assert!(out.contains("still subtracts"), "{out}");
    fs::write(d.path().join("src/calc.sh"), "echo $(($1 + $2))\n").unwrap();
    let (code, r) = countercheck(d.path(), &[]);
    assert_eq!(code, 0, "{r:#}");
    assert!(
        r["checks"][0]["output_tail"]
            .as_str()
            .unwrap()
            .contains("adds")
    );
}

/// `shell = "cmd"` runs the command line as written (quotes, `&&`, `||` and `>NUL` intact).
#[cfg(windows)]
#[test]
fn cmd_checks_run_in_cmd_with_their_quotes() {
    let checks = r#"[[check]]
name = "cmd"
shell = "cmd"
run = 'findstr /C:"+ $2" src\calc.sh >NUL && echo found "a b" || exit /b 5'
"#;
    let d = project(checks);
    let (code, r) = countercheck(d.path(), &[]);
    assert_eq!(code, 2, "{r:#}");
    assert_eq!(r["checks"][0]["exit_code"], 5, "{r:#}");
    fs::write(d.path().join("src/calc.sh"), "echo $(($1 + $2))\n").unwrap();
    let (code, r) = countercheck(d.path(), &[]);
    assert_eq!(code, 0, "{r:#}");
    let out = r["checks"][0]["output_tail"].as_str().unwrap();
    assert!(out.contains("found \"a b\""), "{out}");
}

/// Off Windows, a `cmd` check can't run: it fails with a plain reason instead of stopping countercheck.
#[cfg(not(windows))]
#[test]
fn cmd_checks_fail_plainly_off_windows() {
    let d = project("[[check]]\nname = \"cmd\"\nshell = \"cmd\"\nrun = \"dir\"\n");
    let (code, r) = countercheck(d.path(), &[]);
    assert_eq!(code, 2, "{r:#}");
    assert_eq!(r["checks"][0]["passed"], false);
    assert_eq!(r["checks"][0]["exit_code"], serde_json::Value::Null);
    let out = r["checks"][0]["output_tail"].as_str().unwrap();
    assert!(out.contains("needs Windows"), "{out}");
}

/// An unknown shell is refused when the check file is read.
#[test]
fn unknown_shells_are_refused() {
    let d = project("[[check]]\nname = \"x\"\nshell = \"bash\"\nrun = \"true\"\n");
    let out = Command::new(BIN)
        .args(["verify", "--sandbox", "none", "--repo"])
        .arg(d.path())
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("bash") && err.contains("pwsh"), "{err}");
}

/// The same files give the same tree hash and the same `/` paths on every OS (the constant was
/// computed on Linux; CI checks it on macOS and Windows too).
#[test]
fn tree_hash_and_paths_are_the_same_on_every_os() {
    let d = project(&format!(
        "{CHECKS}[[forbid]]\nname = \"no todo\"\npattern = 'TODO'\npaths = [\"src/**/*.txt\"]\n"
    ));
    let p = d.path();
    fs::write(p.join("src/calc.sh"), "echo $(($1 + $2))\n").unwrap();
    fs::create_dir_all(p.join("src/deep/er")).unwrap();
    fs::write(p.join("src/deep/er/notes.txt"), "TODO: tidy\n").unwrap();
    let (code, r) = countercheck(p, &[]);
    assert_eq!(code, 2, "{r:#}");
    let out = check(&r, "forbid: no todo")["output_tail"]
        .as_str()
        .unwrap();
    assert!(out.contains("src/deep/er/notes.txt:1: TODO: tidy"), "{out}");
    assert_eq!(r["tree_sha256"], TREE_SHA256, "{r:#}");
}

const TREE_SHA256: &str = "7fd45a5b3c4c4630b8b921e748979b53fd49e101cd678bcc12be666c641137e2";

/// A symlink in the worker's tree is recreated in the clean copy (on Windows, as a symlink when
/// allowed, else as a text file with the target, and the check output says so).
#[test]
fn symlinks_in_the_tree_reach_the_clean_copy() {
    let d = project("[[check]]\nname = \"link\"\nrun = \"cat link.txt\"\n");
    let p = d.path();
    fs::write(p.join("real.txt"), "hi\n").unwrap();
    #[cfg(unix)]
    let made = std::os::unix::fs::symlink("real.txt", p.join("link.txt")).is_ok();
    #[cfg(windows)]
    let made = std::os::windows::fs::symlink_file("real.txt", p.join("link.txt")).is_ok();
    if !made {
        return; // this system can't make symlinks at all; the fallback has a unit test
    }
    let (code, r) = countercheck(p, &[]);
    assert_eq!(code, 0, "{r:#}");
    let out = r["checks"][0]["output_tail"].as_str().unwrap();
    if out.contains("Windows refused to create symlinks") {
        // The text-file fallback: `cat` prints the link target.
        assert!(
            out.contains("real.txt") && out.contains("link.txt"),
            "{out}"
        );
    } else {
        // A line of its own, not the first one: a CI runner's shell may print a warning first
        // (Git Bash: "could not find /tmp, please create!").
        assert!(out.lines().any(|l| l.trim_end() == "hi"), "{out}");
    }
}

/// Files checked out with Windows line endings (CRLF) give the same results: the check file,
/// matrix file, coverage list, forbid and coverage scans, golden text and csv, and METRIC lines.
#[test]
fn windows_line_endings_do_not_change_the_results() {
    let checks = "protected = [\"tests/**\"]\n\
        [[check]]\nname = \"bench\"\nrun = \"sh src/bench.sh {t}\"\nmatrix_file = { t = \".countercheck/targets.txt\" }\nbudget = { metric = \"seconds\", max = 5 }\n\
        [[check]]\nname = \"csv\"\nrun = \"sh src/csv.sh > out.csv\"\ngolden = { output = \"out.csv\", expected = \".countercheck/expected.csv\", mode = \"csv\" }\n\
        [[check]]\nname = \"text\"\nrun = \"printf 'a\\\\r\\\\nb\\\\r\\\\n' > out.txt\"\ngolden = { output = \"out.txt\", expected = \".countercheck/expected.txt\" }\n\
        [[forbid]]\nname = \"no todo\"\npattern = 'TODO$'\npaths = [\"src/**\"]\n\
        [[coverage]]\nname = \"funcs\"\nitems = \".countercheck/items.txt\"\npaths = [\"src/**\"]\npattern = '^# fn {item}$'\n";
    let crlf = |t: &str| t.replace('\n', "\r\n");
    let d = project(&crlf(checks));
    let p = d.path();
    fs::write(
        p.join(".countercheck/targets.txt"),
        crlf("# targets\nfast\nslow\n"),
    )
    .unwrap();
    fs::write(
        p.join(".countercheck/expected.csv"),
        crlf("id,total\n1,10\n2,20\n"),
    )
    .unwrap();
    fs::write(p.join(".countercheck/expected.txt"), "a\nb\n").unwrap();
    fs::write(p.join(".countercheck/items.txt"), crlf("alpha\nbeta\n")).unwrap();
    sh(p, "git add -A && git commit -qm crlf");
    // Comment lines with CRLF (read by coverage); the shell code itself with LF, as sh needs.
    fs::write(
        p.join("src/bench.sh"),
        "# fn alpha\r\n# fn beta\r\nif [ \"$1\" = slow ]; then printf 'METRIC seconds=9\\r\\n'; else printf 'METRIC seconds=1\\r\\n'; fi\n",
    )
    .unwrap();
    fs::write(
        p.join("src/csv.sh"),
        "printf 'id,total\\r\\n1,10\\r\\n2,20\\r\\n'\n",
    )
    .unwrap();
    fs::write(p.join("src/notes.md"), crlf("done\nTODO\n")).unwrap();
    let (code, r) = countercheck(p, &[]);
    assert_eq!(code, 2, "{r:#}");
    assert_eq!(check(&r, "bench [t=fast]")["passed"], true, "{r:#}");
    assert_eq!(check(&r, "bench [t=slow]")["passed"], false, "{r:#}");
    assert_eq!(check(&r, "csv")["passed"], true, "{r:#}");
    assert_eq!(check(&r, "text")["passed"], true, "{r:#}");
    let forbid = check(&r, "forbid: no todo")["output_tail"]
        .as_str()
        .unwrap();
    assert!(forbid.contains("src/notes.md:2: TODO"), "{forbid}");
    assert_eq!(check(&r, "coverage: funcs")["passed"], true, "{r:#}");
}

/// Windows payloads: Claude Code and Hermes give `C:\...` paths (sometimes `C:/...`) and a
/// transcript under `~\` with no HOME set. The session's start commit must still be found, so an
/// agent that commits an edited test is still caught, and the claim is read from the transcript.
#[cfg(windows)]
#[test]
fn hooks_work_with_windows_payloads() {
    use std::io::Write;
    let d = project(CHECKS);
    let back = d.path().to_string_lossy().replace('/', "\\");
    let fwd = back.replace('\\', "/");
    let start = serde_json::json!({
        "session_id": "w1", "cwd": fwd, "hook_event_name": "SessionStart", "source": "startup"
    });
    assert_eq!(
        run_hook("session-start", &start.to_string(), &[]),
        (0, String::new())
    );
    sh(
        d.path(),
        "echo 'exit 0' > tests/check.sh && git commit -qam 'fix the tests'",
    );
    let line = serde_json::json!({"type": "assistant", "message": {"role": "assistant", "content": [
        {"type": "text", "text": "Fixed it, all tests pass."}]}});
    fs::write(d.path().join("t.jsonl"), format!("{line}\n")).unwrap();
    let stop = serde_json::json!({
        "session_id": "w1",
        "transcript_path": "~\\t.jsonl",
        "cwd": back,
        "hook_event_name": "Stop",
        "stop_hook_active": false,
    });
    let log = d.path().join("hook.log");
    let mut child = Command::new(BIN)
        .args(["hook", "claude-stop"])
        .env_remove("HOME")
        .env("USERPROFILE", d.path())
        .env("COUNTERCHECK_HOOK_LOG", &log)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(stop.to_string().as_bytes())
        .unwrap();
    let out = child.wait_with_output().unwrap();
    let out = String::from_utf8_lossy(&out.stdout).into_owned();
    assert!(block_reason(&out).contains("FAILED"), "{out}");
    let logged: serde_json::Value =
        serde_json::from_str(fs::read_to_string(&log).unwrap().trim()).unwrap();
    let receipt: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(logged["receipt"].as_str().unwrap()).unwrap())
            .unwrap();
    assert_eq!(receipt["claim"], "Fixed it, all tests pass.", "{receipt:#}");
    assert!(
        receipt["sandbox"]
            .as_str()
            .unwrap()
            .starts_with("windows-restricted-token, partial: sh checks ran unsandboxed"),
        "{receipt:#}"
    );
    let hermes = serde_json::json!({
        "hook_event_name": "pre_verify", "cwd": fwd, "session_id": "w1",
        "extra": { "final_response": "Fixed it, all tests pass." }
    });
    let (_, out) = hermes_hook(&hermes.to_string(), &[]);
    assert!(out.contains("\"continue\""), "{out}");
}

// ---- uncovered changed files (issue #6) ----

/// The calc check covers src/calc.sh only; docs and other source files are uncovered.
const COVERS: &str = "protected = [\"tests/**\"]\n[[check]]\nname = \"calc\"\nrun = \"sh tests/check.sh\"\ntimeout_secs = 30\ncovers = [\"src/calc.sh\"]\n";

fn fix_calc(p: &Path) {
    fs::write(p.join("src/calc.sh"), "echo $(($1 + $2))\n").unwrap();
}

#[test]
fn uncovered_is_off_without_covers() {
    let d = project(CHECKS);
    fix_calc(d.path());
    fs::write(d.path().join("src/billing.sh"), "echo 1\n").unwrap();
    let (code, r) = countercheck(d.path(), &[]);
    assert_eq!(code, 0, "{r:#}");
    assert!(r.get("uncovered").is_none(), "{r:#}");
}

#[test]
fn uncovered_changes_are_listed_and_the_verdict_stays_by_default() {
    let d = project(COVERS);
    fix_calc(d.path());
    fs::write(d.path().join("src/billing.sh"), "echo 1\n").unwrap();
    fs::write(d.path().join("NOTES.md"), "hi\n").unwrap();
    let (code, r) = countercheck(d.path(), &[]);
    assert_eq!(code, 0, "{r:#}");
    assert_eq!(r["verdict"], "verified");
    assert_eq!(r["uncovered"]["mode"], "note");
    assert_eq!(r["uncovered"]["total"], 2);
    assert_eq!(
        r["uncovered"]["paths"],
        serde_json::json!(["NOTES.md", "src/billing.sh"])
    );
    let reasons = r["reasons"].to_string();
    assert!(
        reasons.contains("2 changed file(s) no check covers"),
        "{reasons}"
    );
}

#[test]
fn uncovered_unverified_mode_turns_verified_into_unverified() {
    let d = project(&format!("uncovered = \"unverified\"\n{COVERS}"));
    fix_calc(d.path());
    fs::write(d.path().join("src/billing.sh"), "echo 1\n").unwrap();
    let (code, r) = countercheck(d.path(), &["--claim", "done"]);
    assert_eq!(code, 3, "{r:#}");
    assert_eq!(r["verdict"], "unverified");
    assert_eq!(r["claim_status"], "not_checked");
}

#[test]
fn uncovered_never_rescues_a_failure() {
    // The bug is left in, and the worker touches an uncovered file: still FAILED.
    let d = project(&format!("uncovered = \"unverified\"\n{COVERS}"));
    fs::write(d.path().join("src/billing.sh"), "echo 1\n").unwrap();
    let (code, r) = countercheck(d.path(), &[]);
    assert_eq!(code, 2, "{r:#}");
    assert_eq!(r["verdict"], "failed");
}

#[test]
fn a_fully_covered_change_has_nothing_uncovered() {
    let d = project(&format!("uncovered = \"unverified\"\n{COVERS}"));
    fix_calc(d.path());
    let (code, r) = countercheck(d.path(), &[]);
    assert_eq!(code, 0, "{r:#}");
    assert_eq!(r["uncovered"]["total"], 0);
}

#[test]
fn worker_cannot_widen_covers_or_switch_the_mode() {
    // The worker edits checks.toml to cover everything and drop strict mode: ignored, because
    // the check file is read from the base revision.
    let d = project(&format!("uncovered = \"unverified\"\n{COVERS}"));
    fix_calc(d.path());
    fs::write(d.path().join("src/billing.sh"), "echo 1\n").unwrap();
    fs::write(
        d.path().join(".countercheck/checks.toml"),
        "protected = [\"tests/**\"]\n[[check]]\nname = \"calc\"\nrun = \"sh tests/check.sh\"\ncovers = [\"**\"]\n",
    )
    .unwrap();
    let (code, r) = countercheck(d.path(), &[]);
    assert_eq!(code, 3, "{r:#}");
    assert_eq!(
        r["uncovered"]["paths"],
        serde_json::json!(["src/billing.sh"])
    );
    // The edited check file itself is protected, so it's listed there, not as uncovered.
    assert_eq!(
        r["protected_changes"][0]["path"],
        ".countercheck/checks.toml"
    );
}

fn head(p: &Path) -> String {
    let out = Command::new("git")
        .arg("-C")
        .arg(p)
        .args(["rev-parse", "HEAD"])
        .output()
        .unwrap();
    String::from_utf8(out.stdout).unwrap().trim().to_string()
}

#[test]
fn an_edited_protected_test_is_listed_as_protected_not_uncovered() {
    let d = project(COVERS);
    fix_calc(d.path());
    fs::write(d.path().join("tests/check.sh"), "exit 0\n").unwrap();
    let (_, r) = countercheck(d.path(), &[]);
    assert_eq!(r["protected_changes"][0]["path"], "tests/check.sh", "{r:#}");
    assert_eq!(r["uncovered"]["total"], 0, "{r:#}");
}

#[test]
fn a_deleted_file_is_an_uncovered_change() {
    let d = project(COVERS);
    fs::write(d.path().join("gone.sh"), "x\n").unwrap();
    sh(d.path(), "git add gone.sh && git commit -qm g");
    let base = head(d.path());
    fix_calc(d.path());
    fs::remove_file(d.path().join("gone.sh")).unwrap();
    let (_, r) = countercheck(d.path(), &["--base", &base]);
    assert_eq!(
        r["uncovered"]["paths"],
        serde_json::json!(["gone.sh"]),
        "{r:#}"
    );
}

#[test]
fn a_symlink_into_a_covered_folder_is_judged_by_its_own_path() {
    if cfg!(windows) {
        return;
    }
    let d = project(COVERS);
    fix_calc(d.path());
    // A new link outside src/ pointing at the covered file is itself an uncovered change.
    sh(d.path(), "ln -s src/calc.sh link.sh");
    let (_, r) = countercheck(d.path(), &[]);
    assert_eq!(
        r["uncovered"]["paths"],
        serde_json::json!(["link.sh"]),
        "{r:#}"
    );
}

#[test]
fn renaming_into_a_covered_path_still_runs_the_checks() {
    // Moving a file under a covered pattern makes it covered, and the checks then run on it:
    // the worker gains nothing it couldn't do before.
    let d = project(&format!(
        "uncovered = \"unverified\"\n{}",
        COVERS.replace("[\"src/calc.sh\"]", "[\"src/**\"]")
    ));
    fs::write(d.path().join("billing.sh"), "echo 1\n").unwrap();
    sh(d.path(), "git add billing.sh && git commit -qm b");
    let base = head(d.path());
    sh(d.path(), "git mv billing.sh src/billing.sh");
    // calc is still broken: covered or not, the verdict is FAILED.
    let (code, r) = countercheck(d.path(), &["--base", &base]);
    assert_eq!(code, 2, "{r:#}");
    // billing.sh (deleted at the old path) is uncovered; src/billing.sh is covered.
    assert_eq!(
        r["uncovered"]["paths"],
        serde_json::json!(["billing.sh"]),
        "{r:#}"
    );
}

#[test]
fn bad_covers_and_strict_mode_without_covers_are_refused() {
    let d = project(&format!("{CHECKS}covers = [\"src/[\"]\n"));
    let out = Command::new(BIN)
        .args(["verify", "--repo"])
        .arg(d.path())
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&out.stderr).contains("covers pattern"));
    let d = project(&format!("uncovered = \"unverified\"\n{CHECKS}"));
    let out = Command::new(BIN)
        .args(["verify", "--repo"])
        .arg(d.path())
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("needs at least one [[check]] with covers")
    );
}

/// The worker replaces the protected `tests` folder with a symlink to a folder outside the
/// project: restoring the protected files must not write through it.
#[cfg(unix)]
#[test]
fn protected_files_are_never_restored_through_a_symlinked_folder() {
    let d = project(CHECKS);
    let outside = tempfile::tempdir().unwrap();
    fs::write(outside.path().join("check.sh"), WEAK_TEST_OUTSIDE).unwrap();
    sh(d.path(), "git rm -rq tests");
    std::os::unix::fs::symlink(outside.path(), d.path().join("tests")).unwrap();
    let (code, r) = countercheck(d.path(), &[]);
    assert_eq!(code, 2, "{r:#}");
    assert_eq!(r["protected_changes"][0]["path"], "tests/check.sh");
    assert_eq!(r["protected_changes"][0]["action"], "restored_from_base");
    // the folder outside is untouched: nothing was written there
    assert_eq!(
        fs::read_to_string(outside.path().join("check.sh")).unwrap(),
        WEAK_TEST_OUTSIDE
    );
}

#[cfg(unix)]
const WEAK_TEST_OUTSIDE: &str = "exit 0\n";

/// A golden check's output folder is a symlink out of the project: the file there is not
/// deleted before the run, and the output is refused.
#[cfg(unix)]
#[test]
fn golden_output_is_never_cleared_through_a_symlinked_folder() {
    let d = golden_project("");
    let outside = tempfile::tempdir().unwrap();
    fs::write(outside.path().join("totals.csv"), "keep me\n").unwrap();
    std::os::unix::fs::symlink(outside.path(), d.path().join("out")).unwrap();
    // the check only reads, so whatever is in the folder afterwards is what countercheck left
    sh(
        d.path(),
        "sed -i.bak 's#run = .*#run = \"true\"#' .countercheck/checks.toml && rm .countercheck/checks.toml.bak \
         && git commit -qam 'check only reads'",
    );
    let (code, r) = countercheck(d.path(), &[]);
    assert_eq!(code, 2, "{r:#}");
    assert_eq!(
        fs::read_to_string(outside.path().join("totals.csv")).unwrap(),
        "keep me\n"
    );
}

// ---- protected files stored in Git LFS (issue #11) ----

const LFS_REAL: &str = "id\nrow1\n";
/// The check passes only when it sees the real content, not the pointer.
const LFS_CHECKS: &str = "protected = [\"tests/**\"]\n[[check]]\nname = \"fixture\"\nrun = \"grep -qx row1 tests/data.csv\"\ntimeout_secs = 30\n";

/// A project whose protected tests/data.csv is stored in Git LFS: the commit holds the pointer,
/// the local LFS store and the working tree hold the real content (as `git lfs` leaves them).
/// `store` = what goes in the LFS store (None: not on this computer).
fn lfs_project(store: Option<&str>) -> tempfile::TempDir {
    let d = project(LFS_CHECKS);
    let p = d.path();
    let oid = countercheck_core::sha256_hex(LFS_REAL.as_bytes());
    let pointer = format!(
        "version https://git-lfs.github.com/spec/v1\noid sha256:{oid}\nsize {}\n",
        LFS_REAL.len()
    );
    fs::write(p.join("tests/data.csv"), &pointer).unwrap();
    sh(p, "git add -A && git commit -qm 'lfs fixture'");
    if let Some(content) = store {
        let dir = p.join(".git/lfs/objects").join(&oid[..2]).join(&oid[2..4]);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join(&oid), content).unwrap();
    }
    fs::write(p.join("tests/data.csv"), LFS_REAL).unwrap();
    d
}

#[test]
fn an_untouched_lfs_test_file_is_not_a_change_and_checks_see_its_content() {
    let d = lfs_project(Some(LFS_REAL));
    let (code, r) = countercheck(d.path(), &[]);
    assert_eq!(code, 0, "{r:#}");
    assert_eq!(r["protected_changes"], serde_json::json!([]), "{r:#}");
}

#[test]
fn an_edited_lfs_test_file_is_restored_with_its_real_content() {
    let d = lfs_project(Some(LFS_REAL));
    fs::write(d.path().join("tests/data.csv"), "id\nweakened\n").unwrap();
    let (code, r) = countercheck(d.path(), &[]);
    // The check ran with the base content (row1), so it passed; the edit is on the receipt.
    assert_eq!(code, 0, "{r:#}");
    assert_eq!(r["protected_changes"][0]["path"], "tests/data.csv", "{r:#}");
    assert_eq!(r["protected_changes"][0]["action"], "restored_from_base");
}

fn verify_error(dir: &Path) -> (Option<i32>, String) {
    let mut cmd = Command::new(BIN);
    cmd.args(["verify", "--repo"]).arg(dir);
    if nested() {
        cmd.args(["--sandbox", "none"]);
    }
    let out = cmd.output().unwrap();
    (
        out.status.code(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

#[test]
fn an_edited_lfs_test_file_without_its_content_is_a_plain_error() {
    let d = lfs_project(None);
    fs::write(d.path().join("tests/data.csv"), "id\nweakened\n").unwrap();
    let (code, err) = verify_error(d.path());
    assert_eq!(code, Some(1), "{err}");
    assert!(
        err.contains("stored in Git LFS") && err.contains("git lfs pull"),
        "{err}"
    );
}

#[test]
fn a_changed_lfs_store_is_refused() {
    // The worker rewrites the stored content to match its weakened file: the sha256 no longer
    // matches the pointer in the base, so nothing is trusted.
    let d = lfs_project(Some("id\nweakened\n"));
    fs::write(d.path().join("tests/data.csv"), "id\nweakened\n").unwrap();
    let (code, err) = verify_error(d.path());
    assert_eq!(code, Some(1), "{err}");
    assert!(err.contains("does not match its pointer"), "{err}");
}

// ---- watched paths (issue #19) ----

/// Checks that pass once calc is fixed, with tests/** watched (not protected).
const WATCHED: &str = "watched = [\"tests/**\"]\n[[check]]\nname = \"calc\"\nrun = \"sh tests/check.sh\"\ntimeout_secs = 30\n";

#[test]
fn watched_test_edits_are_listed_on_the_receipt() {
    let d = project(WATCHED);
    fix_calc(d.path());
    // The worker edits the existing test (still passing) and adds a new one.
    fs::write(
        d.path().join("tests/check.sh"),
        "# double-checked\n[ \"$(sh src/calc.sh 2 3)\" = 5 ]\n",
    )
    .unwrap();
    fs::write(d.path().join("tests/more.sh"), "true\ntrue\n").unwrap();
    let (code, r) = countercheck(d.path(), &[]);
    assert_eq!(code, 0, "{r:#}");
    assert_eq!(r["verdict"], "verified");
    let w = &r["watched"];
    assert_eq!(w["total"], 2, "{r:#}");
    assert_eq!(w["existing_changed"], 1);
    assert_eq!(w["changes"][0]["path"], "tests/check.sh");
    assert_eq!(w["changes"][0]["change"], "modified");
    assert_eq!(w["changes"][0]["lines_added"], 1);
    assert_eq!(w["changes"][0]["lines_removed"], 0);
    assert_eq!(w["changes"][1]["path"], "tests/more.sh");
    assert_eq!(w["changes"][1]["change"], "added");
    assert_eq!(w["changes"][1]["lines_added"], 2);
    let reasons = r["reasons"].to_string();
    assert!(
        reasons.contains("M tests/check.sh (+1 -0)") && reasons.contains("A tests/more.sh"),
        "{reasons}"
    );
    // Nothing protected changed: the edit stays and is only reported.
    assert_eq!(r["protected_changes"], serde_json::json!([]));
}

#[test]
fn watched_mode_unverified_flags_edits_to_existing_files_only() {
    let checks = format!("watched_mode = \"unverified\"\n{WATCHED}");
    // Only a new test: still VERIFIED.
    let d = project(&checks);
    fix_calc(d.path());
    fs::write(d.path().join("tests/more.sh"), "true\n").unwrap();
    let (code, r) = countercheck(d.path(), &[]);
    assert_eq!(code, 0, "{r:#}");
    assert_eq!(r["verdict"], "verified");
    // Weakening the existing test: UNVERIFIED (exit 3), with the file named.
    let d = project(&checks);
    fix_calc(d.path());
    fs::write(d.path().join("tests/check.sh"), "true\n").unwrap();
    let (code, r) = countercheck(d.path(), &[]);
    assert_eq!(code, 3, "{r:#}");
    assert_eq!(r["verdict"], "unverified");
    assert!(
        r["reasons"].to_string().contains("M tests/check.sh"),
        "{r:#}"
    );
    // Deleting an existing watched file, with a failing check: FAILED stays FAILED.
    let failing = "watched_mode = \"unverified\"\nwatched = [\"tests/**\"]\n[[check]]\nname = \"t\"\nrun = \"sh tests/other.sh\"\n";
    let d = project(failing);
    fs::remove_file(d.path().join("tests/check.sh")).unwrap();
    fs::write(d.path().join("tests/other.sh"), "exit 1\n").unwrap();
    let (code, r) = countercheck(d.path(), &[]);
    assert_eq!(code, 2, "{r:#}");
    assert_eq!(r["verdict"], "failed");
    let changes = r["watched"]["changes"].to_string();
    assert!(changes.contains("\"deleted\""), "{r:#}");
}

#[test]
fn watched_comes_from_the_base_and_protected_wins() {
    let checks = format!("protected = [\"tests/check.sh\"]\n{WATCHED}");
    let d = project(&checks);
    fix_calc(d.path());
    fs::write(d.path().join("tests/more.sh"), "true\n").unwrap();
    // The worker drops `watched` from its copy of the check file and edits the protected test.
    fs::write(
        d.path().join(".countercheck/checks.toml"),
        "[[check]]\nname = \"calc\"\nrun = \"true\"\n",
    )
    .unwrap();
    fs::write(d.path().join("tests/check.sh"), "true\n").unwrap();
    let (code, r) = countercheck(d.path(), &[]);
    assert_eq!(code, 0, "{r:#}");
    // The base check file still watches tests/**: the new test is listed...
    let paths: Vec<&str> = r["watched"]["changes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["path"].as_str().unwrap())
        .collect();
    assert_eq!(paths, vec!["tests/more.sh"], "{r:#}");
    // ...and the protected test is restored and reported as protected, not as watched.
    let restored: Vec<&str> = r["protected_changes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["path"].as_str().unwrap())
        .collect();
    assert!(restored.contains(&"tests/check.sh"), "{r:#}");
}

#[test]
fn no_watched_section_without_watched_paths() {
    let d = project(CHECKS);
    fix_calc(d.path());
    let (_, r) = countercheck(d.path(), &[]);
    assert!(r.get("watched").is_none(), "{r:#}");
}

// ---- forbid scope = "added" (issue #17) ----

const FORBID_ADDED: &str = "[[check]]\nname = \"calc\"\nrun = \"sh tests/check.sh\"\ntimeout_secs = 30\n[[forbid]]\nname = \"no skipped tests\"\npattern = 'skip\\('\npaths = [\"tests/**\"]\nscope = \"added\"\n";

/// A project whose base already has one legitimate skip in tests/.
fn project_with_a_skip(checks: &str) -> tempfile::TempDir {
    let d = project(checks);
    fs::write(
        d.path().join("tests/legacy.py"),
        "def test_db():\n    skip(\"needs a database\")\n",
    )
    .unwrap();
    sh(d.path(), "git add -A && git commit -qm legacy");
    d
}

#[test]
fn forbid_added_ignores_old_lines_and_catches_new_ones() {
    // The old skip alone passes.
    let d = project_with_a_skip(FORBID_ADDED);
    fix_calc(d.path());
    let (code, r) = countercheck(d.path(), &[]);
    assert_eq!(code, 0, "{r:#}");
    let f = r["checks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["name"] == "forbid (added lines): no skipped tests")
        .unwrap_or_else(|| panic!("{r:#}"));
    assert_eq!(f["passed"], true);
    // A new skip in the same file and one in a new file: FAILED, only the new lines named.
    fs::write(
        d.path().join("tests/legacy.py"),
        "def test_db():\n    skip(\"needs a database\")\n\ndef test_calc():\n    skip(\"flaky\")\n",
    )
    .unwrap();
    fs::write(d.path().join("tests/new.py"), "skip(\"later\")\n").unwrap();
    let (code, r) = countercheck(d.path(), &[]);
    assert_eq!(code, 2, "{r:#}");
    let f = r["checks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["name"] == "forbid (added lines): no skipped tests")
        .unwrap();
    let out = f["output_tail"].as_str().unwrap();
    assert!(
        out.contains("2 new (added since the base) forbidden line(s)"),
        "{out}"
    );
    assert!(
        out.contains("tests/legacy.py:5:") && out.contains("tests/new.py:1:"),
        "{out}"
    );
    assert!(
        !out.contains("tests/legacy.py:2:"),
        "the old skip is not new: {out}"
    );
}

#[test]
fn forbid_added_catches_a_copied_old_line_and_scope_all_is_unchanged() {
    // Copying the existing skip line still adds a skip.
    let d = project_with_a_skip(FORBID_ADDED);
    fix_calc(d.path());
    fs::write(
        d.path().join("tests/legacy.py"),
        "def test_db():\n    skip(\"needs a database\")\n\ndef test_x():\n    skip(\"needs a database\")\n",
    )
    .unwrap();
    let (code, r) = countercheck(d.path(), &[]);
    assert_eq!(code, 2, "{r:#}");
    // Without `scope`, the old line fails as before.
    let all = FORBID_ADDED.replace("scope = \"added\"\n", "");
    let d = project_with_a_skip(&all);
    fix_calc(d.path());
    let (code, r) = countercheck(d.path(), &[]);
    assert_eq!(code, 2, "{r:#}");
    assert!(
        r["checks"].to_string().contains("forbid: no skipped tests"),
        "{r:#}"
    );
}

// ---- extra write locations per check (issue #18) ----

/// `verify` that is expected to stop with an error: (exit code, stderr).
#[cfg(target_os = "macos")]
fn verify_fails(dir: &Path) -> (Option<i32>, String) {
    let out = Command::new(BIN)
        .args(["verify", "--repo"])
        .arg(dir)
        .output()
        .unwrap();
    (
        out.status.code(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

#[cfg(target_os = "macos")]
#[test]
fn writes_lets_one_check_write_there_and_nowhere_else() {
    if nested() {
        return;
    }
    let outside = tempfile::tempdir().unwrap();
    let base = outside.path().canonicalize().unwrap();
    let allowed = base.join("runner-scratch");
    let other = base.join("other");
    let checks = format!(
        "[[check]]\nname = \"allowed\"\nrun = \"mkdir -p '{a}/sub' && echo x > '{a}/sub/f'\"\nwrites = [\"{a}\"]\n\
         [[check]]\nname = \"elsewhere\"\nrun = \"echo x > '{o}'\"\nwrites = [\"{a}\"]\n\
         [[check]]\nname = \"not listed\"\nrun = \"echo x > '{a}/g'\"\n",
        a = allowed.display(),
        o = other.display()
    );
    let d = project(&checks);
    let (code, r) = countercheck(d.path(), &[]);
    assert_eq!(code, 2, "{r:#}");
    let by = |n: &str| {
        r["checks"]
            .as_array()
            .unwrap()
            .iter()
            .find(|c| c["name"] == n)
            .unwrap()
            .clone()
    };
    assert_eq!(by("allowed")["passed"], true, "{r:#}");
    assert!(allowed.join("sub/f").exists());
    assert_eq!(by("allowed")["writes"][0], allowed.display().to_string());
    // Only that location, and only for the check that lists it.
    assert_eq!(by("elsewhere")["passed"], false);
    assert!(!other.exists());
    assert_eq!(by("not listed")["passed"], false);
    assert!(!allowed.join("g").exists());
    assert!(by("not listed").get("writes").is_none());
}

#[cfg(target_os = "macos")]
#[test]
fn writes_never_reaches_the_repository_or_home() {
    if nested() {
        return;
    }
    // The repository itself, a folder inside it, and home are refused before anything runs.
    for target in ["{repo}", "{repo}/tests", "{home}", "/Users"] {
        let p = project("[[check]]\nname = \"c\"\nrun = \"true\"\n");
        let proj = p.path().canonicalize().unwrap();
        let w = target.replace("{repo}", &proj.display().to_string());
        fs::write(
            p.path().join(".countercheck/checks.toml"),
            format!("[[check]]\nname = \"c\"\nrun = \"true\"\nwrites = [\"{w}\"]\n"),
        )
        .unwrap();
        sh(p.path(), "git commit -qam 'writes'");
        let (code, err) = verify_fails(p.path());
        assert_eq!(code, Some(1), "{target}: {err}");
        assert!(err.contains("writes path"), "{target}: {err}");
    }
    // A symlink that points into the repository is followed and refused too.
    let outside = tempfile::tempdir().unwrap();
    let link = outside.path().canonicalize().unwrap().join("innocent");
    let p = project("[[check]]\nname = \"c\"\nrun = \"true\"\n");
    std::os::unix::fs::symlink(p.path().join("src"), &link).unwrap();
    fs::write(
        p.path().join(".countercheck/checks.toml"),
        format!(
            "[[check]]\nname = \"c\"\nrun = \"true\"\nwrites = [\"{}\"]\n",
            link.display()
        ),
    )
    .unwrap();
    sh(p.path(), "git commit -qam 'writes via a symlink'");
    let (code, err) = verify_fails(p.path());
    assert_eq!(code, Some(1), "{err}");
    assert!(err.contains("repository"), "{err}");
}

#[cfg(target_os = "macos")]
#[test]
fn a_worker_cannot_grant_itself_writes() {
    if nested() {
        return;
    }
    let outside = tempfile::tempdir().unwrap();
    let target = outside.path().canonicalize().unwrap().join("leak");
    let checks = format!(
        "[[check]]\nname = \"c\"\nrun = \"echo x > '{}'\"\n",
        target.display()
    );
    let d = project(&checks);
    // The worker adds `writes` to its copy of the check file: the base version is used.
    fs::write(
        d.path().join(".countercheck/checks.toml"),
        format!("{checks}writes = [\"{}\"]\n", target.display()),
    )
    .unwrap();
    let (code, r) = countercheck(d.path(), &[]);
    assert_eq!(code, 2, "{r:#}");
    assert!(!target.exists());
    assert!(r["checks"][0].get("writes").is_none(), "{r:#}");
}

// ---- the Linux sandbox (Landlock + namespaces) ----

/// What `countercheck sandbox-exec --probe` says this Linux system can do: (landlock, namespaces).
#[cfg(target_os = "linux")]
fn linux_caps() -> (bool, bool) {
    let out = Command::new(BIN)
        .args(["sandbox-exec", "--probe"])
        .output()
        .unwrap();
    let s = String::from_utf8_lossy(&out.stdout).to_string();
    (s.contains("landlock=1"), s.contains("namespaces=1"))
}

#[cfg(target_os = "linux")]
#[test]
fn the_linux_sandbox_confines_writes_and_says_what_it_enforced() {
    let (landlock, namespaces) = linux_caps();
    if !landlock || nested() {
        eprintln!("skipped: no Landlock on this kernel");
        return;
    }
    let outside = tempfile::tempdir().unwrap();
    let leak = outside.path().canonicalize().unwrap().join("leak.txt");
    let checks = format!(
        "[[check]]\nname = \"inside\"\nrun = \"mkdir -p a/b && echo x > a/b/c\"\n\
         [[check]]\nname = \"outside\"\nrun = \"echo x > '{}'\"\n",
        leak.display()
    );
    let d = project(&checks);
    let (code, r) = countercheck(d.path(), &[]);
    assert_eq!(code, 2, "{r:#}");
    let sandbox = r["sandbox"].as_str().unwrap();
    assert!(sandbox.starts_with("linux-landlock"), "{sandbox}");
    assert_eq!(sandbox.contains("no internet"), namespaces, "{sandbox}");
    assert_eq!(r["checks"][0]["passed"], true, "{r:#}");
    assert_eq!(r["checks"][1]["passed"], false);
    assert!(!leak.exists());
    let tail = r["checks"][1]["output_tail"].as_str().unwrap();
    assert!(
        tail.contains("tried to write outside the check folder"),
        "{tail}"
    );
    // `writes` opens exactly that location.
    let checks = format!(
        "[[check]]\nname = \"allowed\"\nrun = \"echo x > '{}'\"\nwrites = [\"{}\"]\n",
        leak.display(),
        outside.path().canonicalize().unwrap().display()
    );
    let d = project(&checks);
    let (code, r) = countercheck(d.path(), &[]);
    assert_eq!(code, 0, "{r:#}");
    assert!(leak.exists());
}

#[cfg(target_os = "linux")]
#[test]
fn the_linux_sandbox_kills_what_a_check_detached_with_setsid() {
    let (landlock, _) = linux_caps();
    if !landlock || nested() {
        eprintln!("skipped: no Landlock on this kernel");
        return;
    }
    // Found by a unique command line, which every PID namespace shows the same way (a pid the
    // daemon prints would be its pid inside the check's namespace).
    let marker = format!("{}", 40_000 + std::process::id() % 20_000);
    let checks = format!(
        "[[check]]\nname = \"daemon\"\nrun = \"setsid sh -c 'exec sleep {marker}' & sleep 0.5; exit 0\"\n"
    );
    let d = project(&checks);
    let (code, r) = countercheck(d.path(), &[]);
    assert_eq!(code, 0, "{r:#}");
    let alive = || {
        Command::new("pgrep")
            .args(["-f", &format!("^sleep {marker}$")])
            .output()
            .is_ok_and(|o| o.status.success())
    };
    for _ in 0..30 {
        if !alive() {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    let _ = Command::new("pkill")
        .args(["-f", &format!("^sleep {marker}$")])
        .status();
    panic!("the setsid daemon (sleep {marker}) survived its check");
}

#[cfg(target_os = "linux")]
#[test]
fn with_namespaces_the_internet_is_blocked_but_localhost_works() {
    let (landlock, namespaces) = linux_caps();
    if !landlock || !namespaces || nested() {
        eprintln!("skipped: no user namespaces here");
        return;
    }
    let checks = "[[check]]\nname = \"internet\"\nrun = \"python3 -c \\\"import socket; socket.create_connection(('1.1.1.1', 443), timeout=5)\\\"\"\n\
        [[check]]\nname = \"localhost\"\nrun = \"python3 -c \\\"import socket; s=socket.socket(); s.bind(('127.0.0.1', 0)); s.listen(); socket.create_connection(s.getsockname(), timeout=5)\\\"\"\n\
        [[check]]\nname = \"allowed\"\nrun = \"python3 -c \\\"import socket; socket.getaddrinfo('localhost', 80)\\\"\"\nnetwork = true\n";
    let d = project(checks);
    let (_, r) = countercheck(d.path(), &[]);
    assert_eq!(r["checks"][0]["passed"], false, "{r:#}");
    assert!(
        r["checks"][0]["output_tail"]
            .as_str()
            .unwrap()
            .contains("network = true"),
        "{r:#}"
    );
    assert_eq!(r["checks"][1]["passed"], true, "{r:#}");
    assert_eq!(r["checks"][2]["passed"], true, "{r:#}");
}

// ---- the Windows sandbox (write-restricted token + Job Object) ----
//
// Git for Windows' sh (MSYS) can't start under the sandbox's token, so these checks use
// `shell = "cmd"` and `"pwsh"`; a run with sh checks runs without the sandbox and says so.

/// What `countercheck sandbox-exec --probe` says about this Windows machine. On CI the sandbox
/// must work (that runner is where it is proven); elsewhere the tests below skip without it.
#[cfg(windows)]
fn windows_sandbox_works() -> bool {
    let out = Command::new(BIN)
        .args(["sandbox-exec", "--probe"])
        .output()
        .unwrap();
    let ok = String::from_utf8_lossy(&out.stdout).contains("restricted=1");
    // Printed so a run with --nocapture shows each sandbox test really used the sandbox.
    eprintln!("windows sandbox probe: restricted={}", u8::from(ok));
    if std::env::var_os("CI").is_some() || std::env::var_os("GITHUB_ACTIONS").is_some() {
        assert!(
            ok,
            "the Windows sandbox probe failed on CI: {}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
    }
    ok && !nested()
}

/// The check by name.
#[cfg(windows)]
fn by_name(r: &serde_json::Value, name: &str) -> serde_json::Value {
    r["checks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["name"] == name)
        .unwrap_or_else(|| panic!("no check {name}: {r:#}"))
        .clone()
}

/// `icacls <path>`: the permissions as Windows prints them (unknown SIDs as `S-1-5-21-...`).
#[cfg(windows)]
fn icacls(p: &Path) -> String {
    let out = Command::new("icacls").arg(p).output().unwrap();
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .skip(1)
        .filter(|l| !l.contains("Successfully processed"))
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(windows)]
#[test]
fn the_windows_sandbox_confines_writes_and_says_what_it_enforced() {
    if !windows_sandbox_works() {
        eprintln!("skipped: the Windows sandbox can't start here");
        return;
    }
    let outside = tempfile::tempdir().unwrap();
    let leak = outside.path().join("leak.txt");
    let ps_leak = outside.path().join("ps-leak.txt");
    let py_leak = outside.path().join("py-leak.txt");
    // The repository's own path is only known once it exists: commit the checks again with it.
    let p = project("[[check]]\nname = \"c\"\nshell = \"cmd\"\nrun = \"exit /b 0\"\n");
    let checks = format!(
        r#"[[check]]
name = "inside"
shell = "cmd"
run = 'mkdir a\b && echo x> a\b\c && echo y> "%TEMP%\t" && echo z> src\calc.sh'
[[check]]
name = "outside"
shell = "cmd"
run = 'echo x> "{leak}"'
[[check]]
name = "pwsh outside"
shell = "pwsh"
run = "Set-Content -Path '{ps}' -Value x"
[[check]]
name = "python outside"
shell = "cmd"
run = "python -c \"open(r'{py}', 'w')\""
[[check]]
name = "the worker's files"
shell = "cmd"
run = 'echo x> "{repo}\src\calc.sh"'
"#,
        leak = leak.display(),
        // A TOML basic string: forward slashes (PowerShell takes them), no backslash escapes.
        ps = slash(&ps_leak),
        py = slash(&py_leak),
        repo = p.path().display(),
    );
    fs::write(p.path().join(".countercheck/checks.toml"), checks).unwrap();
    sh(p.path(), "git commit -qam 'checks with the repo path'");
    let before = fs::read(p.path().join("src/calc.sh")).unwrap();
    let (code, r) = countercheck(p.path(), &[]);
    assert_eq!(code, 2, "{r:#}");
    let sandbox = r["sandbox"].as_str().unwrap();
    assert!(
        sandbox.starts_with("windows-restricted-token (writes only in the check folder"),
        "{sandbox}"
    );
    assert!(sandbox.contains("internet NOT blocked"), "{sandbox}");
    assert_eq!(by_name(&r, "inside")["passed"], true, "{r:#}");
    for name in [
        "outside",
        "pwsh outside",
        "python outside",
        "the worker's files",
    ] {
        assert_eq!(by_name(&r, name)["passed"], false, "{name}: {r:#}");
    }
    assert!(!leak.exists() && !ps_leak.exists() && !py_leak.exists());
    assert_eq!(fs::read(p.path().join("src/calc.sh")).unwrap(), before);
    // The path is named when the tool prints it; cmd's bare "Access is denied." gets the
    // general write hint.
    for name in ["pwsh outside", "python outside"] {
        let tail = by_name(&r, name)["output_tail"]
            .as_str()
            .unwrap()
            .to_string();
        assert!(
            tail.contains("tried to write outside the check folder"),
            "{name}: {tail}"
        );
    }
    let tail = by_name(&r, "outside")["output_tail"]
        .as_str()
        .unwrap()
        .to_string();
    assert!(tail.contains("write outside the check folder"), "{tail}");
}

#[cfg(windows)]
#[test]
fn the_windows_sandbox_writes_list_opens_exactly_that_folder_and_is_undone() {
    if !windows_sandbox_works() {
        return;
    }
    let outside = tempfile::tempdir().unwrap();
    let allowed = outside.path().join("runner-scratch");
    let other = outside.path().join("other");
    fs::create_dir_all(allowed.join("old")).unwrap();
    fs::write(allowed.join("old/keep.txt"), "before").unwrap();
    let acl_before = icacls(&allowed);
    let checks = format!(
        r#"[[check]]
name = "allowed"
shell = "cmd"
run = 'mkdir "{a}\sub" && echo x> "{a}\sub\f" && echo after> "{a}\old\keep.txt"'
writes = ["{w}"]
[[check]]
name = "elsewhere"
shell = "cmd"
run = 'echo x> "{o}"'
writes = ["{w}"]
[[check]]
name = "not listed"
shell = "cmd"
run = 'echo x> "{a}\g"'
"#,
        a = allowed.display(),
        w = slash(&allowed),
        o = other.display()
    );
    let d = project(&checks);
    let (code, r) = countercheck(d.path(), &[]);
    assert_eq!(code, 2, "{r:#}");
    assert_eq!(by_name(&r, "allowed")["passed"], true, "{r:#}");
    assert!(allowed.join("sub/f").exists());
    assert_eq!(
        fs::read_to_string(allowed.join("old/keep.txt"))
            .unwrap()
            .trim(),
        "after",
        "a file already there can be changed too"
    );
    let listed = by_name(&r, "allowed")["writes"][0]
        .as_str()
        .unwrap()
        .to_lowercase();
    assert!(
        listed.ends_with("runner-scratch"),
        "the receipt lists the location: {listed}"
    );
    // Only that location, and only for the check that lists it.
    assert_eq!(by_name(&r, "elsewhere")["passed"], false);
    assert!(!other.exists());
    assert_eq!(by_name(&r, "not listed")["passed"], false);
    assert!(!allowed.join("g").exists());
    // The grant is taken off again: the folder's permissions are as before, and nothing the
    // check made there keeps an entry for the run's SID.
    assert_eq!(icacls(&allowed), acl_before);
    for p in [
        allowed.join("sub"),
        allowed.join("sub/f"),
        allowed.join("old/keep.txt"),
    ] {
        let acl = icacls(&p);
        assert!(!acl.contains("S-1-5-21-"), "{}: {acl}", p.display());
    }
}

#[cfg(windows)]
#[test]
fn the_windows_sandbox_keeps_localhost_and_says_the_internet_is_open() {
    if !windows_sandbox_works() {
        return;
    }
    let d = project(
        "[[check]]\nname = \"localhost\"\nshell = \"cmd\"\nrun = \"python tests/net.py localhost\"\n\
         [[check]]\nname = \"internet\"\nshell = \"cmd\"\nrun = \"python tests/net.py internet\"\n\
         [[check]]\nname = \"network true\"\nshell = \"cmd\"\nrun = \"python tests/net.py internet\"\nnetwork = true\n",
    );
    fs::write(
        d.path().join("tests/net.py"),
        "import socket, subprocess, sys\n\
         if sys.argv[1] == 'internet':\n    socket.create_connection(('1.1.1.1', 443), timeout=10).close()\n\
         else:\n    s = socket.socket(); s.bind(('127.0.0.1', 0)); s.listen()\n    port = s.getsockname()[1]\n\
         \x20   # The client is another process, as with a test server and a browser.\n\
         \x20   subprocess.run([sys.executable, '-c', f\"import socket; socket.create_connection(('127.0.0.1', {port}), timeout=10).close()\"], check=True)\n\
         \x20   s.accept()[0].close()\n\
         print('ok', sys.argv[1])\n",
    )
    .unwrap();
    sh(d.path(), "git add -A && git commit -qm net");
    let (code, r) = countercheck(d.path(), &[]);
    assert_eq!(code, 0, "{r:#}");
    // Honest label: the internet is reachable, and the receipt says so.
    assert!(
        r["sandbox"]
            .as_str()
            .unwrap()
            .contains("internet NOT blocked")
    );
    for name in ["localhost", "internet", "network true"] {
        assert_eq!(by_name(&r, name)["passed"], true, "{name}: {r:#}");
    }
}

#[cfg(windows)]
#[test]
fn the_windows_sandbox_stops_what_a_check_left_running() {
    if !windows_sandbox_works() {
        return;
    }
    // A program started with Start-Process outlives its check unless the Job Object stops it;
    // the second check also times out with it running.
    let start = "$p = Start-Process -PassThru -WindowStyle Hidden ping -ArgumentList '-n','60','127.0.0.1'; [Console]::Out.WriteLine(\\\"WINPID=$($p.Id)\\\"); [Console]::Out.Flush()";
    let d = project(&format!(
        "[[check]]\nname = \"detached\"\nshell = \"pwsh\"\nrun = \"{start}\"\n\
         [[check]]\nname = \"timeout\"\nshell = \"pwsh\"\nrun = \"{start}; Start-Sleep 60\"\ntimeout_secs = 5\n"
    ));
    let started = std::time::Instant::now();
    let (code, r) = countercheck(d.path(), &[]);
    assert!(
        started.elapsed().as_secs() < 60,
        "verify took {:?}: something outlived its check",
        started.elapsed()
    );
    assert_eq!(code, 2, "{r:#}");
    assert_eq!(by_name(&r, "detached")["passed"], true, "{r:#}");
    assert_eq!(by_name(&r, "timeout")["timed_out"], true, "{r:#}");
    for name in ["detached", "timeout"] {
        let c = by_name(&r, name);
        let tail = c["output_tail"].as_str().unwrap();
        let pid = tail
            .lines()
            .find_map(|l| l.trim().strip_prefix("WINPID="))
            .unwrap_or_else(|| panic!("{name}: no pid in {tail}"))
            .trim()
            .to_string();
        assert!(pid.parse::<u32>().is_ok(), "{name}: {pid:?}");
        assert!(
            !windows_pid_running(&pid),
            "{name}: background process {pid} survived its check"
        );
    }
}

/// Git for Windows' sh can't start in the sandbox: in a run with sh checks, those run without it
/// and each says so on the receipt, the cmd/pwsh checks still run in it, and the label says the
/// run was partial. `--sandbox windows` refuses such a run.
#[cfg(windows)]
#[test]
fn sh_checks_run_unsandboxed_and_say_so_while_the_others_stay_sandboxed() {
    if !windows_sandbox_works() {
        return;
    }
    let outside = tempfile::tempdir().unwrap();
    let sh_file = outside.path().join("from-sh.txt");
    let cmd_file = outside.path().join("from-cmd.txt");
    let checks = format!(
        r#"[[check]]
name = "sh"
run = "echo x > '{sh}'"
[[check]]
name = "cmd"
shell = "cmd"
run = 'echo x> "{cmd}"'
"#,
        sh = slash(&sh_file),
        cmd = cmd_file.display()
    );
    let d = project(&checks);
    let (code, r) = countercheck(d.path(), &[]);
    assert_eq!(code, 2, "{r:#}");
    let sandbox = r["sandbox"].as_str().unwrap();
    assert!(
        sandbox.starts_with(
            "windows-restricted-token, partial: sh checks ran unsandboxed (Git for Windows can't run in a restricted token)"
        ),
        "{sandbox}"
    );
    // The sh check really ran outside the sandbox (its write went through), and says so.
    let sh = by_name(&r, "sh");
    assert_eq!(sh["passed"], true, "{r:#}");
    assert!(sh_file.exists());
    assert_eq!(sh["sandboxed"], false, "{r:#}");
    assert!(
        sh["unsandboxed_reason"]
            .as_str()
            .unwrap()
            .contains("Git for Windows' sh"),
        "{r:#}"
    );
    // The cmd check in the same run stayed in it.
    let cmd = by_name(&r, "cmd");
    assert_eq!(cmd["passed"], false, "{r:#}");
    assert!(!cmd_file.exists());
    assert!(cmd.get("sandboxed").is_none(), "{r:#}");
    // The text receipt marks the unsandboxed check too.
    let out = Command::new(BIN)
        .args(["verify", "--repo"])
        .arg(d.path())
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.contains("NOT sandboxed"), "{text}");
    // Asked for by name, the sandbox refuses a run it can't fully confine.
    let out = Command::new(BIN)
        .args(["verify", "--sandbox", "windows", "--repo"])
        .arg(d.path())
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1), "{out:?}");
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("shell = \"pwsh\" or \"cmd\""), "{err}");
}

/// `--probe` prints only its answer: the deliberately refused write's "Access is denied." stays off
/// the console.
#[cfg(windows)]
#[test]
fn the_windows_sandbox_probe_prints_only_its_answer() {
    if !windows_sandbox_works() {
        return;
    }
    let out = Command::new(BIN)
        .args(["sandbox-exec", "--probe"])
        .output()
        .unwrap();
    assert_eq!(
        String::from_utf8_lossy(&out.stdout).trim(),
        "restricted=1",
        "{out:?}"
    );
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(!err.contains("Access is denied"), "{err}");
}

/// If the sandbox can't be set up, the check fails; it never runs with less than the label says.
#[cfg(windows)]
#[test]
fn the_windows_sandbox_fails_the_check_when_it_cannot_start() {
    let work = tempfile::tempdir().unwrap();
    let marker = work.path().join("ran.txt");
    let out = Command::new(BIN)
        .args(["sandbox-exec", "--work"])
        .arg(work.path())
        .args(["--sid", "S-1-not-a-sid", "--cmdline"])
        .arg(format!("cmd.exe /D /C \"echo x>{}\"", marker.display()))
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(125), "{out:?}");
    assert!(!marker.exists(), "the check ran without its sandbox");
}

// ---- checks from outside the repository (issue #20, ADR-0002) ----

/// A suite repository with `suites/proj.toml` (CHECKS) committed: (dir, full commit id).
fn suite_repo(checks: &str) -> (tempfile::TempDir, String) {
    let d = tempfile::tempdir().unwrap();
    sh(d.path(), common::GIT_INIT);
    fs::create_dir_all(d.path().join("suites")).unwrap();
    fs::write(d.path().join("suites/proj.toml"), checks).unwrap();
    sh(d.path(), "git add -A && git commit -qm suite");
    let out = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(d.path())
        .output()
        .unwrap();
    (d, String::from_utf8(out.stdout).unwrap().trim().to_string())
}

/// The target project without a check file of its own.
fn project_without_checks() -> tempfile::TempDir {
    let d = project(CHECKS);
    sh(
        d.path(),
        "git rm -q .countercheck/checks.toml && git commit -qm 'no countercheck here'",
    );
    d
}

/// verify with extra args; (exit code, receipt JSON or null, stderr).
fn verify_with(dir: &Path, args: &[&str]) -> (i32, serde_json::Value, String) {
    let mut cmd = Command::new(BIN);
    cmd.args(["verify", "--json", "--repo"]).arg(dir).args(args);
    if nested() {
        cmd.args(["--sandbox", "none"]);
    }
    let out = cmd.output().unwrap();
    (
        out.status.code().unwrap_or(-1),
        serde_json::from_slice(&out.stdout).unwrap_or(serde_json::Value::Null),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

#[test]
fn checks_from_a_suite_commit_lock_the_checks() {
    let (suite, commit) = suite_repo(CHECKS);
    let src = format!("{}@{commit}:suites/proj.toml", suite.path().display());
    let d = project_without_checks();
    // The defect: FAILED.
    let (code, r, err) = verify_with(d.path(), &["--checks-from", &src]);
    assert_eq!(code, 2, "{err}{r:#}");
    assert_eq!(r["checks_locked"], true);
    assert_eq!(r["checks_external"]["kind"], "git");
    assert_eq!(r["checks_external"]["pinned"], true);
    assert!(
        r["checks_source"]
            .as_str()
            .unwrap()
            .starts_with("external: "),
        "{r:#}"
    );
    // Weakening the protected test doesn't help: it comes back from the target's --base.
    fs::write(d.path().join("tests/check.sh"), "true\n").unwrap();
    let (code, r, _) = verify_with(d.path(), &["--checks-from", &src]);
    assert_eq!(code, 2, "{r:#}");
    assert_eq!(r["protected_changes"][0]["action"], "restored_from_base");
    // The honest fix: VERIFIED.
    sh(d.path(), "git checkout -q tests/check.sh");
    fs::write(d.path().join("src/calc.sh"), "echo $(($1 + $2))\n").unwrap();
    let (code, r, err) = verify_with(d.path(), &["--checks-from", &src]);
    assert_eq!(code, 0, "{err}{r:#}");
    assert_eq!(r["verdict"], "verified");
}

#[test]
fn external_checks_replace_the_repository_s_own() {
    // The repository's own check file passes anything; the suite's doesn't.
    let d = project("[[check]]\nname = \"always\"\nrun = \"true\"\n");
    let (suite, commit) = suite_repo(CHECKS);
    let src = format!("{}@{commit}:suites/proj.toml", suite.path().display());
    let (code, r, err) = verify_with(d.path(), &["--checks-from", &src]);
    assert_eq!(code, 2, "{err}{r:#}");
    assert!(
        r["checks_source"]
            .as_str()
            .unwrap()
            .contains("own .countercheck/checks.toml was ignored"),
        "{r:#}"
    );
    assert!(!r["checks"].to_string().contains("always"), "{r:#}");
}

#[test]
fn checks_from_needs_a_full_commit_id_and_a_real_path() {
    let (suite, commit) = suite_repo(CHECKS);
    let d = project_without_checks();
    let base = suite.path().display().to_string();
    for (src, want) in [
        (format!("{base}@main:suites/proj.toml"), "full commit id"),
        (
            format!("{base}@{}:suites/proj.toml", &commit[..7]),
            "full commit id",
        ),
        (
            format!("{base}@{commit}:suites/missing.toml"),
            "is not in commit",
        ),
        (format!("{base}@{commit}:../escape.toml"), "unsafe path"),
        (
            format!("{base}@{}:suites/proj.toml", "0".repeat(40)),
            "--checks-from",
        ),
    ] {
        let (code, _, err) = verify_with(d.path(), &["--checks-from", &src]);
        assert_eq!(code, 1, "{src}: {err}");
        assert!(err.contains(want), "{src}: {err}");
    }
}

#[test]
fn a_rewritten_object_in_the_suite_is_refused() {
    let (suite, commit) = suite_repo(CHECKS);
    let git = |args: &[&str]| {
        let out = Command::new("git")
            .args(args)
            .current_dir(suite.path())
            .output()
            .unwrap();
        String::from_utf8(out.stdout).unwrap().trim().to_string()
    };
    let old = git(&["rev-parse", "HEAD:suites/proj.toml"]);
    fs::write(
        suite.path().join("suites/proj.toml"),
        "[[check]]\nname = \"c\"\nrun = \"true\"\n",
    )
    .unwrap();
    let new = git(&["hash-object", "-w", "suites/proj.toml"]);
    let object = |id: &str| {
        suite
            .path()
            .join(".git/objects")
            .join(&id[..2])
            .join(&id[2..])
    };
    let mut perms = fs::metadata(object(&old)).unwrap().permissions();
    #[allow(clippy::permissions_set_readonly_false)]
    perms.set_readonly(false);
    fs::set_permissions(object(&old), perms).unwrap();
    fs::copy(object(&new), object(&old)).unwrap();
    let d = project_without_checks();
    let src = format!("{}@{commit}:suites/proj.toml", suite.path().display());
    let (code, _, err) = verify_with(d.path(), &["--checks-from", &src]);
    assert_eq!(code, 1, "{err}");
    assert!(err.contains("does not match its id"), "{err}");
}

#[test]
fn a_checks_file_is_locked_only_by_its_pin() {
    let outside = tempfile::tempdir().unwrap();
    let file = outside.path().join("proj.toml");
    fs::write(&file, CHECKS).unwrap();
    let sha = countercheck_core::sha256_hex(CHECKS.as_bytes());
    let f = file.to_str().unwrap();
    let d = project_without_checks();
    fs::write(d.path().join("src/calc.sh"), "echo $(($1 + $2))\n").unwrap();
    // Pinned: VERIFIED.
    let (code, r, err) = verify_with(d.path(), &["--checks-from", f, "--checks-sha256", &sha]);
    assert_eq!(code, 0, "{err}{r:#}");
    assert_eq!(r["checks_external"]["kind"], "file");
    assert_eq!(r["checks_external"]["pinned"], true);
    // Not pinned: UNVERIFIED, with the reason.
    let (code, r, _) = verify_with(d.path(), &["--checks-from", f]);
    assert_eq!(code, 3, "{r:#}");
    assert!(r["reasons"].to_string().contains("not pinned"), "{r:#}");
    // The file changed after it was pinned (an agent with a shell can do that): an error.
    fs::write(&file, "[[check]]\nname = \"c\"\nrun = \"true\"\n").unwrap();
    let (code, _, err) = verify_with(d.path(), &["--checks-from", f, "--checks-sha256", &sha]);
    assert_eq!(code, 1, "{err}");
    assert!(err.contains("not the pinned"), "{err}");
    // A file inside the repository is refused, pinned or not.
    let inside = d.path().join("suite.toml");
    fs::write(&inside, CHECKS).unwrap();
    let (code, _, err) = verify_with(
        d.path(),
        &[
            "--checks-from",
            inside.to_str().unwrap(),
            "--checks-sha256",
            &sha,
        ],
    );
    assert_eq!(code, 1, "{err}");
    assert!(err.contains("inside the repository"), "{err}");
    // A file that names other files is refused: each would need its own pin.
    let golden = "[[check]]\nname = \"g\"\nrun = \"true\"\ngolden = { output = \"o.txt\", expected = \".countercheck/e.txt\" }\n";
    fs::write(&file, golden).unwrap();
    let (code, _, err) = verify_with(
        d.path(),
        &[
            "--checks-from",
            f,
            "--checks-sha256",
            &countercheck_core::sha256_hex(golden.as_bytes()),
        ],
    );
    assert_eq!(code, 1, "{err}");
    assert!(err.contains("can't name other files"), "{err}");
}

/// macOS: a daemon a check detached with `setsid` (double fork, new session, out of the process
/// group) is found by its working folder or its hold on the check's output, and stopped.
#[cfg(target_os = "macos")]
#[test]
fn a_setsid_daemon_is_stopped_when_its_check_ends_on_macos() {
    if nested() {
        return;
    }
    let outside = tempfile::tempdir().unwrap();
    let dir = outside.path().canonicalize().unwrap();
    let daemon = "import os\n\
        pid = os.fork()\n\
        if pid == 0:\n    \
            os.setsid()\n    \
            if os.fork() == 0:\n        \
                open(os.environ['PID_FILE'], 'w').write(str(os.getpid()))\n        \
                os.execvp('sleep', ['sleep', '300'])\n    \
            os._exit(0)\n\
        os.waitpid(pid, 0)\n";
    for sandbox in ["auto", "none"] {
        let pid_file = dir.join(format!("daemon-{sandbox}.pid"));
        let checks = format!(
            "[[check]]\nname = \"daemon\"\nrun = \"PID_FILE='{}' python3 tests/daemon.py; sleep 0.5\"\nwrites = [\"{}\"]\n",
            pid_file.display(),
            dir.display()
        );
        let d = project(&checks);
        fs::write(d.path().join("tests/daemon.py"), daemon).unwrap();
        sh(d.path(), "git add -A && git commit -qm daemon");
        let args: &[&str] = if sandbox == "none" {
            &["--sandbox", "none"]
        } else {
            &[]
        };
        let (code, r) = countercheck(d.path(), args);
        assert_eq!(code, 0, "{sandbox}: {r:#}");
        let pid = fs::read_to_string(&pid_file).unwrap().trim().to_string();
        let running = || {
            let out = Command::new("ps")
                .args(["-o", "stat=", "-p", &pid])
                .output()
                .unwrap();
            let stat = String::from_utf8_lossy(&out.stdout).trim().to_string();
            !stat.is_empty() && !stat.starts_with('Z')
        };
        let mut gone = false;
        for _ in 0..30 {
            if !running() {
                gone = true;
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
        if !gone {
            let _ = Command::new("kill").args(["-9", &pid]).status();
            panic!("{sandbox}: the setsid daemon {pid} survived its check");
        }
    }
}

// ---- the Windows full sandbox: the runner account (ADR-0004) ----
// CI runs these after `countercheck windows-setup` (the runner is admin), with
// COUNTERCHECK_TEST_RUNNER=1; elsewhere they skip.

#[cfg(windows)]
fn runner_set_up() -> bool {
    std::env::var("COUNTERCHECK_TEST_RUNNER").is_ok_and(|v| v == "1") && !nested()
}

#[cfg(windows)]
#[test]
fn windows_runner_sandboxes_sh_and_blocks_the_internet() {
    if !runner_set_up() {
        eprintln!(
            "skipped: COUNTERCHECK_TEST_RUNNER is not set (needs countercheck windows-setup)"
        );
        return;
    }
    // C:\ProgramData lets members of Users create folders: the account could write there if
    // only its rights applied. The restricted token is what refuses it.
    let tag = std::process::id();
    let sh_leak = format!("C:/ProgramData/cc-leak-sh-{tag}");
    let cmd_leak = format!(r"C:\ProgramData\cc-leak-cmd-{tag}");
    let checks = format!(
        r#"[[check]]
name = "sh inside"
run = "mkdir -p a/b && echo x > a/b/c && echo y > \"$TEMP/t\""
[[check]]
name = "sh outside"
run = "mkdir -p {sh_leak}"
[[check]]
name = "cmd outside"
shell = "cmd"
run = 'mkdir "{cmd_leak}"'
[[check]]
name = "who"
shell = "cmd"
run = "whoami"
[[check]]
name = "internet"
shell = "pwsh"
run = "$c = [Net.Sockets.TcpClient]::new(); $c.Connect('1.1.1.1', 443)"
timeout_secs = 60
[[check]]
name = "localhost"
shell = "pwsh"
run = "$l = [Net.Sockets.TcpListener]::new([Net.IPAddress]::Loopback, 0); $l.Start(); $c = [Net.Sockets.TcpClient]::new(); $c.Connect('127.0.0.1', $l.LocalEndpoint.Port); if (-not $c.Connected) {{ exit 1 }}"
[[check]]
name = "internet allowed"
shell = "pwsh"
run = "$c = [Net.Sockets.TcpClient]::new(); $c.Connect('1.1.1.1', 443)"
network = true
timeout_secs = 60
"#
    );
    let p = project(&checks);
    let (code, r) = countercheck(p.path(), &[]);
    assert_eq!(code, 2, "{r:#}");
    let sandbox = r["sandbox"].as_str().unwrap();
    assert!(
        sandbox.starts_with("windows-runner (no internet"),
        "{sandbox}"
    );
    let ok = |n: &str| by_name(&r, n)["passed"] == true;
    assert!(ok("sh inside"), "{r:#}");
    assert!(
        by_name(&r, "sh inside").get("sandboxed").is_none(),
        "sh is sandboxed: {r:#}"
    );
    assert!(!ok("sh outside"), "{r:#}");
    assert!(!Path::new(&sh_leak).exists(), "sh wrote outside");
    assert!(!ok("cmd outside"), "{r:#}");
    assert!(!Path::new(&cmd_leak).exists(), "cmd wrote outside");
    let who = by_name(&r, "who")["output_tail"]
        .as_str()
        .unwrap()
        .to_lowercase();
    assert!(who.contains("countercheck-runner"), "{who}");
    assert!(!ok("internet"), "the internet should be blocked: {r:#}");
    assert!(ok("localhost"), "localhost should work: {r:#}");
    assert!(
        ok("internet allowed"),
        "network = true runs as you, with internet: {r:#}"
    );
    assert_eq!(by_name(&r, "internet allowed")["network"], true);
}

#[cfg(windows)]
#[test]
fn windows_runner_stops_what_a_check_left_running() {
    if !runner_set_up() {
        eprintln!(
            "skipped: COUNTERCHECK_TEST_RUNNER is not set (needs countercheck windows-setup)"
        );
        return;
    }
    let p = project(
        "[[check]]\nname = \"bg\"\nshell = \"cmd\"\nrun = 'start \"\" /b ping -n 300 -w 1000 127.0.0.1 >NUL & exit /b 0'\n",
    );
    let (code, r) = countercheck(p.path(), &[]);
    assert_eq!(code, 0, "{r:#}");
    let left = || {
        let out = Command::new("powershell")
            .args([
                "-NoProfile",
                "-Command",
                "@(Get-Process -IncludeUserName -Name PING -ErrorAction SilentlyContinue | Where-Object { $_.UserName -like '*countercheck-runner' }).Count",
            ])
            .output()
            .unwrap();
        String::from_utf8_lossy(&out.stdout).trim().to_string()
    };
    for _ in 0..30 {
        if left() == "0" {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(200));
    }
    panic!("the runner account's ping survived its check ({})", left());
}
