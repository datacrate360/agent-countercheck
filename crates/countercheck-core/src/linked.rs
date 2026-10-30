//! Folders named in `link` (a `.venv`, `node_modules`) come from the worker's tree, not the clean
//! copy. Two ways they can undo the clean copy, decided here (the scan itself lives in the app):
//!
//! 1. They point back into the project: an editable install writes the project's absolute path
//!    into the venv (`__editable__*.pth`, a finder module, `*.egg-link`), and a workspace package
//!    is a symlink to a project folder. A check then imports the worker's own files from the real
//!    project instead of the clean copy, so a protected file Countercheck put back (or a file
//!    Countercheck never copies, such as a planted bytecode cache) is not what runs.
//! 2. They changed while the session ran: an install, or an edited package (CC-22).

use crate::protect::ProtectedChange;
use crate::receipt::Verdict;
use serde::{Deserialize, Serialize};

/// Receipts list at most this many pointers (the count is in the reason).
pub const MAX_LISTED: usize = 20;

/// A file or symlink inside a linked folder that names a project path outside the linked folders.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Pointer {
    /// The file or symlink, relative to the project (`.venv/lib/.../x.pth`).
    pub file: String,
    /// The project path it names (`.` for the project root itself).
    pub target: String,
}

/// Characters that end a path in a text file (quotes, separators, brackets, whitespace).
fn ends_path(c: char) -> bool {
    c.is_whitespace() || "'\"`,;()[]{}<>|=".contains(c) || c == ':' || c == '\0'
}

/// `\\` (escaped in a string literal) and `\` become `/`, so Windows paths compare like Unix ones.
fn slashes(s: &str) -> String {
    s.replace("\\\\", "/").replace('\\', "/")
}

/// The project path (relative, `.` for the root) an absolute path names, if it is inside one of
/// the project's `roots` (its absolute path in every form a tool may have written).
pub fn project_rel(abs: &str, roots: &[String], fold_case: bool) -> Option<String> {
    let norm = |s: &str| {
        let s = slashes(s);
        if fold_case { s.to_ascii_lowercase() } else { s }
    };
    let abs = norm(abs);
    for root in roots {
        let root = norm(root);
        let root = root.trim_end_matches('/');
        if root.is_empty() {
            continue;
        }
        if abs == root {
            return Some(".".to_string());
        }
        if let Some(rest) = abs.strip_prefix(root).and_then(|r| r.strip_prefix('/')) {
            let rest = rest.trim_end_matches('/');
            return Some(if rest.is_empty() { "." } else { rest }.to_string());
        }
    }
    None
}

/// True when a project path is outside every linked folder (and so is the worker's own file).
/// A path with `..` in it is treated as outside: it can lead anywhere.
pub fn outside_linked(rel: &str, linked: &[String]) -> bool {
    if rel == "." || rel.split('/').any(|c| c == "..") {
        return true;
    }
    let rel = rel.trim_start_matches("./");
    !linked.iter().any(|l| {
        let l = l.trim_end_matches('/').trim_start_matches("./");
        rel == l || rel.strip_prefix(l).is_some_and(|r| r.starts_with('/'))
    })
}

/// Every project path outside the linked folders that `text` names, in any of the project's
/// `roots` forms (also as a `file://` URL). Sorted, without repeats.
pub fn pointers_in(
    text: &str,
    roots: &[String],
    linked: &[String],
    fold_case: bool,
) -> Vec<String> {
    let hay = if fold_case {
        slashes(text).to_ascii_lowercase()
    } else {
        slashes(text)
    };
    let mut found = Vec::new();
    for root in roots {
        let mut r = slashes(root);
        if fold_case {
            r = r.to_ascii_lowercase();
        }
        let r = r.trim_end_matches('/');
        if r.len() < 2 {
            continue;
        }
        for (i, _) in hay.match_indices(r) {
            // The root must start a path: not `/x/repo` for the root `/repo`.
            let before = &hay[..i];
            let starts_path = before.is_empty()
                || before.ends_with("file://")
                || before.chars().next_back().is_some_and(ends_path);
            if !starts_path {
                continue;
            }
            let after = &hay[i + r.len()..];
            let rel = match after.chars().next() {
                None => ".".to_string(),
                Some(c) if ends_path(c) => ".".to_string(),
                Some('/') => {
                    let rest: String = after[1..].chars().take_while(|c| !ends_path(*c)).collect();
                    let rest = rest.trim_end_matches('/');
                    if rest.is_empty() {
                        ".".to_string()
                    } else {
                        rest.to_string()
                    }
                }
                Some(_) => continue, // `/repo-old`: another folder
            };
            if outside_linked(&rel, linked) {
                found.push(rel);
            }
        }
    }
    found.sort();
    found.dedup();
    found
}

