//! The check file a project commits at `.countercheck/checks.toml`.
//!
//! ```toml
//! protected = ["tests/**"]      # paths a worker may not change; .countercheck/** is always protected
//!
//! [[check]]
//! name = "unit tests"
//! run = "pytest -q"             # run by `sh -c` in a clean copy of the project
//! timeout_secs = 600            # optional, default 600, max 7200
//! shell = "sh"                  # optional: "sh" (default), "pwsh" or "cmd"
//! ```
//!
//! A check can also run once per target (`matrix`, see [`crate::matrix`]); `[[forbid]]` entries
//! list patterns that must not appear (see [`crate::forbid`]); `golden` compares a check's output
//! file with a locked reference (see [`crate::golden`]); `budget` keeps a printed metric within
//! limits (see [`crate::budget`]). docs/checks.md has every
//! check type with an example.
//!
//! `link = ["node_modules", ".venv"]` makes git-ignored dependency folders visible in the clean
//! copy (read-only under the sandbox). The receipt lists them, because they come from the worker's tree.
//!
//! countercheck reads this file from the **base** revision (before the worker's change), so a worker
//! cannot weaken the checks it is judged by.

use crate::budget::BudgetSpec;
use crate::coverage::CoverageSpec;
use crate::forbid::{self, ForbidSpec};
use crate::golden::{self, GoldenSpec};
use crate::matrix;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Where the check file lives, relative to the repository root.
pub const CHECKS_PATH: &str = ".countercheck/checks.toml";
/// Always protected, whatever the file says.
pub const ALWAYS_PROTECTED: &str = ".countercheck/**";
pub const DEFAULT_TIMEOUT_SECS: u64 = 600;
pub const MAX_TIMEOUT_SECS: u64 = 7200;
/// Refuse absurdly large check files (they come from a repository we don't control).
pub const MAX_CHECKS_BYTES: usize = 64 * 1024;

#[derive(Debug, Clone, Default, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ChecksFile {
    #[serde(default)]
    pub protected: Vec<String>,
    #[serde(default, rename = "check")]
    pub checks: Vec<CheckSpec>,
    /// Patterns that must not appear in the worker's files.
    #[serde(default, rename = "forbid", skip_serializing_if = "Vec::is_empty")]
    pub forbids: Vec<ForbidSpec>,
    /// Item lists whose every item must be found in the worker's files.
    #[serde(default, rename = "coverage", skip_serializing_if = "Vec::is_empty")]
    pub coverages: Vec<CoverageSpec>,
    /// Top-level folders linked (not copied) into the clean copy, e.g. node_modules.
    #[serde(default)]
    pub link: Vec<String>,
    /// What a changed file no check `covers` does: `"note"` (listed, default) or `"unverified"`.
    #[serde(default, skip_serializing_if = "crate::uncovered::Mode::is_default")]
    pub uncovered: crate::uncovered::Mode,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CheckSpec {
    pub name: String,
    pub run: String,
    #[serde(default = "default_timeout")]
    pub timeout_secs: u64,
    /// Run once per value: `{ schema = ["CA1", "MN3"] }`. One key.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub matrix: Option<BTreeMap<String, Vec<String>>>,
    /// Like `matrix`, with the values in a file under `.countercheck/`: `{ schema = ".countercheck/x.txt" }`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub matrix_file: Option<BTreeMap<String, String>>,
    /// After `run` passes, its output file must match a locked reference.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub golden: Option<GoldenSpec>,
    /// After `run` passes, a `METRIC <name>=<number>` it printed must be within limits.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub budget: Option<BudgetSpec>,
    /// The check needs the internet: run it with network access even under the sandbox. Set in the
    /// locked check file, so a worker can't grant itself the internet.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub network: bool,
    /// Which shell runs `run`: `sh` (default, every OS; Git for Windows' `sh` on Windows), `pwsh`
    /// or `cmd`.
    #[serde(default, skip_serializing_if = "Shell::is_default")]
    pub shell: Shell,
    /// Files this check looks at (globs, like `protected`). When any check declares `covers`, the
    /// receipt lists changed files that no check covers.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub covers: Vec<String>,
}

/// The shell a check's `run` is given to.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Shell {
    /// `sh -c <run>`.
    #[default]
    Sh,
    /// `pwsh -NoProfile -NonInteractive -Command <run>` (Windows PowerShell if pwsh is missing).
    Pwsh,
    /// `cmd /C <run>`.
    Cmd,
}

impl Shell {
    fn is_default(&self) -> bool {
        *self == Shell::Sh
    }
}

