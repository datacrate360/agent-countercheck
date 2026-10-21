//! Watched paths: files the worker may change, but every change is listed on the receipt (issue #19).
//!
//! ```toml
//! protected = [".github/workflows/**"]
//! watched = ["tests/**"]            # changes allowed, always reported
//! watched_mode = "unverified"       # optional: modifying or deleting one turns VERIFIED into UNVERIFIED
//! ```
//!
//! `protected` puts a file back; an unprotected file can change silently. Watched files sit in
//! between: honest work may need to update a test double, so the change stays, but a reviewer
//! reading the receipt sees it. Adding a new file is normal (new tests); modifying or deleting an
//! existing one is the signal.
//!
//! The patterns come from the check file at the base revision, like every other rule, so a worker
//! can't remove its own edits from the list. This module can only add a reason or turn VERIFIED
//! into UNVERIFIED; it never makes a verdict better.

use crate::protect::{self, Change, Protected};
use crate::receipt::Verdict;
use serde::{Deserialize, Serialize};

/// At most this many changes are stored on the receipt; `total` always counts them all.
pub const MAX_SHOWN: usize = 100;
/// At most this many are named in a reason line.
pub const MAX_IN_REASON: usize = 5;
/// Files larger than this get no line counts.
pub const MAX_COUNT_BYTES: usize = 5 * 1024 * 1024;

/// What a change to an existing watched file does to the verdict (`watched_mode = "..."`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    /// Listed on the receipt; the verdict doesn't change (default).
    #[default]
    Note,
    /// A VERIFIED that modified or deleted an existing watched file becomes UNVERIFIED.
    Unverified,
}

impl Mode {
    pub fn is_default(&self) -> bool {
        *self == Mode::Note
    }
}

/// One changed watched file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WatchedChange {
    pub path: String,
    pub change: Change,
    /// Lines added and removed compared with the base (absent for binary or very large files).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lines_added: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lines_removed: Option<u64>,
}

impl WatchedChange {
    /// `M tests/a.py (+3 -1)`.
    pub fn describe(&self) -> String {
        let letter = match self.change {
            Change::Added => "A",
            Change::Modified => "M",
            Change::Deleted => "D",
        };
        match (self.lines_added, self.lines_removed) {
            (Some(a), Some(r)) => format!("{letter} {} (+{a} -{r})", self.path),
            _ => format!("{letter} {}", self.path),
        }
    }

    /// An existing file changed or removed (not a new file).
    pub fn touches_existing(&self) -> bool {
        self.change != Change::Added
    }
}

/// The receipt's `watched` section.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Watched {
    pub mode: Mode,
    /// Every changed watched file.
    pub total: usize,
    /// How many of them were existing files modified or deleted.
    pub existing_changed: usize,
    /// The first [`MAX_SHOWN`] changes, sorted by path.
    pub changes: Vec<WatchedChange>,
}

/// Longest a line diff may take before it settles for a valid but less minimal one.
const DIFF_DEADLINE: std::time::Duration = std::time::Duration::from_secs(2);

/// Lines added and removed between two versions of a file, like `git diff --stat`: a Myers line
/// diff, so a moved line counts as one removed and one added. Line endings are ignored (a file
/// that only changed CRLF to LF is no change). `None` when either side is binary or too large.
pub fn line_delta(was: &[u8], now: &[u8]) -> Option<(u64, u64)> {
    let text = |b: &[u8]| -> Option<Vec<String>> {
        if b.len() > MAX_COUNT_BYTES || b.contains(&0) {
            return None;
        }
        Some(
            String::from_utf8_lossy(b)
                .lines()
                .map(|l| l.trim_end_matches('\r').to_string())
                .collect(),
        )
    };
    let (was, now) = (text(was)?, text(now)?);
    let ops = similar::capture_diff_slices_deadline(
        similar::Algorithm::Myers,
        &was,
        &now,
        Some(std::time::Instant::now() + DIFF_DEADLINE),
    );
    let (mut added, mut removed) = (0u64, 0u64);
    for op in ops {
        match op {
            similar::DiffOp::Equal { .. } => {}
            similar::DiffOp::Delete { old_len, .. } => removed += old_len as u64,
            similar::DiffOp::Insert { new_len, .. } => added += new_len as u64,
            similar::DiffOp::Replace {
                old_len, new_len, ..
            } => {
                removed += old_len as u64;
                added += new_len as u64;
            }
        }
    }
    Some((added, removed))
}

