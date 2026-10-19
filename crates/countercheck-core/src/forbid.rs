//! Forbid checks: patterns that must not appear in the worker's files.
//!
//! ```toml
//! [[forbid]]
//! name = "no debug prints"
//! pattern = '\bconsole\.log\('     # a regex, matched line by line
//! paths = ["src/**/*.js"]          # globs, same matcher as `protected`
//! message = "Use the logger."      # optional, shown to the agent
//! case_insensitive = false         # optional
//! ```
//!
//! The `regex` crate matches in linear time, so a pattern from a repository we don't control
//! can't hang the scan; it is also compiled with a size limit.

use crate::protect::Protected;
use regex::{Regex, RegexBuilder};
use serde::{Deserialize, Serialize};

/// Files larger than this are skipped (and listed as skipped).
pub const MAX_FILE_BYTES: u64 = 5 * 1024 * 1024;
/// At most this many hits are shown; the total is always given.
pub const MAX_HITS_SHOWN: usize = 20;
/// Lines in the output are cut to this many characters.
pub const MAX_LINE_CHARS: usize = 200;
/// At most this many skipped files are named.
pub const MAX_SKIPPED_SHOWN: usize = 20;
/// Compiled-pattern size limit (bytes), so a huge pattern is refused instead of eating memory.
pub const REGEX_SIZE_LIMIT: usize = 1 << 20;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ForbidSpec {
    pub name: String,
    pub pattern: String,
    pub paths: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(default)]
    pub case_insensitive: bool,
}

impl ForbidSpec {
    /// The name on the receipt.
    pub fn result_name(&self) -> String {
        format!("forbid: {}", self.name)
    }

    /// What the receipt shows in place of a command.
    pub fn describe(&self) -> String {
        format!(
            "forbid /{}/{} in {}",
            self.pattern,
            if self.case_insensitive { "i" } else { "" },
            self.paths.join(", ")
        )
    }
}

/// Compile a pattern from a check file with the size limit. Errors are plain sentences.
pub fn compile(what: &str, pattern: &str, case_insensitive: bool) -> Result<Regex, String> {
    RegexBuilder::new(pattern)
        .case_insensitive(case_insensitive)
        .size_limit(REGEX_SIZE_LIMIT)
        .dfa_size_limit(REGEX_SIZE_LIMIT)
        .build()
        .map_err(|e| match e {
            regex::Error::CompiledTooBig(_) => format!("{what}: the pattern is too big"),
            other => {
                let detail = other.to_string();
                let last = detail.lines().last().unwrap_or("").trim().to_string();
                format!("{what}: the pattern is not a valid regex ({last})")
            }
        })
}

/// Paths a rule looks at, built from globs with the same matcher as `protected`.
pub fn path_globs(what: &str, paths: &[String]) -> Result<Protected, String> {
    if paths.is_empty() {
        return Err(format!(
            "{what}: paths is empty, so nothing would be scanned"
        ));
    }
    Protected::labeled(paths, "path pattern").map_err(|e| format!("{what}: {e}"))
}

/// A compiled `[[forbid]]` entry.
pub struct Rule {
    re: Regex,
    globs: Protected,
}

impl Rule {
    pub fn new(spec: &ForbidSpec) -> Result<Self, String> {
        let what = format!("forbid \"{}\"", spec.name);
        if spec.name.trim().is_empty() {
            return Err("a [[forbid]] entry has an empty name".to_string());
        }
        if spec.pattern.is_empty() {
            return Err(format!("{what}: the pattern is empty"));
        }
        Ok(Self {
            re: compile(&what, &spec.pattern, spec.case_insensitive)?,
            globs: path_globs(&what, &spec.paths)?,
        })
    }

    /// True when `path` (relative to the repository) is one of the files to scan.
    pub fn wants(&self, path: &str) -> bool {
        self.globs.matches(path)
    }
}

/// A file looks binary when it has a NUL byte.
pub fn is_binary(bytes: &[u8]) -> bool {
    bytes.contains(&0)
}

/// Cut a line to `MAX_LINE_CHARS` characters, trimming the line ending.
pub fn cut_line(line: &str) -> String {
    let line = line.trim_end_matches(['\r', '\n']);
    if line.chars().count() > MAX_LINE_CHARS {
        let mut s: String = line.chars().take(MAX_LINE_CHARS).collect();
        s.push_str("...");
        s
    } else {
        line.to_string()
    }
}