/// One run of a check, after matrix expansion: what the runner executes.
#[derive(Debug, Clone, PartialEq)]
pub struct PlannedCheck {
    /// The name on the receipt, e.g. `reconcile [schema=CA1]`.
    pub name: String,
    /// The command with `{key}` replaced.
    pub run: String,
    pub timeout_secs: u64,
    /// Extra environment, e.g. `COUNTERCHECK_MATRIX_SCHEMA=CA1`.
    pub env: Vec<(String, String)>,
    /// The golden comparison, with `{key}` replaced in its paths.
    pub golden: Option<GoldenSpec>,
    pub budget: Option<BudgetSpec>,
    /// Internet allowed for this run (`network = true` on the check).
    pub network: bool,
    pub shell: Shell,
}

fn default_timeout() -> u64 {
    DEFAULT_TIMEOUT_SECS
}

impl ChecksFile {
    /// Matrix files the checks need, to be read from the same revision as the check file.
    pub fn matrix_files(&self) -> Vec<String> {
        let mut out: Vec<String> = self
            .checks
            .iter()
            .filter_map(|c| c.matrix_file.as_ref())
            .flat_map(|m| m.values().cloned())
            .collect();
        out.sort();
        out.dedup();
        out
    }

    /// Every file the checks read besides the check file (matrix files, coverage item lists), to be
    /// read from the same revision as the check file.
    pub fn locked_files(&self) -> Vec<String> {
        let mut out = self.matrix_files();
        out.extend(self.coverages.iter().map(|c| c.items.clone()));
        out.sort();
        out.dedup();
        out
    }

    /// True when at least one `[[check]]` declares `covers`: only then are uncovered files listed.
    pub fn uncovered_enabled(&self) -> bool {
        self.checks.iter().any(|c| !c.covers.is_empty())
    }

    /// Every pattern that counts as covered: each check's `covers` plus the `paths` of every
    /// `[[forbid]]` and `[[coverage]]` (they scan those files).
    pub fn covers_patterns(&self) -> Vec<String> {
        let mut out: Vec<String> = self.checks.iter().flat_map(|c| c.covers.clone()).collect();
        out.extend(self.forbids.iter().flat_map(|f| f.paths.clone()));
        out.extend(self.coverages.iter().flat_map(|c| c.paths.clone()));
        out.sort();
        out.dedup();
        out
    }

    /// Protected patterns including the always-protected `.countercheck/**`.
    pub fn protected_patterns(&self) -> Vec<String> {
        let mut out = self.protected.clone();
        if !out.iter().any(|p| p == ALWAYS_PROTECTED) {
            out.push(ALWAYS_PROTECTED.to_string());
        }
        out
    }
}

/// Parse and validate a check file. Errors are plain sentences for the user.
pub fn parse(text: &str) -> Result<ChecksFile, String> {
    if text.len() > MAX_CHECKS_BYTES {
        return Err(format!(
            "{CHECKS_PATH} is larger than {MAX_CHECKS_BYTES} bytes"
        ));
    }
    let file: ChecksFile =
        toml::from_str(text).map_err(|e| format!("{CHECKS_PATH} is not valid: {e}"))?;
    if file.checks.is_empty() && file.forbids.is_empty() && file.coverages.is_empty() {
        return Err(format!(
            "{CHECKS_PATH} has no [[check]], [[forbid]] or [[coverage]] entries"
        ));
    }
    let mut coverage_names = std::collections::BTreeSet::new();
    for c in &file.coverages {
        c.validate()?;
        if !coverage_names.insert(c.name.as_str()) {
            return Err(format!("two [[coverage]] entries are named \"{}\"", c.name));
        }
    }
    let mut forbid_names = std::collections::BTreeSet::new();
    for f in &file.forbids {
        forbid::Rule::new(f)?;
        if !forbid_names.insert(f.name.as_str()) {
            return Err(format!("two [[forbid]] entries are named \"{}\"", f.name));
        }
    }
    let mut seen = std::collections::BTreeSet::new();
    for c in &file.checks {
        if c.name.trim().is_empty() {
            return Err("a check has an empty name".to_string());
        }
        if c.run.trim().is_empty() {
            return Err(format!("check \"{}\" has an empty run command", c.name));
        }
        if c.timeout_secs == 0 || c.timeout_secs > MAX_TIMEOUT_SECS {
            return Err(format!(
                "check \"{}\": timeout_secs must be between 1 and {MAX_TIMEOUT_SECS}",
                c.name
            ));
        }
        if !seen.insert(c.name.as_str()) {
            return Err(format!("two checks are named \"{}\"", c.name));
        }
        check_matrix_shape(c)?;
        if c.covers.iter().any(|p| p.trim().is_empty()) {
            return Err(format!("check \"{}\": covers has an empty pattern", c.name));
        }
        crate::protect::Protected::labeled(&c.covers, "covers pattern")
            .map_err(|e| format!("check \"{}\": {e}", c.name))?;
        if let Some(b) = &c.budget {
            b.validate(&c.name)?;
        }
    }
    for l in &file.link {
        let bad = l.is_empty()
            || l.starts_with('/')
            || l.split('/')
                .any(|seg| seg.is_empty() || seg == "." || seg == "..")
            || l.contains(['*', '?', '[', '\\'])
            || l.starts_with(".countercheck")
            || l.starts_with(".git");
        if bad {
            return Err(format!(
                "link entry \"{l}\" must be a plain relative folder"
            ));
        }
    }
    crate::protect::Protected::new(&file.protected_patterns())?;
    if file.uncovered == crate::uncovered::Mode::Unverified && !file.uncovered_enabled() {
        return Err(
            "uncovered = \"unverified\" needs at least one [[check]] with covers = [...], or every change would be uncovered"
                .to_string(),
        );
    }
    // Everything knowable without the matrix files is checked now; `plan` repeats it with them.
    plan_inner(&file, None)?;
    Ok(file)
}

