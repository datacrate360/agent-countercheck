//! Extra write locations for one check under the macOS sandbox (issue #18).
//!
//! ```toml
//! [[check]]
//! name = "tests (official runner)"
//! run = "scripts/run_tests.sh"
//! writes = ["/var/tmp/hermes-pytest-{uid}"]   # this check may also write here
//! ```
//!
//! The sandbox lets a check write only in its clean copy and its temp dir. Some test runners
//! always write to a fixed path; without this the only way out is `--sandbox none`, which drops
//! every protection. `writes` is read from the locked check file like `network`, so a worker
//! can't grant itself a write location.
//!
//! What's allowed, checked here (pure) when the check file is read: an absolute path, no `.`/`..`
//! or empty segments, no glob or quote characters, only the `{uid}`, `{user}` and `{home}`
//! placeholders. What's refused once the real path is known (after symlinks are resolved, by the
//! binary): `/`, anything that contains or equals the home folder, and anything that contains,
//! equals or is inside the repository, since that would let a check change the worker's files or
//! the user's.

/// At most this many extra locations per check.
pub const MAX_PER_CHECK: usize = 8;
/// Longest template accepted.
pub const MAX_LEN: usize = 512;

const PLACEHOLDERS: [&str; 3] = ["{uid}", "{user}", "{home}"];

/// Check one `writes` entry as written in the check file.
pub fn validate_template(check: &str, t: &str) -> Result<(), String> {
    let bad = |why: &str| Err(format!("check \"{check}\": writes entry \"{t}\" {why}"));
    if t.len() > MAX_LEN {
        return bad("is too long");
    }
    if !(t.starts_with('/') || t.starts_with("{home}")) {
        return bad("must be an absolute path (or start with {home})");
    }
    if t.chars().any(|c| {
        c.is_control()
            || matches!(
                c,
                '"' | '\'' | '\\' | '*' | '?' | '[' | ']' | '#' | '$' | '`'
            )
    }) {
        return bad(
            "has a character that isn't allowed (quotes, backslash, glob or shell characters)",
        );
    }
    let mut rest = t.to_string();
    for p in PLACEHOLDERS {
        rest = rest.replace(p, "");
    }
    if rest.contains('{') || rest.contains('}') {
        return bad("has an unknown placeholder (allowed: {uid}, {user}, {home})");
    }
    let body = t.strip_prefix('/').unwrap_or(t);
    if body
        .split('/')
        .any(|seg| seg.is_empty() || seg == "." || seg == "..")
    {
        return bad("has an empty, . or .. path segment");
    }
    Ok(())
}

/// Check a check's whole `writes` list.
pub fn validate(check: &str, list: &[String]) -> Result<(), String> {
    if list.len() > MAX_PER_CHECK {
        return Err(format!(
            "check \"{check}\": writes lists more than {MAX_PER_CHECK} paths"
        ));
    }
    list.iter().try_for_each(|t| validate_template(check, t))
}

/// Fill in the placeholders. `user` and `uid` must be plain names/numbers (checked here), so a
/// strange environment can't smuggle a path in.
pub fn expand(t: &str, uid: &str, user: &str, home: &str) -> Result<String, String> {
    if t.contains("{uid}") && (uid.is_empty() || !uid.bytes().all(|b| b.is_ascii_digit())) {
        return Err(format!(
            "writes \"{t}\": the user id \"{uid}\" is not a number"
        ));
    }
    if t.contains("{user}")
        && (user.is_empty()
            || !user
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-' | b'.'))
            || user.starts_with('.'))
    {
        return Err(format!(
            "writes \"{t}\": the user name \"{user}\" can't be used in a path"
        ));
    }
    if t.contains("{home}") && (!home.starts_with('/') || home.contains("..")) {
        return Err(format!(
            "writes \"{t}\": the home folder \"{home}\" is not an absolute path"
        ));
    }
    Ok(t.replace("{uid}", uid)
        .replace("{user}", user)
        .replace("{home}", home.trim_end_matches('/')))
}

/// `a` equals `b` or is a folder above it (path-segment aware: `/var/tm` is not above `/var/tmp`).
fn contains(a: &str, b: &str) -> bool {
    let a = a.trim_end_matches('/');
    let b = b.trim_end_matches('/');
    a.is_empty() || a == b || b.strip_prefix(a).is_some_and(|r| r.starts_with('/'))
}

