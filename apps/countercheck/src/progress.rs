//! The in-progress file `verify` keeps while it works (`<receipts>/running/<id>.json`), for the
//! report and live UI. Display only: nothing decides a verdict from it. Writing is best effort (a
//! read-only folder just means no progress is shown), each write replaces the whole file through a
//! rename so a reader never sees half of it, and the file is removed when the run ends, however
//! it ends.

use countercheck_core::progress::{self, Progress, Stage};
use countercheck_core::receipt::{Worker, clean_label};
use std::path::{Path, PathBuf};

/// How long a stage may take before readers drop the run as dead (a killed run leaves its file).
const STAGE_SECS: u64 = 600;
/// Extra time on top of a check's own timeout.
const SLACK_SECS: u64 = 120;

pub struct Reporter {
    path: Option<PathBuf>,
    p: Progress,
}

fn now_unix() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

impl Reporter {
    /// Start reporting into `receipts/running/`. Never fails: without a folder or randomness the
    /// run simply isn't shown as in progress.
    pub fn start(receipts: &Path, worker: &Worker, claim: Option<&str>) -> Self {
        let mut id = [0u8; 8];
        let id = getrandom::fill(&mut id)
            .ok()
            .map(|()| id.iter().map(|b| format!("{b:02x}")).collect::<String>());
        let dir = receipts.join(progress::DIR);
        let path = id
            .as_ref()
            .filter(|_| std::fs::create_dir_all(&dir).is_ok())
            .map(|id| dir.join(format!("{id}.json")));
        let now = now_unix();
        let mut w = worker.clone();
        w.session = None;
        w.cost_usd = None;
        let r = Reporter {
            path,
            p: Progress {
                schema: progress::SCHEMA.to_string(),
                id: id.unwrap_or_default(),
                started_at: humantime::format_rfc3339_seconds(std::time::SystemTime::now())
                    .to_string(),
                updated_unix: now,
                stale_after_unix: now + STAGE_SECS,
                worker: w,
                claim: claim.and_then(|c| clean_label(c, 300)),
                stage: Stage::Lock,
                checks_total: 0,
                checks_done: 0,
                current: None,
            },
        };
        r.write();
        r
    }

    pub fn stage(&mut self, stage: Stage) {
        self.p.stage = stage;
        self.p.current = None;
        self.touch(STAGE_SECS);
    }

    /// Check `done + 1` of `total` starts: `name`, allowed `timeout_secs`.
    pub fn check(&mut self, done: usize, total: usize, name: &str, timeout_secs: u64) {
        self.p.stage = Stage::Check;
        self.p.checks_total = total;
        self.p.checks_done = done;
        self.p.current = clean_label(name, 120);
        self.touch(timeout_secs.saturating_add(SLACK_SECS));
    }

    fn touch(&mut self, secs: u64) {
        let now = now_unix();
        self.p.updated_unix = now;
        self.p.stale_after_unix = now.saturating_add(secs);
        self.write();
    }

    fn write(&self) {
        let Some(path) = &self.path else { return };
        let Ok(text) = serde_json::to_string(&self.p) else {
            return;
        };
        let tmp = path.with_extension("tmp");
        if std::fs::write(&tmp, text).is_ok() && std::fs::rename(&tmp, path).is_err() {
            let _ = std::fs::remove_file(&tmp);
        }
    }
}

impl Drop for Reporter {
    fn drop(&mut self) {
        if let Some(path) = &self.path {
            let _ = std::fs::remove_file(path);
            let _ = std::fs::remove_file(path.with_extension("tmp"));
        }
    }
}

/// Remove what dead runs left in `receipts/running/`: progress files past their `stale_after` time
/// (a killed run can't remove its own), files that aren't progress records, and half-written
/// `.tmp` files. Only files untouched for a minute go, so a run that is starting is never raced.
/// Live runs are kept. Best effort; never fails.
pub fn sweep(receipts: &Path) {
    const QUIET: std::time::Duration = std::time::Duration::from_secs(60);
    let Ok(rd) = std::fs::read_dir(receipts.join(progress::DIR)) else {
        return;
    };
    let now = now_unix();
    for e in rd.flatten() {
        let path = e.path();
        let Ok(meta) = std::fs::symlink_metadata(&path) else {
            continue;
        };
        let quiet = meta
            .modified()
            .ok()
            .and_then(|t| t.elapsed().ok())
            .is_some_and(|age| age > QUIET);
        if !meta.is_file() || !quiet {
            continue;
        }
        let dead = match path.extension().and_then(|x| x.to_str()) {
            Some("tmp") => true,
            Some("json") => {
                meta.len() > progress::MAX_BYTES
                    || std::fs::read_to_string(&path)
                        .ok()
                        .and_then(|t| progress::read(&t, now))
                        .is_none()
            }
            _ => false,
        };
        if dead {
            let _ = std::fs::remove_file(&path);
        }
    }
}

/// Runs in progress in `receipts/running/`, newest first, read as hostile input: size-capped,
/// cleaned, stale ones skipped. Never fails.
pub fn read_running(receipts: &Path) -> Vec<Progress> {
    let Ok(rd) = std::fs::read_dir(receipts.join(progress::DIR)) else {
        return Vec::new();
    };
    let now = now_unix();
    let mut out: Vec<Progress> = rd
        .flatten()
        .filter(|e| e.path().extension().is_some_and(|x| x == "json"))
        .filter(|e| {
            e.metadata()
                .is_ok_and(|m| m.is_file() && m.len() <= progress::MAX_BYTES)
        })
        .filter_map(|e| std::fs::read_to_string(e.path()).ok())
        .filter_map(|t| progress::read(&t, now))
        .collect();
    out.sort_by(|a, b| b.started_at.cmp(&a.started_at));
    out.truncate(progress::MAX_RUNNING);
    out
}