/// A relative path inside the repository: no leading `/`, no `.`, `..` or empty segments.
pub fn plain_path(p: &str) -> bool {
    !p.is_empty()
        && p.len() <= 1024
        && !p.starts_with('/')
        && !p.contains(['\\', '\0'])
        && p.split('/')
            .all(|seg| !seg.is_empty() && seg != "." && seg != "..")
}

fn check_matrix_shape(c: &CheckSpec) -> Result<(), String> {
    let name = &c.name;
    let key = match (&c.matrix, &c.matrix_file) {
        (None, None) => return Ok(()),
        (Some(_), Some(_)) => {
            return Err(format!(
                "check \"{name}\": use matrix or matrix_file, not both"
            ));
        }
        (Some(m), None) => one_key(name, m.keys())?,
        (None, Some(m)) => one_key(name, m.keys())?,
    };
    if !matrix::valid_key(key) {
        return Err(format!(
            "check \"{name}\": matrix key \"{key}\" must be letters, digits or '_' (not starting with a digit)"
        ));
    }
    if let Some(path) = c.matrix_file.as_ref().and_then(|m| m.get(key))
        && (!plain_path(path) || !path.starts_with(".countercheck/"))
    {
        return Err(format!(
            "check \"{name}\": matrix_file \"{path}\" must be a plain path under .countercheck/ (so the worker can't change it)"
        ));
    }
    Ok(())
}

fn one_key<'a>(
    name: &str,
    mut keys: impl Iterator<Item = &'a String>,
) -> Result<&'a String, String> {
    match (keys.next(), keys.next()) {
        (Some(k), None) => Ok(k),
        (None, _) => Err(format!("check \"{name}\": the matrix has no key")),
        (Some(_), Some(_)) => Err(format!(
            "check \"{name}\": a matrix may have only one key for now"
        )),
    }
}

/// The golden spec of one run, with `{key}` replaced and its paths checked.
fn planned_golden(
    c: &CheckSpec,
    matrix_value: Option<(&String, &String)>,
    protected: &crate::protect::Protected,
) -> Result<Option<GoldenSpec>, String> {
    let Some(g) = &c.golden else {
        return Ok(None);
    };
    let name = &c.name;
    let sub = |t: &str| match matrix_value {
        Some((k, v)) => matrix::substitute(t, k, v),
        None => t.to_string(),
    };
    let (output, expected) = (sub(&g.output), sub(&g.expected));
    if !plain_path(&output) {
        return Err(format!(
            "check \"{name}\": golden output \"{output}\" must be a plain relative path"
        ));
    }
    if !plain_path(&expected) {
        return Err(format!(
            "check \"{name}\": golden expected \"{expected}\" must be a plain relative path"
        ));
    }
    if !expected.starts_with(".countercheck/") && !protected.matches(&expected) {
        return Err(format!(
            "check \"{name}\": golden expected \"{expected}\" must be under .countercheck/ or match a protected pattern, or the worker could edit it"
        ));
    }
    if !g.tolerance.is_finite() || g.tolerance < 0.0 {
        return Err(format!(
            "check \"{name}\": golden tolerance must be a number of 0 or more"
        ));
    }
    if g.mode == golden::Mode::Text && (g.sort_rows || g.tolerance != 0.0) {
        return Err(format!(
            "check \"{name}\": golden tolerance and sort_rows only apply to mode = \"csv\""
        ));
    }
    Ok(Some(GoldenSpec {
        output,
        expected,
        ..g.clone()
    }))
}

