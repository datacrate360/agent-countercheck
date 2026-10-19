//! Scans over the worker's files in the clean copy (`[[forbid]]`, `[[coverage]]`).

use countercheck_core::coverage::{Coverage, CoverageSpec};
use countercheck_core::forbid::{self, ForbidSpec, Rule, Scan};
use countercheck_core::receipt::CheckResult;
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use std::time::Instant;

/// Every regular file in the clean copy, as sorted relative paths. Symlinks (including linked
/// dependency folders) are never followed.
pub fn files(work: &Path) -> Result<Vec<String>, String> {
    let mut out = Vec::new();
    walk(work, work, &mut out)?;
    out.sort();
    Ok(out)
}

fn walk(base: &Path, dir: &Path, out: &mut Vec<String>) -> Result<(), String> {
    let rd = fs::read_dir(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    for entry in rd {
        let entry = entry.map_err(|e| e.to_string())?;
        let ft = entry.file_type().map_err(|e| e.to_string())?;
        let path = entry.path();
        if ft.is_dir() {
            walk(base, &path, out)?;
        } else if ft.is_file()
            && let Ok(rel) = path.strip_prefix(base)
        {
            out.push(rel.to_string_lossy().into_owned());
        }
    }
    Ok(())
}

/// Run every `[[forbid]]` entry over the clean copy. Each file is read at most once.
pub fn forbid_all(
    specs: &[ForbidSpec],
    work: &Path,
    files: &[String],
) -> Result<Vec<CheckResult>, String> {
    let started = Instant::now();
    let rules = specs.iter().map(Rule::new).collect::<Result<Vec<_>, _>>()?;
    let mut scans: Vec<Scan> = specs.iter().map(|_| Scan::default()).collect();
    for f in files {
        let wanted: Vec<usize> = (0..rules.len())
            .filter(|&i| rules.get(i).is_some_and(|r| r.wants(f)))
            .collect();
        if wanted.is_empty() {
            continue;
        }
        let path = work.join(f);
        let too_big = fs::metadata(&path).map_or(true, |m| m.len() > forbid::MAX_FILE_BYTES);
        let bytes = if too_big { None } else { fs::read(&path).ok() };
        for i in wanted {
            let (Some(rule), Some(scan)) = (rules.get(i), scans.get_mut(i)) else {
                continue;
            };
            match &bytes {
                Some(b) => scan.add_file(rule, f, b),
                None => scan.skip(f),
            }
        }
    }
    let duration_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
    Ok(specs
        .iter()
        .zip(scans)
        .map(|(spec, scan)| {
            let (passed, output) = scan.report(spec);
            CheckResult {
                name: spec.result_name(),
                run: spec.describe(),
                exit_code: Some(if passed { 0 } else { 1 }),
                timed_out: false,
                duration_ms,
                passed,
                output_sha256: countercheck_core::sha256_hex(output.as_bytes()),
                output_tail: output,
            }
        })
        .collect())
}

/// Run every `[[coverage]]` entry over the clean copy. `lists` holds each item list's text.
pub fn coverage_all(
    specs: &[CoverageSpec],
    lists: &BTreeMap<String, String>,
    work: &Path,
    files: &[String],
) -> Result<Vec<CheckResult>, String> {
    let mut out = Vec::new();
    for spec in specs {
        let started = Instant::now();
        let text = lists
            .get(&spec.items)
            .ok_or_else(|| format!("coverage \"{}\": {} was not found", spec.name, spec.items))?;
        let mut cov = Coverage::new(spec, text)?;
        for f in files {
            if !cov.wants(f) {
                continue;
            }
            let path = work.join(f);
            let too_big = fs::metadata(&path).map_or(true, |m| m.len() > forbid::MAX_FILE_BYTES);
            match if too_big { None } else { fs::read(&path).ok() } {
                Some(bytes) => cov.add_file(f, &bytes),
                None => cov.skip(f),
            }
        }
        let (passed, output) = cov.report();
        out.push(CheckResult {
            name: spec.result_name(),
            run: spec.describe(),
            exit_code: Some(if passed { 0 } else { 1 }),
            timed_out: false,
            duration_ms: u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
            passed,
            output_sha256: countercheck_core::sha256_hex(output.as_bytes()),
            output_tail: output,
        });
    }
    Ok(out)
}
