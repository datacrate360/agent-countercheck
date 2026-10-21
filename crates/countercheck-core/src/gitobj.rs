//! Git objects, checked by their own hash.
//!
//! Countercheck reads the base revision from the worker's own `.git`, which the worker can write
//! to: a replace ref, a rewritten object file or an index flag could make git hand back something
//! else than what the base commit holds. So every object read for the base is hashed again here
//! and refused unless its id matches, starting from the commit id the receipt names. The
//! worker's files are compared with the base by their bytes, never through git's index or
//! filters.

use std::collections::BTreeMap;

/// The repository's hash: SHA-1 (40 hex characters) or SHA-256 (64).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Sha1,
    Sha256,
}

impl Format {
    /// Length of an object id in bytes.
    pub fn id_len(self) -> usize {
        match self {
            Format::Sha1 => 20,
            Format::Sha256 => 32,
        }
    }
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// The id git gives an object of `kind` ("blob", "tree", "commit") with this content.
pub fn object_id(format: Format, kind: &str, body: &[u8]) -> String {
    let header = format!("{kind} {}\0", body.len());
    match format {
        Format::Sha1 => {
            use sha1::{Digest, Sha1};
            let mut h = Sha1::new();
            h.update(header.as_bytes());
            h.update(body);
            hex(&h.finalize())
        }
        Format::Sha256 => {
            use sha2::{Digest, Sha256};
            let mut h = Sha256::new();
            h.update(header.as_bytes());
            h.update(body);
            hex(&h.finalize())
        }
    }
}

/// True when `id` is a full object id of this format in lowercase hex.
pub fn is_full_id(format: Format, id: &str) -> bool {
    id.len() == format.id_len() * 2 && id.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

/// The tree id on a commit's first line (`tree <id>`).
pub fn commit_tree(format: Format, body: &[u8]) -> Result<String, String> {
    let first = body.split(|b| *b == b'\n').next().unwrap_or_default();
    let id = first
        .strip_prefix(b"tree ")
        .map(|id| String::from_utf8_lossy(id).into_owned())
        .ok_or("the base commit has no tree")?;
    if !is_full_id(format, &id) {
        return Err("the base commit has a malformed tree id".to_string());
    }
    Ok(id)
}

/// What a tree entry points at.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// A regular or executable file.
    File,
    /// A symlink; the blob holds the link target.
    Symlink,
    /// A folder (another tree).
    Tree,
    /// A submodule commit; its files are not in this repository.
    Submodule,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub kind: Kind,
    pub name: String,
    pub id: String,
}

/// Parse a tree object: entries are `<mode> <name>\0<raw id>`.
pub fn parse_tree(format: Format, body: &[u8]) -> Result<Vec<Entry>, String> {
    let bad = || "a tree in the base revision is malformed".to_string();
    let mut out = Vec::new();
    let mut rest = body;
    while !rest.is_empty() {
        let sp = rest.iter().position(|b| *b == b' ').ok_or_else(bad)?;
        let mode = rest.get(..sp).ok_or_else(bad)?;
        rest = rest.get(sp + 1..).ok_or_else(bad)?;
        let nul = rest.iter().position(|b| *b == 0).ok_or_else(bad)?;
        let name = String::from_utf8_lossy(rest.get(..nul).ok_or_else(bad)?).into_owned();
        rest = rest.get(nul + 1..).ok_or_else(bad)?;
        let id = rest.get(..format.id_len()).ok_or_else(bad)?;
        rest = rest.get(format.id_len()..).ok_or_else(bad)?;
        let kind = match mode {
            b"40000" | b"040000" => Kind::Tree,
            b"120000" => Kind::Symlink,
            b"160000" => Kind::Submodule,
            m if m.starts_with(b"100") => Kind::File,
            _ => return Err(bad()),
        };
        if name.is_empty() || name == "." || name == ".." || name.contains('/') {
            return Err(bad());
        }
        out.push(Entry {
            kind,
            name,
            id: hex(id),
        });
    }
    Ok(out)
}

/// One file of a tree, for comparing with the worker's copy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileId {
    pub symlink: bool,
    /// The blob id of the file's bytes (or of the link target).
    pub id: String,
}

/// Files by `/`-separated path.
pub type Files = BTreeMap<String, FileId>;

/// How a path differs between the base and the worker's copy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Diff {
    Added,
    Modified,
    Deleted,
    /// Only line endings differ (CRLF in the worker's file where the base has LF), as git's own
    /// text normalization writes files (e.g. `core.autocrlf` on Windows). See [`eol_equal`].
    LineEndings,
}