/// Expand every check into the runs to do. `matrix_files` maps each path from
/// [`ChecksFile::matrix_files`] to its text (read from the same revision as the check file).
pub fn plan(
    file: &ChecksFile,
    matrix_files: &BTreeMap<String, String>,
) -> Result<Vec<PlannedCheck>, String> {
    plan_inner(file, Some(matrix_files))
}

/// With `None`, checks that use a matrix file are skipped (their values aren't known yet).
fn plan_inner(
    file: &ChecksFile,
    matrix_files: Option<&BTreeMap<String, String>>,
) -> Result<Vec<PlannedCheck>, String> {
    let protected = crate::protect::Protected::new(&file.protected_patterns())?;
    let mut out: Vec<PlannedCheck> = Vec::new();
    for c in &file.checks {
        let values: Option<(&String, Vec<String>)> = match (&c.matrix, &c.matrix_file) {
            (Some(m), _) => m.iter().next().map(|(k, v)| (k, v.clone())),
            (None, Some(m)) => match (m.iter().next(), matrix_files) {
                (Some((k, path)), Some(texts)) => {
                    let text = texts.get(path).ok_or_else(|| {
                        format!("check \"{}\": matrix file {path} was not found", c.name)
                    })?;
                    Some((k, matrix::parse_values_file(&c.name, path, text)?))
                }
                _ => continue,
            },
            (None, None) => None,
        };
        let Some((key, values)) = values else {
            out.push(PlannedCheck {
                name: c.name.clone(),
                run: c.run.clone(),
                timeout_secs: c.timeout_secs,
                env: Vec::new(),
                golden: planned_golden(c, None, &protected)?,
                budget: c.budget.clone(),
                network: c.network,
                shell: c.shell,
            });
            continue;
        };
        if values.is_empty() {
            return Err(format!(
                "check \"{}\": the matrix has no values, so nothing would run",
                c.name
            ));
        }
        let mut seen = std::collections::BTreeSet::new();
        for v in &values {
            matrix::check_value(&c.name, v)?;
            if !seen.insert(v.as_str()) {
                return Err(format!(
                    "check \"{}\": matrix value \"{v}\" appears twice",
                    c.name
                ));
            }
            out.push(PlannedCheck {
                name: matrix::expansion_name(&c.name, key, v),
                run: matrix::substitute(&c.run, key, v),
                timeout_secs: c.timeout_secs,
                env: vec![(matrix::env_name(key), v.clone())],
                golden: planned_golden(c, Some((key, v)), &protected)?,
                budget: c.budget.clone(),
                network: c.network,
                shell: c.shell,
            });
            if out.len() > matrix::MAX_EXPANSIONS {
                break; // refused just below; stop before building more
            }
        }
    }
    if out.len() > matrix::MAX_EXPANSIONS {
        return Err(format!(
            "{CHECKS_PATH} expands to more than {} check runs",
            matrix::MAX_EXPANSIONS
        ));
    }
    // Every result on the receipt needs its own name.
    let mut names = std::collections::BTreeSet::new();
    let forbids = file.forbids.iter().map(ForbidSpec::result_name);
    let coverages = file.coverages.iter().map(CoverageSpec::result_name);
    for name in out
        .iter()
        .map(|p| p.name.clone())
        .chain(forbids)
        .chain(coverages)
    {
        if names.contains(&name) {
            return Err(format!("two checks are named \"{name}\""));
        }
        names.insert(name);
    }
    Ok(out)
}

/// The starter file `countercheck init` writes.
pub const TEMPLATE: &str = r#"# countercheck checks: what must be true before work counts as done.
# countercheck reads this file from the base revision, so a worker cannot change its own checks.

# Paths a worker may not change. If it does, countercheck runs the checks with the base version
# and says so on the receipt. .countercheck/** is always protected.
protected = ["tests/**"]

