//! Coverage checks: every item in a locked list must be accounted for in the worker's files.
//!
//! ```toml
//! [[coverage]]
//! name = "every procedure converted"
//! items = ".countercheck/inventory/procedures.txt"   # one item per line, under .countercheck/
//! paths = ["snowflake/**/*.sql"]
//! pattern = 'CREATE\s+OR\s+REPLACE\s+PROCEDURE\s+{item}\b'   # {item} = the escaped item
//! case_insensitive = true
//! ```

use crate::forbid::{self, MAX_FILE_BYTES};
use crate::protect::Protected;
use regex::Regex;
use serde::{Deserialize, Serialize};

/// The placeholder in `pattern`.
pub const ITEM: &str = "{item}";
/// At most this many items per list.
pub const MAX_ITEMS: usize = 10_000;
/// At most this many missing items are listed; the total is always given.
pub const MAX_MISSING_SHOWN: usize = 50;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CoverageSpec {
    pub name: String,
    /// The item list, under `.countercheck/` (read like the check file).
    pub items: String,
    pub paths: Vec<String>,
    pub pattern: String,
    #[serde(default)]
    pub case_insensitive: bool,
}

impl CoverageSpec {
    /// The name on the receipt.
    pub fn result_name(&self) -> String {
        format!("coverage: {}", self.name)
    }

    /// What the receipt shows in place of a command.
    pub fn describe(&self) -> String {
        format!(
            "coverage of {} by /{}/{} in {}",
            self.items,
            self.pattern,
            if self.case_insensitive { "i" } else { "" },
            self.paths.join(", ")
        )
    }

    fn what(&self) -> String {
        format!("coverage \"{}\"", self.name)
    }

    /// Check everything knowable without the item list. Errors are plain sentences.
    pub fn validate(&self) -> Result<(), String> {
        let what = self.what();
        if self.name.trim().is_empty() {
            return Err("a [[coverage]] entry has an empty name".to_string());
        }
        if !crate::config::plain_path(&self.items) || !self.items.starts_with(".countercheck/") {
            return Err(format!(
                "{what}: items \"{}\" must be a plain path under .countercheck/ (so the worker can't change it)",
                self.items
            ));
        }
        if !self.pattern.contains(ITEM) {
            return Err(format!("{what}: the pattern must contain {ITEM}"));
        }
        forbid::compile(
            &what,
            &self.pattern.replace(ITEM, "x"),
            self.case_insensitive,
        )?;
        forbid::path_globs(&what, &self.paths)?;
        Ok(())
    }
}

/// Read an item list: one item per line; blank lines and lines starting with `#` are skipped.
pub fn parse_items(path: &str, text: &str) -> Result<Vec<String>, String> {
    let mut out: Vec<String> = Vec::new();
    let mut seen = std::collections::BTreeSet::new();
    for line in text.lines() {
        let item = line.trim();
        if item.is_empty() || item.starts_with('#') {
            continue;
        }
        if seen.insert(item.to_string()) {
            out.push(item.to_string());
        }
        if out.len() > MAX_ITEMS {
            return Err(format!("{path} has more than {MAX_ITEMS} items"));
        }
    }
    if out.is_empty() {
        return Err(format!("{path} has no items, so nothing would be checked"));
    }
    Ok(out)
}

/// A compiled `[[coverage]]` entry with its items, tracking which have been found.
pub struct Coverage {
    globs: Protected,
    items: Vec<(String, Regex)>,
    found: Vec<bool>,
    pub files_scanned: usize,
    pub skipped: Vec<String>,
}

impl Coverage {
    pub fn new(spec: &CoverageSpec, items_text: &str) -> Result<Self, String> {
        spec.validate()?;
        let what = spec.what();
        let mut items = Vec::new();
        for item in parse_items(&spec.items, items_text)? {
            let pattern = spec.pattern.replace(ITEM, &regex::escape(&item));
            let re = forbid::compile(&what, &pattern, spec.case_insensitive)?;
            items.push((item, re));
        }
        let found = vec![false; items.len()];
        Ok(Self {
            globs: forbid::path_globs(&what, &spec.paths)?,
            items,
            found,
            files_scanned: 0,
            skipped: Vec::new(),
        })
    }

    /// True when `path` is one of the files to look in.
    pub fn wants(&self, path: &str) -> bool {
        self.globs.matches(path)
    }

    /// Look for the items not found yet in one file (the whole text, so patterns may span lines).
    pub fn add_file(&mut self, path: &str, bytes: &[u8]) {
        if forbid::is_binary(bytes) || bytes.len() as u64 > MAX_FILE_BYTES {
            self.skipped.push(path.to_string());
            return;
        }
        self.files_scanned += 1;
        let text = String::from_utf8_lossy(bytes);
        for ((_, re), found) in self.items.iter().zip(self.found.iter_mut()) {
            if !*found && re.is_match(&text) {
                *found = true;
            }
        }
    }

    /// A file that matches `paths` but was not read.
    pub fn skip(&mut self, path: &str) {
        self.skipped.push(path.to_string());
    }

