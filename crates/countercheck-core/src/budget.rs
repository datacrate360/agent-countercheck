//! Budget checks: a number the command prints must stay within a limit.
//!
//! ```toml
//! [[check]]
//! name = "export speed"
//! run = "python3 scripts/bench.py"          # prints a line like  METRIC seconds=12.4
//! budget = { metric = "seconds", max = 30 }  # or min = ..., or both
//! ```
//!
//! The last `METRIC <name>=<number>` line of the command's output counts.

use serde::{Deserialize, Serialize};

/// Longest allowed metric name.
pub const MAX_METRIC_LEN: usize = 64;

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BudgetSpec {
    pub metric: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max: Option<f64>,
}

impl BudgetSpec {
    /// Check the spec itself. Errors are plain sentences.
    pub fn validate(&self, check: &str) -> Result<(), String> {
        let m = &self.metric;
        let ok_name = !m.is_empty()
            && m.len() <= MAX_METRIC_LEN
            && m.bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'.' | b'-'));
        if !ok_name {
            return Err(format!(
                "check \"{check}\": budget metric \"{m}\" must be 1 to {MAX_METRIC_LEN} letters, digits, '_', '.' or '-'"
            ));
        }
        match (self.min, self.max) {
            (None, None) => Err(format!(
                "check \"{check}\": budget needs a min, a max or both"
            )),
            (Some(v), _) | (_, Some(v)) if !v.is_finite() => {
                Err(format!("check \"{check}\": budget limits must be numbers"))
            }
            (Some(lo), Some(hi)) if !hi.is_finite() || lo > hi => Err(format!(
                "check \"{check}\": budget min must not be above max"
            )),
            _ => Ok(()),
        }
    }

    /// The limit in words, e.g. `max 30` or `between 1 and 30`.
    pub fn limit_text(&self) -> String {
        match (self.min, self.max) {
            (Some(lo), Some(hi)) => format!("between {lo} and {hi}"),
            (Some(lo), None) => format!("min {lo}"),
            (None, Some(hi)) => format!("max {hi}"),
            (None, None) => "no limit".to_string(),
        }
    }
}

/// The value on one output line, if it is `METRIC <name>=<number>` for this metric.
pub fn metric_in_line(line: &str, name: &str) -> Option<f64> {
    let rest = line.trim().strip_prefix("METRIC ")?;
    let (k, v) = rest.trim().split_once('=')?;
    if k.trim() != name {
        return None;
    }
    v.trim().parse::<f64>().ok().filter(|x| x.is_finite())
}

/// The last value of the metric in some output text.
pub fn last_metric(text: &str, name: &str) -> Option<f64> {
    text.lines().rev().find_map(|l| metric_in_line(l, name))
}

/// Pass or fail, and one plain line for the receipt.
pub fn judge(spec: &BudgetSpec, value: Option<f64>) -> (bool, String) {
    let Some(v) = value else {
        return (
            false,
            format!(
                "budget: the output has no \"METRIC {}=<number>\" line",
                spec.metric
            ),
        );
    };
    let low = spec.min.is_some_and(|lo| v < lo);
    let high = spec.max.is_some_and(|hi| v > hi);
    let verdict = if low {
        "below the limit"
    } else if high {
        "over the limit"
    } else {
        "within the limit"
    };
    (
        !low && !high,
        format!(
            "budget: {} = {v} ({}; {verdict})",
            spec.metric,
            spec.limit_text()
        ),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec(min: Option<f64>, max: Option<f64>) -> BudgetSpec {
        BudgetSpec {
            metric: "seconds".into(),
            min,
            max,
        }
    }

    #[test]
    fn metric_lines_with_windows_line_endings_parse() {
        assert_eq!(metric_in_line("METRIC seconds=1.5\r", "seconds"), Some(1.5));
        assert_eq!(
            last_metric("x\r\nMETRIC seconds=2\r\nMETRIC seconds=3\r\n", "seconds"),
            Some(3.0)
        );
    }

    #[test]
    fn the_last_metric_line_counts() {
        let out = "warming up\nMETRIC seconds=40\nMETRIC rows=10\n  METRIC seconds = 12.4  \nMETRIC seconds=abc\n";
        assert_eq!(last_metric(out, "seconds"), Some(12.4));
        assert_eq!(last_metric(out, "rows"), Some(10.0));
        assert_eq!(last_metric(out, "missing"), None);
        assert_eq!(metric_in_line("METRICS seconds=1", "seconds"), None);
        assert_eq!(metric_in_line("METRIC seconds=inf", "seconds"), None);
        assert_eq!(metric_in_line("note: METRIC seconds=1", "seconds"), None);
    }

    #[test]
    fn limits_are_inclusive() {
        assert!(judge(&spec(None, Some(30.0)), Some(30.0)).0);
        assert!(!judge(&spec(None, Some(30.0)), Some(30.1)).0);
        assert!(judge(&spec(Some(1.0), None), Some(1.0)).0);
        assert!(!judge(&spec(Some(1.0), None), Some(0.5)).0);
        let (ok, line) = judge(&spec(Some(1.0), Some(30.0)), Some(12.4));
        assert!(ok);
        assert_eq!(
            line,
            "budget: seconds = 12.4 (between 1 and 30; within the limit)"
        );
        let (ok, line) = judge(&spec(None, Some(30.0)), Some(41.0));
        assert!(!ok);
        assert_eq!(line, "budget: seconds = 41 (max 30; over the limit)");
    }

    #[test]
    fn a_missing_metric_fails_plainly() {
        let (ok, line) = judge(&spec(None, Some(1.0)), None);
        assert!(!ok);
        assert_eq!(
            line,
            "budget: the output has no \"METRIC seconds=<number>\" line"
        );
    }

    #[test]
    fn bad_specs_are_refused() {
        assert!(spec(None, Some(1.0)).validate("c").is_ok());
        assert!(
            spec(None, None)
                .validate("c")
                .unwrap_err()
                .contains("min, a max")
        );
        assert!(
            spec(Some(2.0), Some(1.0))
                .validate("c")
                .unwrap_err()
                .contains("not be above")
        );
        assert!(spec(Some(f64::NAN), None).validate("c").is_err());
        assert!(spec(None, Some(f64::INFINITY)).validate("c").is_err());
        let mut s = spec(None, Some(1.0));
        s.metric = "a b".into();
        assert!(s.validate("c").unwrap_err().contains("budget metric"));
        s.metric = String::new();
        assert!(s.validate("c").is_err());
    }
}
