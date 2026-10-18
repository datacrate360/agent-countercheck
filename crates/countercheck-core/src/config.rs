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
//! `link = ["node_modules", ".venv"]` makes git-ignored dependency folders visible in the clean
//! copy (read-only under the sandbox). The receipt lists them, because they come from the worker's tree.
//!
//! countercheck reads this file from the **base** revision (before the worker's change), so a worker
//! cannot weaken the checks it is judged by.

use serde::{Deserialize, Serialize};

/// Where the check file lives, relative to the repository root.
pub const CHECKS_PATH: &str = ".countercheck/checks.toml";
/// Always protected, whatever the file says.
pub const ALWAYS_PROTECTED: &str = ".countercheck/**";
pub const DEFAULT_TIMEOUT_SECS: u64 = 600;
pub const MAX_TIMEOUT_SECS: u64 = 7200;
/// Refuse absurdly large check files (they come from a repository we don't control).
pub const MAX_CHECKS_BYTES: usize = 64 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
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

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CheckSpec {
    pub name: String,
    pub run: String,
    #[serde(default = "default_timeout")]
    pub timeout_secs: u64,
}

fn default_timeout() -> u64 {
    DEFAULT_TIMEOUT_SECS
}

impl ChecksFile {
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
    Ok(file)
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
    fn template_parses() {
        assert!(parse(TEMPLATE).is_ok());
    }
}