/// The running result of scanning files for one rule.
#[derive(Debug, Default)]
pub struct Scan {
    /// The first hits, as `path:line: text`.
    pub shown: Vec<String>,
    /// Every matching line, shown or not.
    pub total: usize,
    pub files_scanned: usize,
    pub files_with_hits: usize,
    /// Files that match `paths` but were not read (binary or too big).
    pub skipped: Vec<String>,
}

impl Scan {
    /// Scan one file's bytes. Binary files are skipped.
    pub fn add_file(&mut self, rule: &Rule, path: &str, bytes: &[u8]) {
        if is_binary(bytes) || bytes.len() as u64 > MAX_FILE_BYTES {
            self.skipped.push(path.to_string());
            return;
        }
        self.files_scanned += 1;
        let text = String::from_utf8_lossy(bytes);
        let mut hit_here = false;
        for (i, line) in text.lines().enumerate() {
            if rule.re.is_match(line) {
                hit_here = true;
                self.total += 1;
                if self.shown.len() < MAX_HITS_SHOWN {
                    self.shown
                        .push(format!("{path}:{}: {}", i + 1, cut_line(line)));
                }
            }
        }
        if hit_here {
            self.files_with_hits += 1;
        }
    }

    /// A file that matches `paths` but was not read.
    pub fn skip(&mut self, path: &str) {
        self.skipped.push(path.to_string());
    }

