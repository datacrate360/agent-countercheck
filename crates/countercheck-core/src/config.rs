//! The check file a project commits at `.countercheck/checks.toml`.
//!
//! ```toml
//! protected = ["tests/**"]      # paths a worker may not change; .countercheck/** is always protected
//!
//! [[check]]
//! name = "unit tests"
//! run = "pytest -q"             # run by `sh -c` in a clean copy of the project
//! timeout_secs = 600            # optional, default 600, max 7200
//! ```
//!
//! A check can also run once per target (`matrix`, see [`crate::matrix`]). docs/checks.md has every
//! check type with an example.
//!
//! `link = ["node_modules", ".venv"]` makes git-ignored dependency folders visible in the clean
//! copy (read-only under the sandbox). The receipt lists them, because they come from the worker's tree.
//!
//! countercheck reads this file from the **base** revision (before the worker's change), so a worker
//! cannot weaken the checks it is judged by.

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
    /// Top-level folders linked (not copied) into the clean copy, e.g. node_modules.
    #[serde(default)]
    pub link: Vec<String>,
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
    if file.checks.is_empty() {
        return Err(format!("{CHECKS_PATH} has no [[check]] entries"));
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
    let mut names = std::collections::BTreeSet::new();
    for p in &out {
        if !names.insert(p.name.as_str()) {
            return Err(format!("two checks are named \"{}\"", p.name));
        }
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

    #[test]
    fn template_parses() {
        assert!(parse(TEMPLATE).is_ok());
    }
}
