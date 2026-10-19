//! Golden checks: a command's output file must match a locked reference file.
//!
//! ```toml
//! [[check]]
//! name = "monthly totals"
//! run = "python3 scripts/export.py > out/totals.csv"
//! golden = { output = "out/totals.csv", expected = ".countercheck/golden/totals.csv", mode = "csv", tolerance = 1e-6, sort_rows = true }
//! ```
//!
//! This module only compares bytes; the binary reads the files.

use serde::{Deserialize, Serialize};
use std::cmp::Ordering;

/// At most this many differences are listed.
pub const MAX_DIFFS_SHOWN: usize = 20;
/// Cells and lines in reports are cut to this many characters.
pub const MAX_SHOWN_CHARS: usize = 200;
/// Output and expected files larger than this are refused (they come from a repository we don't control).
pub const MAX_FILE_BYTES: u64 = 64 * 1024 * 1024;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    /// Exact match after normalising line endings.
    #[default]
    Text,
    /// Same header, rows compared cell by cell, numbers within a tolerance.
    Csv,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GoldenSpec {
    /// The file the command writes, relative to the repository root.
    pub output: String,
    /// The locked reference: under `.countercheck/` or matching a `protected` pattern.
    pub expected: String,
    #[serde(default)]
    pub mode: Mode,
    /// Absolute tolerance for cells that are numbers on both sides (csv only).
    #[serde(default)]
    pub tolerance: f64,
    /// Compare rows as a set: sort both sides first (csv only).
    #[serde(default)]
    pub sort_rows: bool,
}

/// The result of comparing an output with its reference.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Comparison {
    pub equal: bool,
    /// Plain lines explaining the differences (empty when equal).
    pub report: String,
}

impl Comparison {
    fn same() -> Self {
        Self {
            equal: true,
            report: String::new(),
        }
    }

    fn differs(report: String) -> Self {
        Self {
            equal: false,
            report: report.trim_end().to_string(),
        }
    }
}

/// Compare according to the spec's mode.
pub fn compare(spec: &GoldenSpec, expected: &[u8], actual: &[u8]) -> Comparison {
    match spec.mode {
        Mode::Text => compare_text(expected, actual),
        Mode::Csv => compare_csv(expected, actual, spec.tolerance, spec.sort_rows),
    }
}

fn cut(s: &str) -> String {
    if s.chars().count() > MAX_SHOWN_CHARS {
        let mut c: String = s.chars().take(MAX_SHOWN_CHARS).collect();
        c.push_str("...");
        c
    } else {
        s.to_string()
    }
}

fn normalise(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes)
        .replace("\r\n", "\n")
        .replace('\r', "\n")
}

/// Exact match after normalising line endings; reports the first differing line.
pub fn compare_text(expected: &[u8], actual: &[u8]) -> Comparison {
    let (e, a) = (normalise(expected), normalise(actual));
    if e == a {
        return Comparison::same();
    }
    let el: Vec<&str> = e.split('\n').collect();
    let al: Vec<&str> = a.split('\n').collect();
    let n = el.len().max(al.len());
    for i in 0..n {
        let (x, y) = (el.get(i), al.get(i));
        if x != y {
            let show = |l: Option<&&str>| {
                l.map_or("(end of file)".to_string(), |l| format!("{:?}", cut(l)))
            };
            return Comparison::differs(format!(
                "line {}: expected {}, got {}",
                i + 1,
                show(x),
                show(y)
            ));
        }
    }
    Comparison::differs("the files differ".to_string())
}

/// A finite number, if the cell is one.
fn number(cell: &str) -> Option<f64> {
    cell.trim().parse::<f64>().ok().filter(|v| v.is_finite())
}

/// Cell order for sorting rows: numbers by value before text, text by bytes.
fn cell_order(a: &str, b: &str) -> Ordering {
    match (number(a), number(b)) {
        (Some(x), Some(y)) => x.total_cmp(&y),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => a.cmp(b),
    }
}

fn row_order(a: &[String], b: &[String]) -> Ordering {
    for (x, y) in a.iter().zip(b) {
        let o = cell_order(x, y);
        if o != Ordering::Equal {
            return o;
        }
    }
    a.len().cmp(&b.len())
}

