//! Checks from outside the repository (`--checks-from`, ADR-0002, issue #20).
//!
//! Two forms, both pinned by something given on the command line, never found on disk:
//! - `<suite repo>@<full commit id>:<path>`: read through the same hash-checked object reader as
//!   the base revision. Branch and tag names are refused (they move).
//! - `<file>` with `--checks-sha256 <hex>`: the file's SHA-256 must match. Without the hash the
//!   checks are not locked, so the verdict can't be VERIFIED.
//!
//! A file outside the repository is not outside the worker's reach (an agent has a shell), which
//! is why the pin, not the location, is what locks the checks.

use crate::{git, snapshot};
use countercheck_core::config::MAX_CHECKS_BYTES;
use countercheck_core::receipt::ExternalChecks;
use std::path::Path;

/// The external check file, read and checked.
pub struct Loaded {
    pub text: String,
    /// Pinned (git form, or file form with a matching `--checks-sha256`).
    pub locked: bool,
    /// For the receipt's `checks_source`.
    pub label: String,
    pub info: ExternalChecks,
    /// The suite revision (git form), for the files the check file names.
    pub suite: Option<git::Revision>,
}

/// `(repo, commit, path)` when `src` is `<repo>@<full commit id>:<path>`.
fn git_form(src: &str) -> Option<(&str, &str, &str)> {
    // Try every `@`: the repository path may contain one too (`/x/me@work/suites@<id>:<path>`).
    for (at, _) in src.match_indices('@') {
        let (Some(repo), Some(rest)) = (src.get(..at), src.get(at + 1..)) else {
            continue;
        };
        for len in [40usize, 64] {
            let (Some(id), Some(after)) = (rest.get(..len), rest.get(len..)) else {
                continue;
            };
            if id
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
                && let Some(path) = after.strip_prefix(':')
            {
                return Some((repo, id, path));
            }
        }
    }
    None
}

/// Looks like `<repo>@<something>:<path>` with a name instead of a full commit id.
fn looks_like_a_ref(src: &str) -> bool {
    !Path::new(src).exists()
        && src
            .rsplit_once('@')
            .is_some_and(|(repo, rest)| !repo.is_empty() && rest.contains(':'))
}

