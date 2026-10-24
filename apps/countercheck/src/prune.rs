//! `countercheck prune`: delete receipts older than a given age, on request only.
//!
//! Receipts are the audit trail, so nothing deletes them on its own; this is for getting the
//! space back. Age comes from the receipt's own `created_at` (not the file time, which a copy or
//! checkout changes). Files that don't read as receipts are never touched, nor is anything
//! outside the receipt folder (no subfolders, no symlinks).

use crate::report::{Located, locate};
use countercheck_core::receipt::Receipt;
use std::path::PathBuf;
use std::time::{Duration, SystemTime};

pub struct Options {
    pub repo: PathBuf,
    pub receipts: Option<PathBuf>,
    /// e.g. `90d`, `12weeks`, `6months`.
    pub older_than: String,
    pub dry_run: bool,
}

/// Receipts read larger than this are left alone (same cap as the report).
const MAX_BYTES: u64 = 8 * 1024 * 1024;

pub struct Pruned {
    pub removed: usize,
    pub bytes: u64,
    pub kept: usize,
}

pub fn run(opts: &Options) -> Result<Pruned, String> {
    let age: Duration = humantime::parse_duration(&opts.older_than).map_err(|e| {
        format!(
            "--older-than {:?}: {e} (examples: 90d, 12weeks, 6months)",
            opts.older_than
        )
    })?;
    let cutoff = SystemTime::now()
        .checked_sub(age)
        .ok_or("--older-than: that is before the start of time")?;
    let Located { dir, .. } = locate(&opts.repo, opts.receipts.as_deref())?;
    let rd = std::fs::read_dir(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let mut out = Pruned {
        removed: 0,
        bytes: 0,
        kept: 0,
    };
    let mut old: Vec<(String, PathBuf, u64)> = Vec::new();
    for e in rd.flatten() {
        let path = e.path();
        if path.extension().is_none_or(|x| x != "json") {
            continue;
        }
        let Ok(meta) = std::fs::symlink_metadata(&path) else {
            continue;
        };
        if !meta.is_file() || meta.len() > MAX_BYTES {
            continue;
        }
        let Some(r) = std::fs::read_to_string(&path)
            .ok()
            .and_then(|t| serde_json::from_str::<Receipt>(&t).ok())
        else {
            continue; // not a receipt: never ours to delete
        };
        let created = humantime::parse_rfc3339_weak(&r.created_at).ok();
        if created.is_some_and(|t| t < cutoff) {
            old.push((
                format!("{} {} {}", r.created_at, r.verdict.label(), r.id),
                path,
                meta.len(),
            ));
        } else {
            out.kept += 1;
        }
    }
    old.sort();
    let verb = if opts.dry_run {
        "would remove"
    } else {
        "removed"
    };
    for (label, path, len) in &old {
        if !opts.dry_run {
            std::fs::remove_file(path).map_err(|e| format!("{}: {e}", path.display()))?;
        }
        out.removed += 1;
        out.bytes += len;
        println!("{verb} {label}");
    }
    Ok(out)
}