/// Add what was found in the linked folders to the verdict. Never makes a verdict better.
///
/// - A pointer back into the project, when the worker changed a protected or frozen file or
///   added a link out: FAILED. The checks may have used exactly the files Countercheck took out.
/// - A pointer back otherwise: VERIFIED becomes UNVERIFIED. The checks may have read the worker's
///   real folder (files Countercheck never copies included), so the clean copy proves nothing.
/// - A linked folder that changed during the session: VERIFIED becomes UNVERIFIED.
pub fn apply(
    verdict: Verdict,
    mut reasons: Vec<String>,
    pointers: &[Pointer],
    pointers_total: usize,
    protected: &[ProtectedChange],
    changed: &[String],
) -> (Verdict, Vec<String>) {
    let mut verdict = verdict;
    if let Some(first) = pointers.first() {
        let more = match pointers_total.max(pointers.len()) - 1 {
            0 => String::new(),
            n => format!(" and {n} more"),
        };
        let folder = first.file.split('/').next().unwrap_or(&first.file);
        let target = if first.target == "." {
            "the project folder".to_string()
        } else {
            first.target.clone()
        };
        // New files on protected paths are allowed (new tests); anything Countercheck put back or
        // took out is what the pointer could bring back.
        let undone = protected.iter().any(|p| p.action != "kept_new_file");
        if !undone {
            reasons.push(format!(
                "the linked folder {folder} points back into the project ({} -> {target}{more}), so the checks may have \
                 read the worker's real folder instead of the clean copy; use a non-editable install there, or have the \
                 check load the project's code from the clean copy first",
                first.file
            ));
            if verdict == Verdict::Verified {
                verdict = Verdict::Unverified;
            }
        } else {
            reasons.push(format!(
                "the worker changed protected or frozen files, and the linked folder {folder} points back into the \
                 project ({} -> {target}{more}), so the checks may have used the worker's changed files instead of \
                 the base versions",
                first.file
            ));
            verdict = Verdict::Failed;
        }
    }
    for name in changed {
        reasons.push(format!(
            "the linked folder {name} changed during this session (a package was installed or edited), so the checks \
             may have run with tools the agent changed; run again from a fresh session to verify"
        ));
        if verdict == Verdict::Verified {
            verdict = Verdict::Unverified;
        }
    }
    (verdict, reasons)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protect::Change;

    fn roots() -> Vec<String> {
        vec!["/work/proj".to_string()]
    }
    fn linked() -> Vec<String> {
        vec![".venv".to_string(), "web/node_modules".to_string()]
    }

    #[test]
    fn an_editable_install_points_back_into_the_project() {
        // setuptools' .pth for a src layout, and its finder module's mapping.
        assert_eq!(
            pointers_in("/work/proj/src\n", &roots(), &linked(), false),
            ["src"]
        );
        assert_eq!(
            pointers_in(
                "MAPPING = {'calc': '/work/proj/src/calc'}",
                &roots(),
                &linked(),
                false
            ),
            ["src/calc"]
        );
        // A flat layout points at the project folder itself; pip records the URL.
        assert_eq!(
            pointers_in("/work/proj\n", &roots(), &linked(), false),
            ["."]
        );
        assert_eq!(
            pointers_in(
                r#"{"url": "file:///work/proj"}"#,
                &roots(),
                &linked(),
                false
            ),
            ["."]
        );
    }

    #[test]
    fn paths_inside_linked_folders_and_other_folders_are_fine() {
        // A venv names itself (shebangs, activate scripts).
        assert!(
            pointers_in(
                "#!/work/proj/.venv/bin/python\n",
                &roots(),
                &linked(),
                false
            )
            .is_empty()
        );
        assert!(
            pointers_in("VIRTUAL_ENV='/work/proj/.venv'", &roots(), &linked(), false).is_empty()
        );
        assert!(
            pointers_in("/work/proj/web/node_modules/x", &roots(), &linked(), false).is_empty()
        );
        // Other folders that merely start with the same text.
        assert!(pointers_in("/work/proj-old/src", &roots(), &linked(), false).is_empty());
        assert!(pointers_in("/x/work/proj/src", &roots(), &linked(), false).is_empty());
        assert!(pointers_in("home = /usr/bin", &roots(), &linked(), false).is_empty());
    }

    #[test]
    fn a_way_out_of_a_linked_folder_counts() {
        assert_eq!(
            pointers_in("/work/proj/.venv/../src", &roots(), &linked(), false),
            [".venv/../src"]
        );
        assert!(outside_linked(".venvx/a", &linked()));
        assert!(!outside_linked(".venv", &linked()));
    }

    #[test]
    fn windows_paths_compare_without_case_and_with_either_slash() {
        let roots = vec![r"C:\Work\Proj".to_string()];
        assert_eq!(
            pointers_in(r"c:\\work\\proj\\src\\calc", &roots, &linked(), true),
            ["src/calc"]
        );
        assert_eq!(
            pointers_in("C:/Work/Proj/src", &roots, &linked(), true),
            ["src"]
        );
        assert_eq!(
            project_rel(r"C:\WORK\proj\pkg", &roots, true).as_deref(),
            Some("pkg")
        );
        assert_eq!(
            project_rel("/work/proj", &super::tests::roots(), false).as_deref(),
            Some(".")
        );
        assert_eq!(
            project_rel("/elsewhere/x", &super::tests::roots(), false),
            None
        );
    }

    fn ptr() -> Vec<Pointer> {
        vec![Pointer {
            file: ".venv/lib/python3.12/site-packages/__editable__.calc-0.0.1.pth".to_string(),
            target: "src".to_string(),
        }]
    }
    fn restored() -> Vec<ProtectedChange> {
        vec![ProtectedChange::decide(
            "src/calc/limits.py",
            Change::Modified,
        )]
    }

    #[test]
    fn a_pointer_back_never_leaves_a_verified() {
        let (v, r) = apply(Verdict::Verified, vec![], &ptr(), 1, &[], &[]);
        assert_eq!(v, Verdict::Unverified);
        assert!(r[0].contains("points back into the project"), "{r:?}");
        assert!(r[0].contains("-> src"), "{r:?}");
        // With a protected change it is FAILED, so the hook sends the agent back.
        let (v, r) = apply(Verdict::Verified, vec![], &ptr(), 3, &restored(), &[]);
        assert_eq!(v, Verdict::Failed);
        assert!(r[0].contains("and 2 more"), "{r:?}");
        // Never better than it was.
        assert_eq!(
            apply(Verdict::Failed, vec![], &ptr(), 1, &[], &[]).0,
            Verdict::Failed
        );
        assert_eq!(
            apply(Verdict::Verified, vec![], &[], 0, &restored(), &[]).0,
            Verdict::Verified
        );
        // A new test file (kept) is not something the pointer could undo.
        let new_test = vec![ProtectedChange::decide("tests/test_new.py", Change::Added)];
        assert_eq!(
            apply(Verdict::Verified, vec![], &ptr(), 1, &new_test, &[]).0,
            Verdict::Unverified
        );
    }

    #[test]
    fn a_linked_folder_changed_during_the_session_is_unverified() {
        let (v, r) = apply(
            Verdict::Verified,
            vec![],
            &[],
            0,
            &[],
            &[".venv".to_string()],
        );
        assert_eq!(v, Verdict::Unverified);
        assert!(r[0].contains(".venv changed during this session"), "{r:?}");
        assert_eq!(
            apply(Verdict::Failed, vec![], &[], 0, &[], &[".venv".to_string()]).0,
            Verdict::Failed
        );
    }
}
