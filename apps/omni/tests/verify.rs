//! End-to-end: a real git repo, a "worker" that changes files, and the real `omni` binary.

use std::fs;
use std::path::Path;
use std::process::Command;

const OMNI: &str = env!("CARGO_BIN_EXE_omni");

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
    fs::create_dir_all(p.join(".omni")).unwrap();
    // Bug at base: subtracts instead of adding.
    fs::write(p.join("src/calc.sh"), "echo $(($1 - $2))\n").unwrap();
    fs::write(
        p.join("tests/check.sh"),
        "[ \"$(sh src/calc.sh 2 3)\" = 5 ]\n",
    )
    .unwrap();
    fs::write(p.join(".omni/checks.toml"), checks).unwrap();
    sh(p, "git add -A && git commit -qm base");
    d
}

const CHECKS: &str = "protected = [\"tests/**\"]\n[[check]]\nname = \"calc\"\nrun = \"sh tests/check.sh\"\ntimeout_secs = 30\n";

/// Inside an omni check (OMNI_VERIFY=1) we already run in a sandbox, and macOS can't nest them.
fn nested() -> bool {
    std::env::var_os("OMNI_VERIFY").is_some()
}

fn omni(dir: &Path, args: &[&str]) -> (i32, serde_json::Value) {
    let mut cmd = Command::new(OMNI);
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
    let (code, r) = omni(
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
    let (code, r) = omni(d.path(), &["--claim", "all tests pass"]);
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
        d.path().join(".omni/checks.toml"),
        "[[check]]\nname = \"calc\"\nrun = \"true\"\n",
    )
    .unwrap();
    let (code, r) = omni(d.path(), &[]);
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
    let (code, r) = omni(d.path(), &["--base", "main"]);
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
    fs::create_dir_all(p.join(".omni")).unwrap();
    fs::write(
        p.join(".omni/checks.toml"),
        "[[check]]\nname = \"ok\"\nrun = \"true\"\n",
    )
    .unwrap();
    let (code, r) = omni(p, &[]);
    assert_eq!(code, 3, "{r:#}");
    assert_eq!(r["verdict"], "unverified");
    assert_eq!(r["checks_locked"], false);
}

#[test]
fn timeouts_fail() {
    let d = project("[[check]]\nname = \"slow\"\nrun = \"sleep 30\"\ntimeout_secs = 1\n");
    let (code, r) = omni(d.path(), &[]);
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
    let (code, _) = omni(d.path(), &[]);
    assert_eq!(code, 2);
    assert!(!target.exists());
    let (code, _) = omni(d.path(), &["--sandbox", "none"]);
    assert_eq!(code, 0);
    assert!(target.exists());
}

#[test]
fn edited_receipts_are_detected() {
    let d = project(CHECKS);
    let (_, r) = omni(d.path(), &[]);
    let path = d
        .path()
        .join(".omni/receipts")
        .join(format!("{}.json", r["id"].as_str().unwrap()));
    let show = |p: &Path| {
        Command::new(OMNI)
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
fn ignored_dependency_folders_can_be_linked() {
    let base = "protected = [\"tests/**\"]\n[[check]]\nname = \"dep\"\nrun = \"sh deps/lib.sh\"\n";
    let d = project(&format!("link = [\"deps\"]\n{base}"));
    let p = d.path();
    sh(
        p,
        "echo deps/ > .gitignore && git add .gitignore && git commit -qm ignore && mkdir deps && echo 'exit 0' > deps/lib.sh",
    );
    let (code, r) = omni(p, &[]);
    assert_eq!(code, 0, "{r:#}");
    assert_eq!(r["linked"][0], "deps");

    let d2 = project(base);
    let p2 = d2.path();
    sh(
        p2,
        "echo deps/ > .gitignore && git add .gitignore && git commit -qm ignore && mkdir deps && echo 'exit 0' > deps/lib.sh",
    );
    let (code, _) = omni(p2, &[]);
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
    let (code, r) = omni(p, &[]);
    assert_eq!(code, 0, "{r:#}");
    assert_eq!(r["protected_changes"].as_array().unwrap().len(), 0);
}