fn cells_match(e: &str, a: &str, tolerance: f64) -> bool {
    match (number(e), number(a)) {
        (Some(x), Some(y)) => (x - y).abs() <= tolerance,
        _ => e == a,
    }
}

fn read_csv(bytes: &[u8]) -> Result<Vec<Vec<String>>, String> {
    let text = normalise(bytes);
    let mut rdr = csv::ReaderBuilder::new()
        .has_headers(false)
        .flexible(true)
        .from_reader(text.as_bytes());
    let mut rows = Vec::new();
    for rec in rdr.records() {
        let rec = rec.map_err(|e| e.to_string())?;
        rows.push(rec.iter().map(str::to_string).collect());
    }
    Ok(rows)
}

/// Same header; rows compared cell by cell; numeric cells within `tolerance`.
pub fn compare_csv(expected: &[u8], actual: &[u8], tolerance: f64, sort_rows: bool) -> Comparison {
    let mut exp = match read_csv(expected) {
        Ok(r) => r,
        Err(e) => return Comparison::differs(format!("the expected file is not valid CSV: {e}")),
    };
    let mut act = match read_csv(actual) {
        Ok(r) => r,
        Err(e) => return Comparison::differs(format!("the output is not valid CSV: {e}")),
    };
    let tolerance = if tolerance.is_finite() && tolerance > 0.0 {
        tolerance
    } else {
        0.0
    };
    let eh = if exp.is_empty() {
        Vec::new()
    } else {
        exp.remove(0)
    };
    let ah = if act.is_empty() {
        Vec::new()
    } else {
        act.remove(0)
    };
    if eh != ah {
        return Comparison::differs(format!(
            "the header differs: expected {:?}, got {:?}",
            cut(&eh.join(",")),
            cut(&ah.join(","))
        ));
    }
    if sort_rows {
        exp.sort_by(|a, b| row_order(a, b));
        act.sort_by(|a, b| row_order(a, b));
    }
    let mut lines = Vec::new();
    if exp.len() != act.len() {
        lines.push(format!(
            "row count differs: expected {}, got {}",
            exp.len(),
            act.len()
        ));
    }
    let mut diffs = 0usize;
    let mut shown = Vec::new();
    let col_name = |c: usize| {
        eh.get(c)
            .filter(|h| !h.is_empty())
            .map_or_else(|| (c + 1).to_string(), |h| cut(h))
    };
    for (r, (er, ar)) in exp.iter().zip(&act).enumerate() {
        for c in 0..er.len().max(ar.len()) {
            let (e, a) = (er.get(c), ar.get(c));
            let same = match (e, a) {
                (Some(e), Some(a)) => cells_match(e, a, tolerance),
                _ => false,
            };
            if !same {
                diffs += 1;
                if shown.len() < MAX_DIFFS_SHOWN {
                    let show = |v: Option<&String>| v.map_or("(missing)".to_string(), |v| cut(v));
                    shown.push(format!(
                        "row {}, column {}: expected {}, got {}",
                        r + 1,
                        col_name(c),
                        show(e),
                        show(a)
                    ));
                }
            }
        }
    }
    if diffs == 0 && exp.len() == act.len() {
        return Comparison::same();
    }
    if diffs > 0 {
        lines.push(format!(
            "{diffs} cell(s) differ{}",
            if sort_rows {
                " (rows sorted before comparing)"
            } else {
                ""
            }
        ));
    }
    lines.extend(shown);
    if diffs > MAX_DIFFS_SHOWN {
        lines.push(format!(
            "... and {} more not shown",
            diffs - MAX_DIFFS_SHOWN
        ));
    }
    Comparison::differs(lines.join("\n"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_ignores_line_endings_and_reports_the_first_difference() {
        assert!(compare_text(b"a\r\nb\r\n", b"a\nb\n").equal);
        let c = compare_text(b"a\nb\nc\n", b"a\nB\nc\n");
        assert!(!c.equal);
        assert_eq!(c.report, "line 2: expected \"b\", got \"B\"");
        let c = compare_text(b"a\nb\n", b"a\n");
        assert_eq!(c.report, "line 2: expected \"b\", got \"\"");
        let c = compare_text(b"a\n", b"a\nextra");
        assert_eq!(c.report, "line 2: expected \"\", got \"extra\"");
        let c = compare_text(b"a", b"a\nb");
        assert_eq!(c.report, "line 2: expected (end of file), got \"b\"");
    }

    const E: &[u8] = b"id,name,amount\n1,north,10.5\n2,south,20\n3,east,30\n";

    #[test]
    fn csv_equal_and_numbers() {
        assert!(compare_csv(E, E, 0.0, false).equal);
        // 20 and 20.0 are the same number.
        assert!(
            compare_csv(
                E,
                b"id,name,amount\n1,north,10.5\n2,south,20.0\n3,east,30\n",
                0.0,
                false
            )
            .equal
        );
        // Line endings don't matter either.
        assert!(
            compare_csv(
                E,
                b"id,name,amount\r\n1,north,10.5\r\n2,south,20\r\n3,east,30\r\n",
                0.0,
                false
            )
            .equal
        );
    }

    #[test]
    fn csv_tolerance_is_absolute() {
        let near = b"id,name,amount\n1,north,10.5000001\n2,south,20\n3,east,30\n";
        assert!(!compare_csv(E, near, 0.0, false).equal);
        assert!(compare_csv(E, near, 1e-6, false).equal);
        let far = b"id,name,amount\n1,north,10.6\n2,south,20\n3,east,30\n";
        let c = compare_csv(E, far, 1e-6, false);
        assert!(!c.equal);
        assert!(
            c.report
                .contains("row 1, column amount: expected 10.5, got 10.6"),
            "{}",
            c.report
        );
        // Text never gets a tolerance.
        assert!(!compare_csv(b"a\nx\n", b"a\ny\n", 1.0, false).equal);
    }

    #[test]
    fn csv_sort_rows_ignores_order() {
        let shuffled = b"id,name,amount\n3,east,30\n1,north,10.5\n2,south,20\n";
        assert!(compare_csv(E, shuffled, 0.0, true).equal);
        let c = compare_csv(E, shuffled, 0.0, false);
        assert!(!c.equal);
        assert!(c.report.starts_with("9 cell(s) differ"), "{}", c.report);
        assert!(
            c.report.contains("row 1, column id: expected 1, got 3"),
            "{}",
            c.report
        );
        // Numbers sort by value, so 9 comes before 10 on both sides.
        let a = b"n\n10\n9\n";
        let b = b"n\n9.0\n10\n";
        assert!(compare_csv(a, b, 0.0, true).equal);
    }

    #[test]
    fn csv_header_and_row_count() {
        let c = compare_csv(E, b"id,name,total\n1,north,10.5\n", 0.0, false);
        assert!(c.report.starts_with("the header differs"), "{}", c.report);
        let c = compare_csv(E, b"id,name,amount\n1,north,10.5\n2,south,20\n", 0.0, false);
        assert!(!c.equal);
        assert_eq!(c.report, "row count differs: expected 3, got 2");
        let c = compare_csv(
            E,
            b"id,name,amount\n1,north,10.5\n2,south\n3,east,30\n",
            0.0,
            false,
        );
        assert!(
            c.report
                .contains("row 2, column amount: expected 20, got (missing)"),
            "{}",
            c.report
        );
    }

    #[test]
    fn csv_lists_at_most_20_differences() {
        let mut a = String::from("n\n");
        let mut b = String::from("n\n");
        for i in 0..30 {
            a.push_str(&format!("{i}\n"));
            b.push_str(&format!("{}\n", i + 100));
        }
        let c = compare_csv(a.as_bytes(), b.as_bytes(), 0.0, false);
        assert_eq!(
            c.report.lines().filter(|l| l.starts_with("row ")).count(),
            20
        );
        assert!(
            c.report.contains("... and 10 more not shown"),
            "{}",
            c.report
        );
    }

    #[test]
    fn spec_defaults() {
        let s: GoldenSpec = toml::from_str("output = \"o\"\nexpected = \"e\"\n").unwrap();
        assert_eq!(s.mode, Mode::Text);
        assert_eq!(s.tolerance, 0.0);
        assert!(!s.sort_rows);
        assert!(
            toml::from_str::<GoldenSpec>("output = \"o\"\nexpected = \"e\"\nmode = \"xml\"\n")
                .is_err()
        );
    }
}