/// Every path whose content or kind (file / symlink) differs, sorted. Never [`Diff::LineEndings`]:
/// that needs the bytes, see [`eol_equal`].
pub fn diff(base: &Files, worker: &Files) -> Vec<(Diff, String)> {
    let mut out = Vec::new();
    for (path, b) in base {
        match worker.get(path) {
            None => out.push((Diff::Deleted, path.clone())),
            Some(w) if w == b => {}
            Some(_) => out.push((Diff::Modified, path.clone())),
        }
    }
    for path in worker.keys() {
        if !base.contains_key(path) {
            out.push((Diff::Added, path.clone()));
        }
    }
    out.sort_by(|a, b| a.1.cmp(&b.1));
    out
}

/// True when `worker` is `base` with some LF line endings written as CRLF.
pub fn eol_equal(base: &[u8], worker: &[u8]) -> bool {
    if worker.len() < base.len() {
        return false;
    }
    let mut w = worker.iter().peekable();
    for b in base {
        let Some(mut c) = w.next() else {
            return false;
        };
        if *c == b'\r' && *b == b'\n' && w.peek() == Some(&&b'\n') {
            c = match w.next() {
                Some(c) => c,
                None => return false,
            };
        }
        if c != b {
            return false;
        }
    }
    w.next().is_none()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_match_git() {
        // `printf 'hello\n' | git hash-object --stdin` and the empty tree
        assert_eq!(
            object_id(Format::Sha1, "blob", b"hello\n"),
            "ce013625030ba8dba906f756967f9e9ca394464a"
        );
        assert_eq!(
            object_id(Format::Sha1, "tree", b""),
            "4b825dc642cb6eb9a060e54bf8d69288fbee4904"
        );
        assert_eq!(
            object_id(Format::Sha256, "blob", b""),
            "473a0f4c3be8a93681a267e3b1e9a7dcda1185436fe141f7749120a303721813"
        );
    }

    #[test]
    fn trees_parse_and_bad_trees_are_refused() {
        let id = [0xabu8; 20];
        let mut body = b"100644 a.txt\0".to_vec();
        body.extend_from_slice(&id);
        body.extend_from_slice(b"40000 sub\0");
        body.extend_from_slice(&id);
        body.extend_from_slice(b"120000 link\0");
        body.extend_from_slice(&id);
        let entries = parse_tree(Format::Sha1, &body).unwrap();
        let kinds: Vec<_> = entries.iter().map(|e| (e.kind, e.name.as_str())).collect();
        assert_eq!(
            kinds,
            [
                (Kind::File, "a.txt"),
                (Kind::Tree, "sub"),
                (Kind::Symlink, "link")
            ]
        );
        assert_eq!(entries[0].id, "ab".repeat(20));
        assert!(parse_tree(Format::Sha1, &body[..body.len() - 1]).is_err());
        let mut dots = b"100644 ..\0".to_vec();
        dots.extend_from_slice(&id);
        assert!(parse_tree(Format::Sha1, &dots).is_err());
    }

    #[test]
    fn commit_tree_needs_a_full_id() {
        let t = "4b825dc642cb6eb9a060e54bf8d69288fbee4904";
        assert_eq!(
            commit_tree(Format::Sha1, format!("tree {t}\nauthor x\n").as_bytes()).unwrap(),
            t
        );
        assert!(commit_tree(Format::Sha1, b"tree abc\n").is_err());
        assert!(commit_tree(Format::Sha1, b"parent x\n").is_err());
    }

    #[test]
    fn diff_finds_every_kind_of_change() {
        let f = |id: &str, symlink| FileId {
            symlink,
            id: id.to_string(),
        };
        let base: Files = [
            ("a", f("1", false)),
            ("b", f("2", false)),
            ("l", f("3", true)),
        ]
        .into_iter()
        .map(|(p, v)| (p.to_string(), v))
        .collect();
        let worker: Files = [
            ("a", f("1", false)),
            ("l", f("3", false)),
            ("n", f("4", false)),
        ]
        .into_iter()
        .map(|(p, v)| (p.to_string(), v))
        .collect();
        assert_eq!(
            diff(&base, &worker),
            [
                (Diff::Deleted, "b".to_string()),
                (Diff::Modified, "l".to_string()),
                (Diff::Added, "n".to_string())
            ]
        );
    }

    #[test]
    fn only_line_endings_may_differ() {
        assert!(eol_equal(b"a\nb\n", b"a\r\nb\r\n"));
        assert!(eol_equal(b"a\nb\n", b"a\nb\r\n"));
        assert!(eol_equal(b"", b""));
        assert!(!eol_equal(b"a\nb\n", b"a\r\nc\r\n"));
        assert!(!eol_equal(b"a\n", b"a\r\n\n"));
        assert!(!eol_equal(b"a\r\n", b"a\n"));
        assert!(!eol_equal(b"a\n", b"a\r"));
        assert!(!eol_equal(b"a", b"a\r"));
    }
}