/// Refuse a resolved location (symlinks already followed) that could reach the user's or the
/// worker's files. All three paths are absolute and resolved.
pub fn check_resolved(check: &str, path: &str, home: &str, repo: &str) -> Result<(), String> {
    let bad = |why: &str| Err(format!("check \"{check}\": writes path {path} {why}"));
    if path.trim_end_matches('/').matches('/').count() < 2 {
        return bad("is too broad (it needs at least two folders, like /var/tmp/x)");
    }
    if contains(path, home) {
        return bad("contains your home folder");
    }
    if contains(path, repo) || contains(repo, path) {
        return bad(
            "is the repository, inside it or above it, so the check could change the worker's files",
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn templates_are_checked() {
        for ok in [
            "/var/tmp/hermes-pytest-{uid}",
            "/tmp/x-{user}",
            "{home}/.cache/runner",
            "/opt/build/out",
        ] {
            assert!(validate_template("c", ok).is_ok(), "{ok}");
        }
        for bad in [
            "var/tmp/x",
            "/var/../etc",
            "/var/./tmp",
            "/var//tmp",
            "/var/tmp/*",
            "/var/tmp/\"x",
            "/var/tmp/x\\y",
            "/var/tmp/$HOME",
            "/var/tmp/{pid}",
            "/var/tmp/x\n",
            "~/x",
        ] {
            assert!(validate_template("c", bad).is_err(), "{bad:?}");
        }
        assert!(validate_template("c", &format!("/{}", "a".repeat(MAX_LEN))).is_err());
        assert!(validate_template("c", &format!("/{}", "a".repeat(MAX_LEN - 1))).is_ok());
        assert!(
            validate_template("c", "/tmp/x}").is_err(),
            "a lone closing brace"
        );
        assert!(
            validate_template("c", "/tmp/{x").is_err(),
            "a lone opening brace"
        );
        let many: Vec<String> = (0..=MAX_PER_CHECK).map(|i| format!("/tmp/x{i}")).collect();
        assert!(validate("c", &many).is_err());
        assert!(validate("c", &many[..MAX_PER_CHECK]).is_ok());
    }

    #[test]
    fn placeholders_expand_and_are_checked() {
        assert_eq!(
            expand("/var/tmp/hermes-pytest-{uid}", "501", "sri", "/Users/sri").unwrap(),
            "/var/tmp/hermes-pytest-501"
        );
        assert_eq!(
            expand("{home}/.cache/{user}", "501", "sri", "/Users/sri/").unwrap(),
            "/Users/sri/.cache/sri"
        );
        assert!(expand("/tmp/{uid}", "5a1", "sri", "/Users/sri").is_err());
        assert!(expand("/tmp/{uid}", "", "sri", "/Users/sri").is_err());
        assert!(expand("/tmp/{user}", "501", "../x", "/Users/sri").is_err());
        assert!(expand("/tmp/{user}", "501", "a/b", "/Users/sri").is_err());
        assert!(expand("{home}/x", "501", "sri", "relative").is_err());
        // The user id is only needed when {uid} is used.
        assert_eq!(expand("/tmp/x", "", "sri", "/Users/sri").unwrap(), "/tmp/x");
        // An odd user name doesn't matter when {user} isn't used.
        assert!(expand("/tmp/{uid}", "501", "a/b", "/Users/sri").is_ok());
    }

    #[test]
    fn resolved_paths_never_reach_home_or_the_repo() {
        let (home, repo) = ("/Users/sri", "/Users/sri/code/proj");
        assert!(check_resolved("c", "/private/var/tmp/hermes-pytest-501", home, repo).is_ok());
        assert!(check_resolved("c", "/Users/sri/.cache/runner", home, repo).is_ok());
        assert!(
            check_resolved("c", "/opt/x", home, repo).is_ok(),
            "two folders is enough"
        );
        for bad in [
            "/",
            "/private",
            "/Users",
            "/Users/sri",
            "/Users/sri/code",
            "/Users/sri/code/proj",
            "/Users/sri/code/proj/tests",
        ] {
            assert!(check_resolved("c", bad, home, repo).is_err(), "{bad}");
        }
        // Segment-aware: a sibling with a common prefix is fine.
        assert!(check_resolved("c", "/Users/sri/code/proj-cache", home, repo).is_ok());
        assert!(!contains("/var/tm", "/var/tmp"));
        assert!(contains("/var/tmp", "/var/tmp/x"));
    }
}
