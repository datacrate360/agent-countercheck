//! Changed files no check covers (issue #6).
//!
//! A check can say which files it looks at with `covers = ["src/**"]`; `[[forbid]]` and
//! `[[coverage]]` cover their `paths`. When at least one `[[check]]` declares `covers`, the receipt
//! lists every file the worker changed that none of them covers, so a VERIFIED can't quietly say
//! more than it checked. Off when no check declares `covers`.
//!
//! The patterns come from the check file at the base revision, like every other rule, so a worker
//! can't widen them to hide its change. This module can only add a reason or turn VERIFIED into
//! UNVERIFIED (with `uncovered = "unverified"`); it never makes a verdict better.

use crate::protect::{self, Change, Protected};
use crate::receipt::Verdict;
use serde::{Deserialize, Serialize};

/// At most this many uncovered paths are stored on the receipt; `total` always counts them all.
pub const MAX_PATHS_SHOWN: usize = 100;
/// At most this many are named in a reason line.
pub const MAX_IN_REASON: usize = 5;

/// What an uncovered change does to the verdict (top-level `uncovered = "..."` in checks.toml).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    /// Listed on the receipt; the verdict doesn't change (default).
    #[default]
    Note,
    /// A VERIFIED with uncovered changes becomes UNVERIFIED.
    Unverified,
}

impl Mode {
    pub fn is_default(&self) -> bool {
        *self == Mode::Note
    }
}

/// The receipt's `uncovered` section.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Uncovered {
    pub mode: Mode,
    /// Every uncovered changed file.
    pub total: usize,
    /// The first [`MAX_PATHS_SHOWN`] of them, sorted.
    pub paths: Vec<String>,
}

/// Changed files that no pattern in `covers` matches. Protected files (they're restored or listed
/// separately), `.countercheck/**` and build junk are never uncovered. Deleted files count:
/// deleting a file is a change like any other.
pub fn find(
    changes: &[(Change, String)],
    covers: &Protected,
    protected: &Protected,
    mode: Mode,
) -> Uncovered {
    let mut paths: Vec<String> = changes
        .iter()
        .map(|(_, p)| p)
        .filter(|p| !protect::is_junk(p) && !protected.matches(p) && !covers.matches(p))
        .cloned()
        .collect();
    paths.sort();
    paths.dedup();
    let total = paths.len();
    paths.truncate(MAX_PATHS_SHOWN);
    Uncovered { mode, total, paths }
}

