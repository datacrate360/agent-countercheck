//! The environment block for a process started as another account (`CreateProcessWithLogonW`):
//! `NAME=value` pairs as NUL-terminated UTF-16 strings, sorted by name without regard to case (as
//! Windows expects), ending with an empty string. Pure, so it is tested on every OS.
#![forbid(unsafe_code)]

/// Build the block. Names may not be empty or contain `=` (except a leading one, which Windows
/// uses for per-drive current directories); neither names nor values may contain NUL.
pub fn env_block(vars: &[(String, String)]) -> Result<Vec<u16>, String> {
    let mut sorted: Vec<&(String, String)> = vars.iter().collect();
    sorted.sort_by_key(|(k, _)| k.to_uppercase());
    let mut out = Vec::new();
    let mut last: Option<String> = None;
    for (k, v) in sorted {
        let body = k.strip_prefix('=').unwrap_or(k);
        if body.is_empty() || body.contains('=') || k.contains('\0') || v.contains('\0') {
            return Err(format!("environment variable {k:?} can't be passed on"));
        }
        let upper = k.to_uppercase();
        if last.as_deref() == Some(upper.as_str()) {
            return Err(format!("environment variable {k} is set twice"));
        }
        last = Some(upper);
        out.extend(format!("{k}={v}").encode_utf16());
        out.push(0);
    }
    if out.is_empty() {
        out.push(0);
    }
    out.push(0);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(v: &[(&str, &str)]) -> Vec<(String, String)> {
        v.iter()
            .map(|(a, b)| (a.to_string(), b.to_string()))
            .collect()
    }

    #[test]
    fn sorted_without_case_and_double_nul_terminated() {
        let b = env_block(&s(&[
            ("path", "C:\\x"),
            ("ComSpec", "cmd"),
            ("=C:", "C:\\"),
        ]))
        .unwrap();
        let text = String::from_utf16(&b).unwrap();
        assert_eq!(text, "=C:=C:\\\0ComSpec=cmd\0path=C:\\x\0\0");
        assert_eq!(env_block(&[]).unwrap(), vec![0, 0]);
    }

    #[test]
    fn bad_names_and_values_are_refused() {
        for bad in [
            s(&[("", "x")]),
            s(&[("A=B", "x")]),
            s(&[("A", "x\0y")]),
            s(&[("Path", "a"), ("PATH", "b")]),
        ] {
            assert!(env_block(&bad).is_err(), "{bad:?}");
        }
    }
}