pub fn load(src: &str, pin: Option<&str>, target_root: &Path) -> Result<Loaded, String> {
    if let Some((repo, commit, path)) = git_form(src) {
        if pin.is_some() {
            return Err(
                "--checks-sha256 is for a check file; the commit id already pins the git form"
                    .to_string(),
            );
        }
        let rel = snapshot::safe_rel(path)?;
        let root = git::toplevel(Path::new(repo))
            .map_err(|e| format!("--checks-from: {repo} is not a git repository ({e})"))?;
        let mut suite = git::Revision::load(&root, commit)
            .map_err(|e| format!("--checks-from: commit {commit} in {repo}: {e}"))?;
        let rel = rel.to_string_lossy().replace('\\', "/");
        let bytes = suite
            .read(&rel)?
            .ok_or_else(|| format!("--checks-from: {rel} is not in commit {commit} of {repo}"))?;
        if bytes.len() > MAX_CHECKS_BYTES {
            return Err(format!(
                "--checks-from: {rel} is larger than {MAX_CHECKS_BYTES} bytes"
            ));
        }
        let sha256 = countercheck_core::sha256_hex(&bytes);
        let short = commit.get(..12).unwrap_or(commit);
        return Ok(Loaded {
            text: String::from_utf8_lossy(&bytes).into_owned(),
            locked: true,
            label: format!("external: {repo}@{short}:{rel}"),
            info: ExternalChecks {
                kind: "git".to_string(),
                source: format!("{repo}@{commit}:{rel}"),
                sha256,
                pinned: true,
            },
            suite: Some(suite),
        });
    }
    if looks_like_a_ref(src) {
        return Err(format!(
            "--checks-from {src}: use a full commit id (<repo>@<40 or 64 hex>:<path>); branch and tag names move, so they can't lock the checks"
        ));
    }
    let file = Path::new(src)
        .canonicalize()
        .map_err(|e| format!("--checks-from {src}: {e}"))?;
    let repo = target_root
        .canonicalize()
        .map_err(|e| format!("{}: {e}", target_root.display()))?;
    if file.starts_with(&repo) {
        return Err(format!(
            "--checks-from {src} is inside the repository being verified, where the worker can change it; keep the check file outside it"
        ));
    }
    let meta = std::fs::metadata(&file).map_err(|e| format!("--checks-from {src}: {e}"))?;
    if !meta.is_file() || meta.len() > MAX_CHECKS_BYTES as u64 {
        return Err(format!(
            "--checks-from {src} must be a file of at most {MAX_CHECKS_BYTES} bytes"
        ));
    }
    let bytes = std::fs::read(&file).map_err(|e| format!("--checks-from {src}: {e}"))?;
    let sha256 = countercheck_core::sha256_hex(&bytes);
    let locked = match pin {
        None => false,
        Some(want) => {
            let want = want.trim().to_ascii_lowercase();
            if want.len() != 64 || !want.bytes().all(|b| b.is_ascii_hexdigit()) {
                return Err("--checks-sha256 must be 64 hex characters".to_string());
            }
            if want != sha256 {
                return Err(format!(
                    "--checks-from {src}: its SHA-256 is {sha256}, not the pinned {want}; the file changed, so countercheck won't use it"
                ));
            }
            true
        }
    };
    let shown = file.display().to_string();
    Ok(Loaded {
        text: String::from_utf8_lossy(&bytes).into_owned(),
        locked,
        label: format!(
            "external: {shown} sha256:{} ({})",
            sha256.get(..12).unwrap_or(&sha256),
            if locked { "pinned" } else { "NOT pinned" }
        ),
        info: ExternalChecks {
            kind: "file".to_string(),
            source: shown,
            sha256,
            pinned: locked,
        },
        suite: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_git_form_needs_a_full_commit_id() {
        let id = "3f9c2e1d3f9c2e1d3f9c2e1d3f9c2e1d3f9c2e1d";
        assert_eq!(
            git_form(&format!("../suites@{id}:hermes/checks.toml")),
            Some(("../suites", id, "hermes/checks.toml"))
        );
        // A repo path with an @ in it still works.
        assert_eq!(
            git_form(&format!("/x/me@work/suites@{id}:c.toml")),
            Some(("/x/me@work/suites", id, "c.toml"))
        );
        let long = "a".repeat(64);
        assert!(git_form(&format!("s@{long}:c.toml")).is_some());
        for not in [
            "s@main:c.toml",
            "s@3f9c2e1:c.toml",
            "s@3F9C2E1D3F9C2E1D3F9C2E1D3F9C2E1D3F9C2E1D:c.toml",
            "/plain/file.toml",
        ] {
            assert!(git_form(not).is_none(), "{not}");
        }
        assert!(looks_like_a_ref("suites@main:c.toml"));
        assert!(looks_like_a_ref("suites@v1.2:c.toml"));
        assert!(!looks_like_a_ref("/plain/file.toml"));
    }

    fn git(dir: &Path, args: &[&str]) {
        let ok = std::process::Command::new("git")
            .args(args)
            .current_dir(dir)
            .status()
            .unwrap();
        assert!(ok.success(), "git {args:?}");
    }

    #[test]
    fn file_form_limits_and_pin_format() {
        let target = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let f = outside.path().join("c.toml");
        let src = f.to_str().unwrap();
        // Exactly the limit is fine (unpinned: not locked); one byte more is refused.
        std::fs::write(&f, vec![b'#'; MAX_CHECKS_BYTES]).unwrap();
        let l = load(src, None, target.path()).unwrap();
        assert!(!l.locked && !l.info.pinned);
        std::fs::write(&f, vec![b'#'; MAX_CHECKS_BYTES + 1]).unwrap();
        assert!(load(src, None, target.path()).is_err());
        // A folder is not a check file.
        assert!(load(outside.path().to_str().unwrap(), None, target.path()).is_err());
        // The pin must be 64 hex characters.
        std::fs::write(&f, "x").unwrap();
        for bad in ["abc", &"z".repeat(64)] {
            let e = load(src, Some(bad), target.path()).err().unwrap();
            assert!(e.contains("64 hex"), "{e}");
        }
        let good = countercheck_core::sha256_hex(b"x");
        assert!(
            load(src, Some(&good.to_uppercase()), target.path())
                .unwrap()
                .locked
        );
        assert!(!looks_like_a_ref("@main:c.toml"), "no repository part");
    }

    #[test]
    fn git_form_size_limit() {
        let suite = tempfile::tempdir().unwrap();
        let target = tempfile::tempdir().unwrap();
        git(suite.path(), &["init", "-q", "-b", "main"]);
        git(suite.path(), &["config", "user.email", "t@t"]);
        git(suite.path(), &["config", "user.name", "t"]);
        git(suite.path(), &["config", "core.autocrlf", "false"]);
        std::fs::write(suite.path().join("ok.toml"), vec![b'#'; MAX_CHECKS_BYTES]).unwrap();
        std::fs::write(
            suite.path().join("big.toml"),
            vec![b'#'; MAX_CHECKS_BYTES + 1],
        )
        .unwrap();
        git(suite.path(), &["add", "-A"]);
        git(suite.path(), &["commit", "-qm", "s"]);
        let out = std::process::Command::new("git")
            .args(["rev-parse", "HEAD"])
            .current_dir(suite.path())
            .output()
            .unwrap();
        let id = String::from_utf8(out.stdout).unwrap().trim().to_string();
        let src = |p: &str| format!("{}@{id}:{p}", suite.path().display());
        assert!(load(&src("ok.toml"), None, target.path()).unwrap().locked);
        let e = load(&src("big.toml"), None, target.path()).err().unwrap();
        assert!(e.contains("larger than"), "{e}");
        let e = load(&src("ok.toml"), Some(&"a".repeat(64)), target.path())
            .err()
            .unwrap();
        assert!(e.contains("already pins"), "{e}");
    }
}