/// The changes that match a watched pattern. Protected files (they're restored and listed
/// separately), `.countercheck/**` and build junk are never listed. `counts` gives the line counts
/// for a path, if any.
pub fn find(
    changes: &[(Change, String)],
    watched: &Protected,
    protected: &Protected,
    mode: Mode,
    mut counts: impl FnMut(&str, Change) -> Option<(u64, u64)>,
) -> Watched {
    let mut list: Vec<WatchedChange> = changes
        .iter()
        .filter(|(_, p)| !protect::is_junk(p) && !protected.matches(p) && watched.matches(p))
        .map(|(c, p)| {
            let n = counts(p, *c);
            WatchedChange {
                path: p.clone(),
                change: *c,
                lines_added: n.map(|n| n.0),
                lines_removed: n.map(|n| n.1),
            }
        })
        .collect();
    list.sort_by(|a, b| a.path.cmp(&b.path));
    list.dedup_by(|a, b| a.path == b.path);
    let total = list.len();
    let existing_changed = list.iter().filter(|c| c.touches_existing()).count();
    list.truncate(MAX_SHOWN);
    Watched {
        mode,
        total,
        existing_changed,
        changes: list,
    }
}

/// Add the watched changes to the verdict. Only ever adds a reason, or (mode `unverified`, an
/// existing watched file modified or deleted) turns VERIFIED into UNVERIFIED.
pub fn apply(verdict: Verdict, mut reasons: Vec<String>, w: &Watched) -> (Verdict, Vec<String>) {
    if w.total == 0 {
        return (verdict, reasons);
    }
    let mut named = w
        .changes
        .iter()
        .take(MAX_IN_REASON)
        .map(WatchedChange::describe)
        .collect::<Vec<_>>()
        .join(", ");
    if w.total > MAX_IN_REASON {
        named.push_str(&format!(", and {} more", w.total - MAX_IN_REASON));
    }
    let n = w.total;
    if verdict == Verdict::Verified && w.mode == Mode::Unverified && w.existing_changed > 0 {
        reasons.push(format!(
            "{} existing watched file(s) were modified or deleted; review them before trusting this: {named}",
            w.existing_changed
        ));
        return (Verdict::Unverified, reasons);
    }
    reasons.push(format!(
        "{n} watched file(s) changed (review them): {named}"
    ));
    (verdict, reasons)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn globs(p: &[&str]) -> Protected {
        Protected::labeled(
            &p.iter().map(|s| s.to_string()).collect::<Vec<_>>(),
            "watched pattern",
        )
        .unwrap()
    }

    fn ch(paths: &[(Change, &str)]) -> Vec<(Change, String)> {
        paths.iter().map(|(c, p)| (*c, p.to_string())).collect()
    }

    #[test]
    fn lists_watched_changes_but_not_protected_or_unwatched_ones() {
        let w = find(
            &ch(&[
                (Change::Modified, "tests/test_a.py"),
                (Change::Added, "tests/test_new.py"),
                (Change::Deleted, "tests/test_old.py"),
                (Change::Modified, "tests/conftest.py"),
                (Change::Modified, "src/a.py"),
                (Change::Added, "tests/__pycache__/x.pyc"),
            ]),
            &globs(&["tests/**"]),
            &globs(&["**/conftest.py"]),
            Mode::Note,
            |p, _| (p == "tests/test_a.py").then_some((3, 1)),
        );
        let got: Vec<String> = w.changes.iter().map(WatchedChange::describe).collect();
        assert_eq!(
            got,
            vec![
                "M tests/test_a.py (+3 -1)",
                "A tests/test_new.py",
                "D tests/test_old.py"
            ]
        );
        assert_eq!((w.total, w.existing_changed), (3, 2));
    }

    #[test]
    fn only_existing_changes_make_it_unverified_and_never_better() {
        let added = Watched {
            mode: Mode::Unverified,
            total: 1,
            existing_changed: 0,
            changes: vec![WatchedChange {
                path: "tests/new.py".into(),
                change: Change::Added,
                lines_added: Some(5),
                lines_removed: Some(0),
            }],
        };
        let (v, r) = apply(Verdict::Verified, vec![], &added);
        assert_eq!(v, Verdict::Verified);
        assert!(r[0].contains("A tests/new.py (+5 -0)"), "{r:?}");

        let mut modified = added.clone();
        modified.existing_changed = 1;
        modified.changes[0].change = Change::Modified;
        let (v, r) = apply(Verdict::Verified, vec![], &modified);
        assert_eq!(v, Verdict::Unverified);
        assert!(r[0].contains("modified or deleted"), "{r:?}");
        // Never changes FAILED, and note mode never changes the verdict.
        assert_eq!(apply(Verdict::Failed, vec![], &modified).0, Verdict::Failed);
        modified.mode = Mode::Note;
        assert_eq!(
            apply(Verdict::Verified, vec![], &modified).0,
            Verdict::Verified
        );
        // Nothing changed: no reason at all.
        let none = Watched {
            mode: Mode::Unverified,
            total: 0,
            existing_changed: 0,
            changes: vec![],
        };
        assert_eq!(
            apply(Verdict::Verified, vec![], &none),
            (Verdict::Verified, vec![])
        );
    }

    #[test]
    fn line_delta_counts_like_git_diff_stat() {
        assert_eq!(line_delta(b"a\nb\nc\n", b"a\nb\nc\n"), Some((0, 0)));
        assert_eq!(
            line_delta(b"a\nb\n", b"b\na\n"),
            Some((1, 1)),
            "a moved line is a change, like git diff --stat"
        );
        assert_eq!(
            line_delta(b"a\nb\nc\nd\n", b"a\nX\nc\nY\nd\n"),
            Some((2, 1)),
            "b replaced by X, Y inserted"
        );
        assert_eq!(line_delta(b"a\nb\n", b"a\nX\nb\n"), Some((1, 0)));
        assert_eq!(line_delta(b"a\nb\n", b"a\n"), Some((0, 1)));
        assert_eq!(line_delta(b"x = 1\n", b"x = 2\n"), Some((1, 1)));
        assert_eq!(
            line_delta(b"a\r\nb\r\n", b"a\nb\n"),
            Some((0, 0)),
            "line endings"
        );
        assert_eq!(line_delta(b"", b"a\na\n"), Some((2, 0)), "duplicates count");
        assert_eq!(line_delta(b"a\0", b"b"), None, "binary");
    }

    #[test]
    fn limits_and_reason_lengths() {
        // Line counts up to 5 MiB, none above.
        assert_eq!(MAX_COUNT_BYTES, 5_242_880, "docs/checks.md says 5 MiB");
        let at = vec![b'a'; MAX_COUNT_BYTES];
        let over = vec![b'a'; MAX_COUNT_BYTES + 1];
        assert_eq!(line_delta(b"", &at), Some((1, 0)));
        assert_eq!(line_delta(b"", &over), None);
        assert_eq!(line_delta(&over, b""), None);
        // The reason names 5 changes, then "and N more".
        let mk = |n: usize| Watched {
            mode: Mode::Note,
            total: n,
            existing_changed: 0,
            changes: (0..n)
                .map(|i| WatchedChange {
                    path: format!("tests/t{i}.py"),
                    change: Change::Added,
                    lines_added: None,
                    lines_removed: None,
                })
                .collect(),
        };
        let (_, r) = apply(Verdict::Verified, vec![], &mk(5));
        assert!(!r[0].contains("more"), "{r:?}");
        let (_, r) = apply(Verdict::Verified, vec![], &mk(6));
        assert!(r[0].ends_with(", and 1 more"), "{r:?}");
        assert!(!r[0].contains("t5.py"), "{r:?}");
        // The default mode is left out when serialized; "unverified" is written.
        assert_eq!(
            serde_json::to_value(Mode::Unverified).unwrap(),
            "unverified"
        );
        assert!(Mode::Note.is_default() && !Mode::Unverified.is_default());
    }
}