[[check]]
name = "tests"
run = "echo 'replace me with your test command, e.g. pytest -q or npm test'; exit 1"
timeout_secs = 600
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_a_valid_file_and_adds_countercheck_protection() {
        let f =
            parse("protected = [\"tests/**\"]\n[[check]]\nname = \"t\"\nrun = \"true\"\n").unwrap();
        assert_eq!(f.checks[0].timeout_secs, DEFAULT_TIMEOUT_SECS);
        assert_eq!(f.protected_patterns(), vec!["tests/**", ".countercheck/**"]);
    }

    #[test]
    fn network_is_off_unless_the_check_file_turns_it_on() {
        let f = parse(
            "[[check]]\nname = \"a\"\nrun = \"true\"\n[[check]]\nname = \"b\"\nrun = \"true\"\nnetwork = true\n",
        )
        .unwrap();
        assert!(!f.checks[0].network);
        assert!(f.checks[1].network);
        assert!(parse("[[check]]\nname = \"a\"\nrun = \"true\"\nnetwork = \"yes\"\n").is_err());
    }

    #[test]
    fn shell_defaults_to_sh_and_only_known_shells_parse() {
        let f = parse(
            "[[check]]\nname = \"a\"\nrun = \"true\"\n[[check]]\nname = \"b\"\nrun = \"dir\"\nshell = \"cmd\"\n[[check]]\nname = \"c\"\nrun = \"1\"\nshell = \"pwsh\"\n",
        )
        .unwrap();
        let shells: Vec<Shell> = f.checks.iter().map(|c| c.shell).collect();
        assert_eq!(shells, vec![Shell::Sh, Shell::Cmd, Shell::Pwsh]);
        let p = plan(&f, &BTreeMap::new()).unwrap();
        assert_eq!(p[1].shell, Shell::Cmd);
        for bad in ["bash", "SH", "", "powershell"] {
            let e = parse(&format!(
                "[[check]]\nname = \"a\"\nrun = \"true\"\nshell = \"{bad}\"\n"
            ))
            .unwrap_err();
            assert!(e.contains("shell") || e.contains("variant"), "{bad}: {e}");
        }
        // A check file that doesn't name a shell serializes as before.
        assert!(!toml::to_string(&f.checks[0]).unwrap().contains("shell"));
    }

    #[test]
    fn a_check_file_with_windows_line_endings_parses_the_same() {
        let lf = "protected = [\"tests/**\"]\n[[check]]\nname = \"a\"\nrun = \"sh t.sh\"\nmatrix_file = { s = \".countercheck/s.txt\" }\n";
        let crlf = lf.replace('\n', "\r\n");
        assert_eq!(parse(lf).unwrap(), parse(&crlf).unwrap());
        let mut texts = BTreeMap::new();
        texts.insert(
            ".countercheck/s.txt".to_string(),
            "# targets\r\nCA1\r\nNY2 # comment\r\n\r\n".to_string(),
        );
        let p = plan(&parse(&crlf).unwrap(), &texts).unwrap();
        let names: Vec<&str> = p.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(names, vec!["a [s=CA1]", "a [s=NY2]"]);
        assert_eq!(
            p[0].env,
            vec![("COUNTERCHECK_MATRIX_S".to_string(), "CA1".to_string())]
        );
    }

    #[test]
    fn rejects_bad_files() {
        assert!(parse("").unwrap_err().contains("no [[check]]"));
        assert!(parse("[[check]]\nname=\"a\"\nrun=\"\"\n").is_err());
        assert!(parse("[[check]]\nname=\"a\"\nrun=\"x\"\ntimeout_secs=0\n").is_err());
        assert!(
            parse("[[check]]\nname=\"a\"\nrun=\"x\"\n[[check]]\nname=\"a\"\nrun=\"y\"\n")
                .unwrap_err()
                .contains("two checks")
        );
        assert!(parse("[[check]]\nname=\"a\"\nrun=\"x\"\nextra=1\n").is_err());
        assert!(parse("protected=[\"[\"]\n[[check]]\nname=\"a\"\nrun=\"x\"\n").is_err());
        for bad in [
            "../x",
            "/abs",
            "a/../b",
            "*.x",
            ".git",
            ".countercheck/r",
            "",
        ] {
            let t = format!("link=[\"{bad}\"]\n[[check]]\nname=\"a\"\nrun=\"x\"\n");
            assert!(parse(&t).is_err(), "{bad}");
        }
        assert!(
            parse("link=[\"node_modules\",\".venv\"]\n[[check]]\nname=\"a\"\nrun=\"x\"\n").is_ok()
        );
    }

    #[test]
    fn limits_are_inclusive() {
        let one = "[[check]]\nname = \"a\"\nrun = \"x\"\n";
        // The size cap is 64 KiB: a file of exactly that size is read, one byte more is refused.
        let at_cap = format!("{one}{}", "#".repeat(65536 - one.len()));
        assert!(parse(&at_cap).is_ok());
        assert!(
            parse(&format!("{at_cap}#"))
                .unwrap_err()
                .contains("larger than")
        );
        let with_timeout = |t: u64| format!("{one}timeout_secs = {t}\n");
        assert!(parse(&with_timeout(1)).is_ok());
        assert!(parse(&with_timeout(7200)).is_ok());
        assert!(
            parse(&with_timeout(7201))
                .unwrap_err()
                .contains("between 1 and 7200")
        );
    }

    #[test]
    fn link_entries_with_empty_or_dot_segments_are_refused() {
        for bad in ["a//b", "a/", "./x", "a/./b", "."] {
            let t = format!("link=[\"{bad}\"]\n[[check]]\nname=\"a\"\nrun=\"x\"\n");
            assert!(parse(&t).is_err(), "{bad}");
        }
        assert!(parse("link=[\"a/b\"]\n[[check]]\nname=\"a\"\nrun=\"x\"\n").is_ok());
    }

    const M: &str = "[[check]]\nname = \"rec\"\nrun = \"r --schema {schema}\"\n";

    #[test]
    fn matrix_expands_into_named_runs() {
        let f = parse(&format!("{M}matrix = {{ schema = [\"CA1\", \"MN3\"] }}\n")).unwrap();
        let p = plan(&f, &BTreeMap::new()).unwrap();
        assert_eq!(p.len(), 2);
        assert_eq!(p[0].name, "rec [schema=CA1]");
        assert_eq!(p[0].run, "r --schema CA1");
        assert_eq!(
            p[1].env,
            vec![("COUNTERCHECK_MATRIX_SCHEMA".to_string(), "MN3".to_string())]
        );
        // A plain check plans as itself.
        let f = parse("[[check]]\nname = \"t\"\nrun = \"x {schema}\"\n").unwrap();
        let p = plan(&f, &BTreeMap::new()).unwrap();
        assert_eq!((p[0].name.as_str(), p[0].run.as_str()), ("t", "x {schema}"));
        assert!(p[0].env.is_empty());
    }

    #[test]
    fn matrix_values_must_be_plain() {
        for bad in ["a b", "x;rm -rf /", "$(id)", "", "a/b"] {
            let t = format!("{M}matrix = {{ schema = [\"{bad}\"] }}\n");
            let e = parse(&t).unwrap_err();
            assert!(e.contains("not allowed"), "{bad}: {e}");
        }
    }

    #[test]
    fn matrix_shape_is_checked() {
        let cases = [
            ("matrix = { a = [\"x\"], b = [\"y\"] }", "only one key"),
            ("matrix = {}", "no key"),
            ("matrix = { schema = [] }", "no values"),
            ("matrix = { schema = [\"x\", \"x\"] }", "appears twice"),
            ("matrix = { \"a-b\" = [\"x\"] }", "matrix key"),
            (
                "matrix = { s = [\"x\"] }\nmatrix_file = { s = \".countercheck/s.txt\" }",
                "not both",
            ),
            (
                "matrix_file = { s = \"targets/s.txt\" }",
                "under .countercheck/",
            ),
            (
                "matrix_file = { s = \".countercheck/../s.txt\" }",
                "under .countercheck/",
            ),
            (
                "matrix_file = { s = \"/etc/passwd\" }",
                "under .countercheck/",
            ),
        ];
        for (extra, want) in cases {
            let e = parse(&format!("{M}{extra}\n")).unwrap_err();
            assert!(e.contains(want), "{extra}: {e}");
        }
    }

    #[test]
    fn matrix_file_values_come_from_the_given_text() {
        let f = parse(&format!(
            "{M}matrix_file = {{ schema = \".countercheck/targets/s.txt\" }}\n"
        ))
        .unwrap();
        assert_eq!(f.matrix_files(), vec![".countercheck/targets/s.txt"]);
        let mut texts = BTreeMap::new();
        assert!(plan(&f, &texts).unwrap_err().contains("not found"));
        texts.insert(
            ".countercheck/targets/s.txt".to_string(),
            "# targets\nCA1\nNYJUV1\n".to_string(),
        );
        let p = plan(&f, &texts).unwrap();
        let names: Vec<&str> = p.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(names, vec!["rec [schema=CA1]", "rec [schema=NYJUV1]"]);
        texts.insert(
            ".countercheck/targets/s.txt".to_string(),
            "x|y\n".to_string(),
        );
        assert!(plan(&f, &texts).unwrap_err().contains("not allowed"));
        texts.insert(
            ".countercheck/targets/s.txt".to_string(),
            "# none\n".to_string(),
        );
        assert!(plan(&f, &texts).unwrap_err().contains("no values"));
    }

    #[test]
    fn at_most_1000_runs_per_file() {
        let vals = |n: usize| {
            (0..n)
                .map(|i| format!("\"v{i}\""))
                .collect::<Vec<_>>()
                .join(",")
        };
        let ok = format!("{M}matrix = {{ schema = [{}] }}\n", vals(1000));
        assert_eq!(
            plan(&parse(&ok).unwrap(), &BTreeMap::new()).unwrap().len(),
            1000
        );
        let over = format!(
            "{M}matrix = {{ schema = [{}] }}\n[[check]]\nname = \"one more\"\nrun = \"x\"\n",
            vals(1000)
        );
        assert!(parse(&over).unwrap_err().contains("more than 1000"));
    }

    #[test]
    fn expanded_names_must_be_unique() {
        let t = format!(
            "{M}matrix = {{ schema = [\"A\"] }}\n[[check]]\nname = \"rec [schema=A]\"\nrun = \"x\"\n"
        );
        assert!(parse(&t).unwrap_err().contains("two checks"));
    }

    const F: &str = "[[forbid]]\nname = \"no todo\"\npattern = 'TODO'\npaths = [\"src/**\"]\n";

    #[test]
    fn forbid_entries_parse_and_old_files_still_do() {
        let f = parse(F).unwrap();
        assert!(f.checks.is_empty());
        assert_eq!(f.forbids[0].name, "no todo");
        assert!(!f.forbids[0].case_insensitive);
        assert_eq!(f.forbids[0].message, None);
        let f = parse(&format!(
            "{F}message = \"Finish it.\"\ncase_insensitive = true\n[[check]]\nname = \"t\"\nrun = \"x\"\n"
        ))
        .unwrap();
        assert!(f.forbids[0].case_insensitive);
        assert_eq!(f.checks.len(), 1);
        // A file without [[forbid]] is unchanged: no forbids.
        assert!(
            parse("[[check]]\nname=\"a\"\nrun=\"x\"\n")
                .unwrap()
                .forbids
                .is_empty()
        );
    }

    #[test]
    fn bad_forbid_entries_are_refused() {
        let e =
            parse("[[forbid]]\nname = \"r\"\npattern = '(oops'\npaths = [\"a\"]\n").unwrap_err();
        assert!(e.contains("not a valid regex"), "{e}");
        let e = parse(&format!("{F}{F}")).unwrap_err();
        assert!(e.contains("two [[forbid]]"), "{e}");
        assert!(parse(&format!("{F}extra = 1\n")).is_err());
        assert!(parse("[[forbid]]\nname = \"r\"\npattern = 'x'\n").is_err());
        let e = parse("[[forbid]]\nname = \"r\"\npattern = 'x'\npaths = []\n").unwrap_err();
        assert!(e.contains("paths is empty"), "{e}");
        let e = parse(&format!(
            "{F}[[check]]\nname = \"forbid: no todo\"\nrun = \"x\"\n"
        ))
        .unwrap_err();
        assert!(e.contains("two checks"), "{e}");
    }

    const G: &str = "[[check]]\nname = \"g\"\nrun = \"x\"\n";

    #[test]
    fn golden_parses_with_defaults_and_matrix() {
        let f = parse(&format!(
            "{G}golden = {{ output = \"out/t.csv\", expected = \".countercheck/golden/t.csv\" }}\n"
        ))
        .unwrap();
        let p = plan(&f, &BTreeMap::new()).unwrap();
        let g = p[0].golden.clone().unwrap();
        assert_eq!(g.mode, golden::Mode::Text);
        assert_eq!(g.output, "out/t.csv");
        // `{key}` is replaced in both paths; a protected pattern also locks the reference.
        let f = parse(&format!(
            "protected = [\"golden/**\"]\n{G}matrix = {{ s = [\"A\", \"B\"] }}\n\
             golden = {{ output = \"out/{{s}}.csv\", expected = \"golden/{{s}}.csv\", mode = \"csv\", tolerance = 0.01, sort_rows = true }}\n"
        ))
        .unwrap();
        let p = plan(&f, &BTreeMap::new()).unwrap();
        let g = p[1].golden.clone().unwrap();
        assert_eq!(
            (g.output.as_str(), g.expected.as_str()),
            ("out/B.csv", "golden/B.csv")
        );
        assert!(g.sort_rows);
        assert_eq!(g.tolerance, 0.01);
        assert!(p.iter().all(|c| c.golden.is_some()));
        // No golden: none planned.
        assert!(
            plan(&parse(G).unwrap(), &BTreeMap::new()).unwrap()[0]
                .golden
                .is_none()
        );
    }

    #[test]
    fn golden_reference_must_be_protected() {
        let e = parse(&format!(
            "{G}golden = {{ output = \"out/t.csv\", expected = \"golden/t.csv\" }}\n"
        ))
        .unwrap_err();
        assert!(
            e.contains("must be under .countercheck/ or match a protected pattern"),
            "{e}"
        );
        let cases = [
            (
                "output = \"/tmp/o\", expected = \".countercheck/e\"",
                "plain relative path",
            ),
            (
                "output = \"../o\", expected = \".countercheck/e\"",
                "plain relative path",
            ),
            (
                "output = \"o\", expected = \".countercheck/../e\"",
                "plain relative path",
            ),
            (
                "output = \"o\", expected = \".countercheck/e\", tolerance = -1.0",
                "0 or more",
            ),
            (
                "output = \"o\", expected = \".countercheck/e\", tolerance = nan",
                "0 or more",
            ),
            (
                "output = \"o\", expected = \".countercheck/e\", sort_rows = true",
                "only apply to mode",
            ),
            (
                "output = \"o\", expected = \".countercheck/e\", tolerance = 0.1",
                "only apply to mode",
            ),
            (
                "output = \"o\", expected = \".countercheck/e\", mode = \"xml\"",
                "not valid",
            ),
            ("output = \"o\"", "not valid"),
        ];
        for (g, want) in cases {
            let e = parse(&format!("{G}golden = {{ {g} }}\n")).unwrap_err();
            assert!(e.contains(want), "{g}: {e}");
        }
        // A matrix value can't climb out of the protected folder either.
        let e = parse(&format!(
            "{G}matrix = {{ s = [\"..\"] }}\ngolden = {{ output = \"o\", expected = \".countercheck/{{s}}/e\" }}\n"
        ))
        .unwrap_err();
        assert!(e.contains("plain relative path"), "{e}");
    }

    #[test]
    fn budget_parses_and_is_checked() {
        let f = parse(&format!(
            "{G}budget = {{ metric = \"seconds\", max = 30 }}\nmatrix = {{ s = [\"a\", \"b\"] }}\n"
        ))
        .unwrap();
        let p = plan(&f, &BTreeMap::new()).unwrap();
        assert!(
            p.iter()
                .all(|c| c.budget.as_ref().is_some_and(|b| b.max == Some(30.0)))
        );
        for (b, want) in [
            ("metric = \"s\"", "min, a max"),
            ("metric = \"s\", min = 5, max = 1", "not be above"),
            ("metric = \"a b\", max = 1", "budget metric"),
            ("max = 1", "not valid"),
            ("metric = \"s\", max = 1, extra = 2", "not valid"),
        ] {
            let e = parse(&format!("{G}budget = {{ {b} }}\n")).unwrap_err();
            assert!(e.contains(want), "{b}: {e}");
        }
    }

    const C: &str = "[[coverage]]\nname = \"procs\"\nitems = \".countercheck/inv/procs.txt\"\npaths = [\"sql/**\"]\npattern = 'PROCEDURE\\s+{item}\\b'\n";

    #[test]
    fn coverage_entries_parse_alone_and_list_their_items_file() {
        let f = parse(C).unwrap();
        assert_eq!(f.coverages.len(), 1);
        assert!(!f.coverages[0].case_insensitive);
        let f = parse(&format!(
            "{C}{M}matrix_file = {{ schema = \".countercheck/t.txt\" }}\n"
        ))
        .unwrap();
        assert_eq!(
            f.locked_files(),
            vec![".countercheck/inv/procs.txt", ".countercheck/t.txt"]
        );
        assert!(
            parse(&format!("{C}{C}"))
                .unwrap_err()
                .contains("two [[coverage]]")
        );
        assert!(parse(&format!("{C}extra = 1\n")).is_err());
        let e = parse("[[coverage]]\nname = \"p\"\nitems = \"inv.txt\"\npaths = [\"a\"]\npattern = '{item}'\n")
            .unwrap_err();
        assert!(e.contains("under .countercheck/"), "{e}");
        let e = parse("[[coverage]]\nname = \"p\"\nitems = \".countercheck/i\"\npaths = [\"a\"]\npattern = 'x'\n")
            .unwrap_err();
        assert!(e.contains("{item}"), "{e}");
        let e = parse(&format!(
            "{C}[[check]]\nname = \"coverage: procs\"\nrun = \"x\"\n"
        ))
        .unwrap_err();
        assert!(e.contains("two checks"), "{e}");
    }

    #[test]
    fn the_example_check_file_parses() {
        let text = include_str!("../../../examples/data-migration/.countercheck/checks.toml");
        let f = parse(text).unwrap();
        let p = plan(&f, &BTreeMap::new()).unwrap();
        assert_eq!(p.len(), 4);
        assert_eq!((f.forbids.len(), f.coverages.len()), (1, 1));
    }

    #[test]
    fn template_parses() {
        assert!(parse(TEMPLATE).is_ok());
    }
}