    /// Pass or fail, and the output for the receipt.
    pub fn report(&self) -> (bool, String) {
        let missing: Vec<&str> = self
            .items
            .iter()
            .zip(&self.found)
            .filter(|(_, f)| !**f)
            .map(|((item, _), _)| item.as_str())
            .collect();
        let total = self.items.len();
        let mut out = if missing.is_empty() {
            format!(
                "all {total} item(s) found in {} file(s)\n",
                self.files_scanned
            )
        } else {
            let mut s = format!(
                "{} of {total} item(s) missing; {} file(s) scanned\n",
                missing.len(),
                self.files_scanned
            );
            for m in missing.iter().take(MAX_MISSING_SHOWN) {
                s.push_str(&format!("missing: {}\n", forbid::cut_line(m)));
            }
            if missing.len() > MAX_MISSING_SHOWN {
                s.push_str(&format!(
                    "... and {} more not shown\n",
                    missing.len() - MAX_MISSING_SHOWN
                ));
            }
            s
        };
        if !self.skipped.is_empty() {
            let more = self.skipped.len().saturating_sub(forbid::MAX_SKIPPED_SHOWN);
            out.push_str(&format!(
                "skipped {} file(s) (binary or over 5 MiB): {}{}\n",
                self.skipped.len(),
                self.skipped
                    .iter()
                    .take(forbid::MAX_SKIPPED_SHOWN)
                    .map(String::as_str)
                    .collect::<Vec<_>>()
                    .join(", "),
                if more > 0 {
                    format!(" and {more} more")
                } else {
                    String::new()
                }
            ));
        }
        (missing.is_empty(), out.trim_end().to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec() -> CoverageSpec {
        CoverageSpec {
            name: "procs".into(),
            items: ".countercheck/inventory/procs.txt".into(),
            paths: vec!["sql/**/*.sql".into()],
            pattern: r"CREATE\s+OR\s+REPLACE\s+PROCEDURE\s+{item}\b".into(),
            case_insensitive: true,
        }
    }

    #[test]
    fn windows_line_endings_in_the_list_and_the_files_are_found() {
        let mut s = spec();
        s.pattern = r"^CREATE\s+OR\s+REPLACE\s+PROCEDURE\s+{item}$".into();
        let mut cov = Coverage::new(&s, "# procs\r\np_one\r\np_two\r\n\r\n").unwrap();
        cov.add_file(
            "sql/a.sql",
            b"CREATE OR REPLACE PROCEDURE p_one\r\nAS\r\nCREATE OR REPLACE PROCEDURE p_two\r\n",
        );
        let (passed, out) = cov.report();
        assert!(passed, "{out}");
    }

    #[test]
    fn every_item_must_be_found() {
        let mut c = Coverage::new(&spec(), "# procedures\nload_a\nload_b\nload_c\n").unwrap();
        assert!(c.wants("sql/x/y.sql"));
        assert!(!c.wants("docs/y.sql"));
        c.add_file("sql/a.sql", b"create or replace\n  procedure load_a()\n");
        c.add_file("sql/b.sql", b"CREATE OR REPLACE PROCEDURE load_bb()\n");
        let (ok, out) = c.report();
        assert!(!ok);
        assert!(
            out.starts_with("2 of 3 item(s) missing; 2 file(s) scanned"),
            "{out}"
        );
        assert!(out.contains("missing: load_b\nmissing: load_c"), "{out}");
        c.add_file(
            "sql/c.sql",
            b"CREATE OR REPLACE PROCEDURE load_b() ; CREATE OR REPLACE PROCEDURE load_c()",
        );
        let (ok, out) = c.report();
        assert!(ok, "{out}");
        assert_eq!(out, "all 3 item(s) found in 3 file(s)");
    }

    #[test]
    fn items_are_escaped() {
        let mut s = spec();
        s.pattern = "^{item}$".into();
        let mut c = Coverage::new(&s, "a.b\n(x)\n").unwrap();
        // `a.b` must not match `aXb`; `(x)` is literal parentheses.
        c.add_file("sql/a.sql", b"aXb\n(x)\n");
        let (ok, out) = c.report();
        assert!(!ok);
        assert!(
            out.contains("missing: a.b") && !out.contains("missing: (x)"),
            "{out}"
        );
    }

    #[test]
    fn at_most_50_missing_are_listed() {
        let items: String = (0..70).map(|i| format!("p{i}\n")).collect();
        let c = Coverage::new(&spec(), &items).unwrap();
        let (ok, out) = c.report();
        assert!(!ok);
        assert_eq!(
            out.lines().filter(|l| l.starts_with("missing: ")).count(),
            50
        );
        assert!(out.contains("... and 20 more not shown"), "{out}");
        assert!(out.starts_with("70 of 70 item(s) missing"), "{out}");
    }

    #[test]
    fn bad_specs_and_lists_are_refused() {
        let mut s = spec();
        s.items = "inventory/procs.txt".into();
        assert!(s.validate().unwrap_err().contains("under .countercheck/"));
        s = spec();
        s.items = ".countercheck/../procs.txt".into();
        assert!(s.validate().is_err());
        s = spec();
        s.pattern = "PROCEDURE".into();
        assert!(s.validate().unwrap_err().contains("must contain {item}"));
        s.pattern = "({item}".into();
        assert!(s.validate().unwrap_err().contains("not a valid regex"));
        s = spec();
        s.paths = vec![];
        assert!(s.validate().unwrap_err().contains("paths is empty"));
        s = spec();
        s.name = "".into();
        assert!(s.validate().is_err());
        assert!(
            parse_items("f", "# only comments\n\n")
                .unwrap_err()
                .contains("no items")
        );
        let many: String = (0..=MAX_ITEMS).map(|i| format!("i{i}\n")).collect();
        assert!(parse_items("f", &many).unwrap_err().contains("more than"));
        assert_eq!(parse_items("f", "a\na\n b \n").unwrap(), vec!["a", "b"]);
    }
}
