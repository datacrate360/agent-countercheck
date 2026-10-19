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

/// A timed-out check's background children die with it (the whole process group is killed).
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