    /// Pass or fail, and the output for the receipt.
    pub fn report(&self, spec: &ForbidSpec) -> (bool, String) {
        let mut out = String::new();
        let passed = self.total == 0;
        if self.files_scanned == 0 && self.skipped.is_empty() {
            out.push_str(&format!(
                "no files match {}; nothing to scan\n",
                spec.paths.join(", ")
            ));
        } else if passed {
            out.push_str(&format!(
                "no forbidden lines in {} file(s)\n",
                self.files_scanned
            ));
        } else {
            out.push_str(&format!(
                "{} forbidden line(s) in {} file(s); {} file(s) scanned\n",
                self.total, self.files_with_hits, self.files_scanned
            ));
            for h in &self.shown {
                out.push_str(h);
                out.push('\n');
            }
            if self.total > self.shown.len() {
                out.push_str(&format!(
                    "... and {} more not shown\n",
                    self.total - self.shown.len()
                ));
            }
        }
        if !self.skipped.is_empty() {
            let names: Vec<&str> = self
                .skipped
                .iter()
                .take(MAX_SKIPPED_SHOWN)
                .map(String::as_str)
                .collect();
            let more = self.skipped.len().saturating_sub(MAX_SKIPPED_SHOWN);
            out.push_str(&format!(
                "skipped {} file(s) (binary or over 5 MiB): {}{}\n",
                self.skipped.len(),
                names.join(", "),
                if more > 0 {
                    format!(" and {more} more")
                } else {
                    String::new()
                }
            ));
        }
        if !passed && let Some(m) = spec.message.as_deref().filter(|m| !m.trim().is_empty()) {
            out.push_str(m.trim());
            out.push('\n');
        }
        (passed, out.trim_end().to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec(pattern: &str) -> ForbidSpec {
        ForbidSpec {
            name: "oracle".into(),
            pattern: pattern.into(),
            paths: vec!["sql/**/*.sql".into()],
            message: Some("Convert these.".into()),
            case_insensitive: false,
        }
    }

    #[test]
    fn hits_are_reported_with_path_and_line() {
        let s = spec(r"\(\+\)|\bROWNUM\b");
        let rule = Rule::new(&s).unwrap();
        assert!(rule.wants("sql/a/b.sql"));
        assert!(!rule.wants("docs/b.sql"));
        let mut scan = Scan::default();
        scan.add_file(
            &rule,
            "sql/a.sql",
            b"select 1\nwhere a.x = b.x(+)\nand ROWNUM < 5\n",
        );
        scan.add_file(&rule, "sql/b.sql", b"select rownum_total from t\n");
        let (passed, out) = scan.report(&s);
        assert!(!passed);
        assert!(
            out.starts_with("2 forbidden line(s) in 1 file(s); 2 file(s) scanned"),
            "{out}"
        );
        assert!(out.contains("sql/a.sql:2: where a.x = b.x(+)"), "{out}");
        assert!(out.contains("sql/a.sql:3: and ROWNUM < 5"), "{out}");
        assert!(out.ends_with("Convert these."), "{out}");
    }

    #[test]
    fn case_insensitive_is_off_by_default() {
        let mut s = spec(r"\bdecode\s*\(");
        let text = b"SELECT DECODE(a, 1, 2) FROM t\n";
        let mut scan = Scan::default();
        scan.add_file(&Rule::new(&s).unwrap(), "sql/a.sql", text);
        assert!(scan.report(&s).0);
        s.case_insensitive = true;
        let mut scan = Scan::default();
        scan.add_file(&Rule::new(&s).unwrap(), "sql/a.sql", text);
        assert!(!scan.report(&s).0);
    }

    #[test]
    fn at_most_20_hits_are_shown_with_the_total() {
        let s = spec("x");
        let rule = Rule::new(&s).unwrap();
        let mut scan = Scan::default();
        scan.add_file(&rule, "sql/a.sql", "x\n".repeat(45).as_bytes());
        let (passed, out) = scan.report(&s);
        assert!(!passed);
        assert_eq!(
            out.lines().filter(|l| l.starts_with("sql/a.sql:")).count(),
            20
        );
        assert!(out.starts_with("45 forbidden line(s)"), "{out}");
        assert!(out.contains("... and 25 more not shown"), "{out}");
    }

    #[test]
    fn long_lines_are_cut() {
        let s = spec("x");
        let rule = Rule::new(&s).unwrap();
        let mut scan = Scan::default();
        scan.add_file(
            &rule,
            "sql/a.sql",
            format!("x{}\n", "y".repeat(500)).as_bytes(),
        );
        let line = scan.shown[0].clone();
        assert_eq!(line.len(), "sql/a.sql:1: ".len() + 200 + 3, "{line}");
        assert!(line.ends_with("..."));
    }

    #[test]
    fn binary_and_skipped_files_are_listed_not_scanned() {
        let s = spec("x");
        let rule = Rule::new(&s).unwrap();
        let mut scan = Scan::default();
        scan.add_file(&rule, "sql/a.bin", b"x\0x");
        scan.skip("sql/big.sql");
        let (passed, out) = scan.report(&s);
        assert!(passed);
        assert!(
            out.contains("skipped 2 file(s) (binary or over 5 MiB): sql/a.bin, sql/big.sql"),
            "{out}"
        );
        assert!(
            !out.contains("Convert"),
            "the message is only shown on failure: {out}"
        );
    }

    #[test]
    fn no_matching_files_passes_and_says_so() {
        let s = spec("x");
        let (passed, out) = Scan::default().report(&s);
        assert!(passed);
        assert_eq!(out, "no files match sql/**/*.sql; nothing to scan");
    }

    #[test]
    fn bad_rules_are_refused_plainly() {
        let e = Rule::new(&spec("(unclosed")).err().unwrap();
        assert!(
            e.contains("forbid \"oracle\": the pattern is not a valid regex"),
            "{e}"
        );
        assert!(!e.contains('\n'), "{e}");
        let e = Rule::new(&spec("")).err().unwrap();
        assert!(e.contains("empty"), "{e}");
        let mut s = spec("x");
        s.paths = vec![];
        assert!(Rule::new(&s).err().unwrap().contains("paths is empty"));
        s.paths = vec!["[".into()];
        assert!(Rule::new(&s).is_err());
        s = spec("x");
        s.name = " ".into();
        assert!(Rule::new(&s).err().unwrap().contains("empty name"));
        // A pattern that compiles to something enormous is refused, not built.
        let e = Rule::new(&spec(r"\w{1000}\w{1000}\w{1000}")).err().unwrap();
        assert!(e.contains("too big"), "{e}");
    }

    #[test]
    fn names_and_descriptions() {
        let mut s = spec("a|b");
        assert_eq!(s.result_name(), "forbid: oracle");
        assert_eq!(s.describe(), "forbid /a|b/ in sql/**/*.sql");
        s.case_insensitive = true;
        assert_eq!(s.describe(), "forbid /a|b/i in sql/**/*.sql");
    }
}