/// Add the uncovered files to the verdict. Only ever adds a reason, or (mode `unverified`) turns
/// VERIFIED into UNVERIFIED. FAILED and UNVERIFIED are returned unchanged apart from the reason.
pub fn apply(verdict: Verdict, mut reasons: Vec<String>, u: &Uncovered) -> (Verdict, Vec<String>) {
    if u.total == 0 {
        return (verdict, reasons);
    }
    let mut named = u
        .paths
        .iter()
        .take(MAX_IN_REASON)
        .cloned()
        .collect::<Vec<_>>()
        .join(", ");
    if u.total > MAX_IN_REASON {
        named.push_str(&format!(", and {} more", u.total - MAX_IN_REASON));
    }
    let n = u.total;
    if verdict == Verdict::Verified && u.mode == Mode::Unverified {
        reasons.push(format!(
            "{n} changed file(s) no check covers, so countercheck can't vouch for them: {named}"
        ));
        return (Verdict::Unverified, reasons);
    }
    reasons.push(format!(
        "{n} changed file(s) no check covers (listed, not checked): {named}"
    ));
    (verdict, reasons)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn globs(p: &[&str]) -> Protected {
        Protected::labeled(
            &p.iter().map(|s| s.to_string()).collect::<Vec<_>>(),
            "covers pattern",
        )
        .unwrap()
    }

    fn ch(paths: &[(Change, &str)]) -> Vec<(Change, String)> {
        paths.iter().map(|(c, p)| (*c, p.to_string())).collect()
    }

    #[test]
    fn lists_only_changes_no_pattern_covers() {
        let u = find(
            &ch(&[
                (Change::Modified, "src/pricing.py"),
                (Change::Modified, "src/billing.py"),
                (Change::Added, "docs/notes.md"),
            ]),
            &globs(&["src/pricing.py"]),
            &globs(&["tests/**", ".countercheck/**"]),
            Mode::Note,
        );
        assert_eq!(u.paths, vec!["docs/notes.md", "src/billing.py"]);
        assert_eq!(u.total, 2);
    }

    #[test]
    fn protected_countercheck_and_junk_are_never_uncovered() {
        let u = find(
            &ch(&[
                (Change::Modified, "tests/test_a.py"),
                (Change::Modified, ".countercheck/checks.toml"),
                (Change::Added, "src/__pycache__/a.pyc"),
                (Change::Added, "a/.DS_Store"),
            ]),
            &globs(&["nothing/**"]),
            &globs(&["tests/**", ".countercheck/**"]),
            Mode::Note,
        );
        assert_eq!(u.total, 0);
    }

    #[test]
    fn deleted_files_count() {
        let u = find(
            &ch(&[(Change::Deleted, "src/old.py")]),
            &globs(&["lib/**"]),
            &globs(&[".countercheck/**"]),
            Mode::Note,
        );
        assert_eq!(u.paths, vec!["src/old.py"]);
    }

    #[test]
    fn escaping_paths_never_count_as_covered() {
        // `src/../secret.py` is `secret.py`; `../x` and absolute paths match nothing.
        let u = find(
            &ch(&[
                (Change::Modified, "src/../secret.py"),
                (Change::Modified, "../outside.py"),
                (Change::Modified, "/etc/passwd"),
            ]),
            &globs(&["src/**"]),
            &globs(&[".countercheck/**"]),
            Mode::Note,
        );
        assert_eq!(u.total, 3);
    }

    #[test]
    fn the_list_is_capped_but_the_total_is_not() {
        let many: Vec<(Change, String)> = (0..250)
            .map(|i| (Change::Added, format!("gen/f{i:03}.txt")))
            .collect();
        let u = find(
            &many,
            &globs(&["src/**"]),
            &globs(&[".countercheck/**"]),
            Mode::Note,
        );
        assert_eq!(u.total, 250);
        assert_eq!(u.paths.len(), MAX_PATHS_SHOWN);
    }

    fn u(mode: Mode, n: usize) -> Uncovered {
        let paths: Vec<String> = (0..n).map(|i| format!("f{i}")).collect();
        Uncovered {
            mode,
            total: n,
            paths,
        }
    }

    #[test]
    fn note_mode_only_adds_a_reason() {
        let (v, r) = apply(Verdict::Verified, vec![], &u(Mode::Note, 2));
        assert_eq!(v, Verdict::Verified);
        assert!(r[0].contains("2 changed file(s) no check covers"), "{r:?}");
    }

    #[test]
    fn unverified_mode_downgrades_only_a_verified() {
        assert_eq!(
            apply(Verdict::Verified, vec![], &u(Mode::Unverified, 1)).0,
            Verdict::Unverified
        );
        assert_eq!(
            apply(Verdict::Failed, vec![], &u(Mode::Unverified, 1)).0,
            Verdict::Failed
        );
        assert_eq!(
            apply(Verdict::Unverified, vec![], &u(Mode::Unverified, 1)).0,
            Verdict::Unverified
        );
    }

    #[test]
    fn nothing_uncovered_changes_nothing() {
        for v in [Verdict::Verified, Verdict::Failed, Verdict::Unverified] {
            for m in [Mode::Note, Mode::Unverified] {
                let (out, r) = apply(v, vec!["x".into()], &u(m, 0));
                assert_eq!(out, v);
                assert_eq!(r, vec!["x".to_string()]);
            }
        }
    }

    #[test]
    fn long_lists_are_shortened_in_the_reason() {
        let (_, r) = apply(Verdict::Verified, vec![], &u(Mode::Note, 9));
        assert!(r[0].ends_with("f0, f1, f2, f3, f4, and 4 more"), "{r:?}");
    }
}
