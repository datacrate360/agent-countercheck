//! Protected paths: files a worker must not change (its tests, the check file).

use globset::{Glob, GlobSet, GlobSetBuilder};
use serde::{Deserialize, Serialize};

/// Build and cache files that are never part of the worker's change (and never copied).
pub fn is_junk(path: &str) -> bool {
    path.split('/')
        .any(|seg| seg == "__pycache__" || seg == ".DS_Store")
        || path.ends_with(".pyc")
}

pub struct Protected {
    set: GlobSet,
}

impl Protected {
    pub fn new(patterns: &[String]) -> Result<Self, String> {
        Self::labeled(patterns, "protected pattern")
    }

    /// Like [`Protected::new`], with `label` naming the patterns in errors (e.g. "path pattern").
    pub fn labeled(patterns: &[String], label: &str) -> Result<Self, String> {
        let mut b = GlobSetBuilder::new();
        for p in patterns {
            let g = Glob::new(p).map_err(|e| format!("{label} \"{p}\": {e}"))?;
            b.add(g);
        }
        let set = b.build().map_err(|e| format!("{label}s: {e}"))?;
        Ok(Self { set })
    }

    /// Matches the path after resolving `.`, `..` and empty segments, so `./a/b` and `x/../a/b`
    /// match like `a/b`. Absolute paths and paths that escape the repository never match.
    pub fn matches(&self, path: &str) -> bool {
        if path.starts_with('/') {
            return false;
        }
        let mut parts: Vec<&str> = Vec::new();
        for seg in path.split('/') {
            match seg {
                "" | "." => {}
                ".." => {
                    if parts.pop().is_none() {
                        return false;
                    }
                }
                s => parts.push(s),
            }
        }
        self.set.is_match(parts.join("/"))
    }
}

/// How the worker changed a file, relative to the base revision.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Change {
    Added,
    Modified,
    Deleted,
}

/// What countercheck did about a change to a protected file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProtectedChange {
    pub path: String,
    pub change: Change,
    /// `restored_from_base` for modified/deleted files; `kept_new_file` for added ones, or
    /// `removed_new_file` when the path is `frozen`.
    pub action: String,
}

/// The action for a new file on a `frozen` path: taken out of the clean copy.
pub const REMOVED_NEW_FILE: &str = "removed_new_file";
/// The action for a symlink the worker added or changed that leads outside the project: taken
/// out of the clean copy, whatever its path (the checks must never read files Countercheck didn't).
pub const REMOVED_OUTSIDE_LINK: &str = "removed_outside_link";

impl ProtectedChange {
    /// Like [`ProtectedChange::decide`], for a `frozen` path: a new file is removed too, so the
    /// checks run on exactly the base's files there.
    pub fn decide_frozen(path: &str, change: Change) -> Self {
        let mut pc = Self::decide(path, change);
        if change == Change::Added {
            pc.action = REMOVED_NEW_FILE.to_string();
        }
        pc
    }

    /// True when the clean copy no longer holds the worker's version (restored or removed).
    pub fn undone(&self) -> bool {
        self.action == "restored_from_base"
            || self.action == REMOVED_NEW_FILE
            || self.action == REMOVED_OUTSIDE_LINK
    }

    pub fn decide(path: &str, change: Change) -> Self {
        let action = match change {
            Change::Added => "kept_new_file",
            Change::Modified | Change::Deleted => "restored_from_base",
        };
        Self {
            path: path.to_string(),
            change,
            action: action.to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn globs_match_nested_paths() {
        let p = Protected::new(&["tests/**".into(), ".countercheck/**".into()]).unwrap();
        assert!(p.matches("tests/test_a.py"));
        assert!(p.matches("tests/unit/deep/x.rs"));
        assert!(p.matches(".countercheck/checks.toml"));
        assert!(!p.matches("src/tests_helper.py"));
    }

    #[test]
    fn paths_are_resolved_before_matching() {
        let p = Protected::new(&["tests/**".into(), ".countercheck/**".into()]).unwrap();
        assert!(p.matches("./tests/a.py"));
        assert!(p.matches("tests//a.py"));
        assert!(p.matches("src/../.countercheck/checks.toml"));
        assert!(!p.matches("tests/../src/a.py"));
        let all = Protected::new(&["**".into()]).unwrap();
        assert!(all.matches("a"));
        assert!(!all.matches("../a"));
        assert!(!all.matches("a/../../a"));
        assert!(!all.matches("/etc/passwd"));
    }

    #[test]
    fn junk_is_recognised() {
        assert!(is_junk("tests/__pycache__/a.cpython-311.pyc"));
        assert!(is_junk("a/.DS_Store"));
        assert!(!is_junk("tests/test_a.py"));
    }

    #[test]
    fn frozen_paths_remove_new_files_and_restore_the_rest() {
        let f = |c| ProtectedChange::decide_frozen("t", c);
        assert_eq!(f(Change::Added).action, "removed_new_file");
        assert_eq!(f(Change::Modified).action, "restored_from_base");
        assert_eq!(f(Change::Deleted).action, "restored_from_base");
        assert!(f(Change::Added).undone() && f(Change::Modified).undone());
        assert!(!ProtectedChange::decide("t", Change::Added).undone());
    }

    #[test]
    fn added_files_are_kept_changed_files_restored() {
        assert_eq!(
            ProtectedChange::decide("t", Change::Added).action,
            "kept_new_file"
        );
        assert_eq!(
            ProtectedChange::decide("t", Change::Modified).action,
            "restored_from_base"
        );
        assert_eq!(
            ProtectedChange::decide("t", Change::Deleted).action,
            "restored_from_base"
        );
    }
}
