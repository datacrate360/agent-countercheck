//! Matrix checks: one `[[check]]` run once per target value.
//!
//! ```toml
//! [[check]]
//! name = "reconcile"
//! run = "python3 scripts/reconcile.py --schema {schema}"
//! matrix = { schema = ["CA1", "NYJUV1", "MN3"] }
//! # or: matrix_file = { schema = ".countercheck/targets/schemas.txt" }
//! ```
//!
//! Values are pasted into a shell command, so only plain characters are allowed.

/// At most this many runs come out of one check file, counting every matrix expansion.
pub const MAX_EXPANSIONS: usize = 1000;
/// Longest allowed matrix value.
pub const MAX_VALUE_LEN: usize = 128;
/// Longest allowed matrix key (it becomes part of an environment variable name).
pub const MAX_KEY_LEN: usize = 64;
/// Environment variable prefix: `COUNTERCHECK_MATRIX_<KEY>` holds the value.
pub const ENV_PREFIX: &str = "COUNTERCHECK_MATRIX_";

/// A value may go into a shell command: `^[A-Za-z0-9_.-]{1,128}$`, nothing else.
pub fn valid_value(v: &str) -> bool {
    !v.is_empty()
        && v.len() <= MAX_VALUE_LEN
        && v.bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'.' | b'-'))
}

/// A key names the `{key}` placeholder and an environment variable: `^[A-Za-z_][A-Za-z0-9_]{0,63}$`.
pub fn valid_key(k: &str) -> bool {
    let mut bytes = k.bytes();
    let first_ok = bytes
        .next()
        .is_some_and(|b| b.is_ascii_alphabetic() || b == b'_');
    first_ok && k.len() <= MAX_KEY_LEN && bytes.all(|b| b.is_ascii_alphanumeric() || b == b'_')
}

/// Check one value, with a plain message naming where it came from.
pub fn check_value(check: &str, v: &str) -> Result<(), String> {
    if valid_value(v) {
        Ok(())
    } else {
        Err(format!(
            "check \"{check}\": matrix value \"{}\" is not allowed; use 1 to {MAX_VALUE_LEN} letters, digits, '_', '.' or '-'",
            v.chars().take(40).collect::<String>()
        ))
    }
}

/// Read a matrix file: one value per line; blank lines and `#` comments are skipped.
pub fn parse_values_file(check: &str, path: &str, text: &str) -> Result<Vec<String>, String> {
    let mut out = Vec::new();
    for line in text.lines() {
        let v = line.split('#').next().unwrap_or("").trim();
        if v.is_empty() {
            continue;
        }
        check_value(check, v).map_err(|e| format!("{e} (in {path})"))?;
        out.push(v.to_string());
        if out.len() > MAX_EXPANSIONS {
            return Err(format!(
                "{path} has more than {MAX_EXPANSIONS} matrix values"
            ));
        }
    }
    Ok(out)
}

/// The environment variable that carries the value of `key`.
pub fn env_name(key: &str) -> String {
    format!("{ENV_PREFIX}{}", key.to_ascii_uppercase())
}

/// The name of one expansion on the receipt, e.g. `reconcile [schema=CA1]`.
pub fn expansion_name(check: &str, key: &str, value: &str) -> String {
    format!("{check} [{key}={value}]")
}

/// Replace `{key}` with the value (every occurrence).
pub fn substitute(text: &str, key: &str, value: &str) -> String {
    text.replace(&format!("{{{key}}}"), value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn values_are_plain() {
        for ok in ["CA1", "NYJUV1", "a.b-c_d", "0", &"x".repeat(128)] {
            assert!(valid_value(ok), "{ok}");
        }
        for bad in [
            "",
            "a b",
            "a;b",
            "$(x)",
            "`x`",
            "a/b",
            "a'b",
            "é",
            &"x".repeat(129),
        ] {
            assert!(!valid_value(bad), "{bad}");
        }
    }

    #[test]
    fn keys_are_identifiers() {
        assert!(valid_key("schema"));
        assert!(valid_key("_x1"));
        assert!(!valid_key(""));
        assert!(!valid_key("1x"));
        assert!(!valid_key("a-b"));
        assert!(!valid_key(&"k".repeat(65)));
        assert_eq!(env_name("schema"), "COUNTERCHECK_MATRIX_SCHEMA");
    }

    #[test]
    fn values_file_skips_comments_and_blanks() {
        let v = parse_values_file("c", "f", "# targets\nCA1\n\n  MN3  # trailing\n").unwrap();
        assert_eq!(v, vec!["CA1", "MN3"]);
        let e = parse_values_file("c", ".countercheck/t.txt", "CA1\nrm -rf /\n").unwrap_err();
        assert!(
            e.contains("not allowed") && e.contains(".countercheck/t.txt"),
            "{e}"
        );
        let many: String = (0..=MAX_EXPANSIONS).map(|i| format!("v{i}\n")).collect();
        assert!(
            parse_values_file("c", "f", &many)
                .unwrap_err()
                .contains("more than 1000")
        );
    }

    #[test]
    fn substitution_and_names() {
        assert_eq!(
            substitute("x --schema {schema} {schema} {other}", "schema", "CA1"),
            "x --schema CA1 CA1 {other}"
        );
        assert_eq!(
            expansion_name("reconcile", "schema", "CA1"),
            "reconcile [schema=CA1]"
        );
    }
}
