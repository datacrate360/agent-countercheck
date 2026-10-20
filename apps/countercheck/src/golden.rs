//! Golden checks: compare a check's output file in the clean copy with its locked reference.

use countercheck_core::golden::{self, GoldenSpec};
use countercheck_core::receipt::CheckResult;
use std::fs;
use std::path::Path;

/// Remove an output file the worker left in the clean copy, so only what the command writes
/// is compared (a committed fake output can't pass).
pub fn clear_output(work: &Path, g: &GoldenSpec) -> Result<(), String> {
    let rel = crate::snapshot::safe_rel(&g.output)?;
    if crate::snapshot::link_on_the_way(work, &rel) {
        return Ok(()); // outside the clean copy: never deleted, refused after the run
    }
    let path = work.join(rel);
    match fs::symlink_metadata(&path) {
        Ok(m) if m.is_dir() => Ok(()), // reported as "not a file" after the run
        Ok(_) => fs::remove_file(&path).map_err(|e| format!("{}: {e}", g.output)),
        Err(_) => Ok(()),
    }
}

/// Read the output file, refusing anything that leaves the clean copy or is too big.
fn read_output(work: &Path, g: &GoldenSpec) -> Result<Vec<u8>, String> {
    let path = work.join(crate::snapshot::safe_rel(&g.output)?);
    let meta = fs::symlink_metadata(&path)
        .map_err(|_| format!("the check did not write its output file {}", g.output))?;
    if meta.file_type().is_symlink() {
        return Err(format!(
            "the output {} is a symlink; countercheck doesn't follow it",
            g.output
        ));
    }
    if !meta.is_file() {
        return Err(format!("the output {} is not a file", g.output));
    }
    // A folder on the way could be a symlink out of the clean copy.
    let real = crate::snapshot::canonical(&path).map_err(|e| format!("{}: {e}", g.output))?;
    if !real.starts_with(work) {
        return Err(format!("the output {} is outside the clean copy", g.output));
    }
    if meta.len() > golden::MAX_FILE_BYTES {
        return Err(format!("the output {} is larger than 64 MiB", g.output));
    }
    fs::read(&real).map_err(|e| format!("{}: {e}", g.output))
}

/// After the command passed: compare and record the outcome on the result.
pub fn judge(
    work: &Path,
    g: &GoldenSpec,
    expected: &Result<Vec<u8>, String>,
    result: &mut CheckResult,
) {
    if !result.passed {
        return;
    }
    let outcome = match (expected, read_output(work, g)) {
        (Err(e), _) => Err(e.clone()),
        (_, Err(e)) => Err(e),
        (Ok(exp), Ok(out)) => {
            let c = golden::compare(g, exp, &out);
            if c.equal { Ok(()) } else { Err(c.report) }
        }
    };
    let summary = match &outcome {
        Ok(()) => format!("golden: {} matches {}", g.output, g.expected),
        Err(report) => format!(
            "golden: {} does not match {}\n{report}",
            g.output, g.expected
        ),
    };
    result.passed = outcome.is_ok();
    if result.output_tail.is_empty() {
        result.output_tail = summary;
    } else {
        result.output_tail = format!("{}\n{summary}", result.output_tail);
    }
}
