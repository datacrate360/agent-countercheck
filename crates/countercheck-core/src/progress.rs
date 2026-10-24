//! A run in progress: what `countercheck verify` is doing right now, for the report and live UI.
//!
//! `verify` writes one small file per run to `<receipts>/running/<id>.json` while it works and
//! removes it when the receipt is written. It is for display only: nothing reads it to decide a
//! verdict, and the page labels it "in progress, not a receipt". The worker can write such files
//! too (the folder is in its tree), so readers treat every field as hostile: size-capped, labels
//! cleaned, and a run past its `stale_after_unix` time is ignored (a killed run leaves its file).

use crate::receipt::{MAX_MODEL_CHARS, MAX_ROLE_CHARS, Worker, clean_label};
use serde::{Deserialize, Serialize};

pub const SCHEMA: &str = "countercheck.progress/v1";
/// Folder inside the receipts folder.
pub const DIR: &str = "running";
/// Larger progress files are ignored.
pub const MAX_BYTES: u64 = 64 * 1024;
/// At most this many runs are shown.
pub const MAX_RUNNING: usize = 24;
/// Longest claim shown (characters).
const MAX_CLAIM_CHARS: usize = 300;
/// Longest check name shown (characters).
const MAX_NAME_CHARS: usize = 120;

/// Where the run is. The order is the order of the steps a finished run's replay shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Stage {
    /// Reading the checks (from the base revision when locked).
    Lock,
    /// Copying the worker's files into a clean folder.
    Copy,
    /// Comparing with the base; putting protected files back.
    Protect,
    /// Running the checks.
    Check,
    /// Deciding and sealing the receipt.
    Seal,
}

impl Stage {
    /// 1-based step number in the replay (step 1 is the agent's claim).
    pub fn step(self) -> usize {
        match self {
            Stage::Lock => 2,
            Stage::Copy => 3,
            Stage::Protect => 4,
            Stage::Check => 5,
            Stage::Seal => 6,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Stage::Lock => "locking the checks",
            Stage::Copy => "making a clean copy",
            Stage::Protect => "putting protected files back",
            Stage::Check => "running the checks",
            Stage::Seal => "sealing the receipt",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Progress {
    pub schema: String,
    pub id: String,
    /// RFC 3339, like a receipt's `created_at`.
    pub started_at: String,
    pub updated_unix: u64,
    /// After this the run is taken as dead (killed before it could clean up).
    pub stale_after_unix: u64,
    pub worker: Worker,
    #[serde(default)]
    pub claim: Option<String>,
    pub stage: Stage,
    #[serde(default)]
    pub checks_total: usize,
    #[serde(default)]
    pub checks_done: usize,
    /// The check running now.
    #[serde(default)]
    pub current: Option<String>,
}

impl Progress {
    /// Still running at `now_unix` by its own word.
    pub fn is_live(&self, now_unix: u64) -> bool {
        self.schema == SCHEMA && now_unix <= self.stale_after_unix
    }

    /// Make a progress file read back from disk safe to show: labels cleaned and capped, counts
    /// kept consistent. Returns `None` when it isn't a usable progress record.
    pub fn cleaned(mut self) -> Option<Self> {
        if self.schema != SCHEMA {
            return None;
        }
        self.id = self
            .id
            .chars()
            .filter(char::is_ascii_alphanumeric)
            .take(32)
            .collect();
        if self.id.is_empty() {
            return None;
        }
        let w = &mut self.worker;
        w.agent = w.agent.as_deref().and_then(|s| clean_label(s, 60));
        w.model = w
            .model
            .as_deref()
            .and_then(|s| clean_label(s, MAX_MODEL_CHARS));
        w.role = w
            .role
            .as_deref()
            .and_then(|s| clean_label(s, MAX_ROLE_CHARS));
        w.session = None;
        w.cost_usd = None;
        self.claim = self
            .claim
            .as_deref()
            .and_then(|s| clean_label(s, MAX_CLAIM_CHARS));
        self.current = self
            .current
            .as_deref()
            .and_then(|s| clean_label(s, MAX_NAME_CHARS));
        self.started_at = self
            .started_at
            .chars()
            .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | ':' | '.' | '+'))
            .take(40)
            .collect();
        self.checks_total = self.checks_total.min(100_000);
        self.checks_done = self.checks_done.min(self.checks_total);
        Some(self)
    }
}

/// Parse and clean one progress file; anything malformed, foreign or stale is `None`.
pub fn read(text: &str, now_unix: u64) -> Option<Progress> {
    let p: Progress = serde_json::from_str(text).ok()?;
    let p = p.cleaned()?;
    p.is_live(now_unix).then_some(p)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Progress {
        Progress {
            schema: SCHEMA.into(),
            id: "abc123".into(),
            started_at: "2026-10-23T10:00:00Z".into(),
            updated_unix: 100,
            stale_after_unix: 200,
            worker: Worker {
                agent: Some("claude-code".into()),
                model: None,
                cost_usd: Some(1.0),
                role: Some("  UI\n design ".into()),
                session: Some("s".into()),
            },
            claim: Some("Done.".into()),
            stage: Stage::Check,
            checks_total: 3,
            checks_done: 9,
            current: Some("unit\u{1b}[31m".into()),
        }
    }

    #[test]
    fn a_live_run_is_read_back_cleaned() {
        let text = serde_json::to_string(&sample()).unwrap_or_default();
        let p = read(&text, 150).expect("live");
        assert_eq!(p.worker.role.as_deref(), Some("UI design"));
        assert_eq!(p.worker.session, None);
        assert_eq!(p.worker.cost_usd, None);
        assert_eq!(p.checks_done, 3, "done can't exceed total");
        assert_eq!(p.current.as_deref(), Some("unit [31m"));
    }

    #[test]
    fn stale_foreign_or_broken_files_are_ignored() {
        let text = serde_json::to_string(&sample()).unwrap_or_default();
        assert!(read(&text, 201).is_none(), "past stale_after");
        let mut other = sample();
        other.schema = "x".into();
        assert!(read(&serde_json::to_string(&other).unwrap_or_default(), 150).is_none());
        let mut no_id = sample();
        no_id.id = "../..".into();
        assert!(read(&serde_json::to_string(&no_id).unwrap_or_default(), 150).is_none());
        assert!(read("{not json", 150).is_none());
    }

    #[test]
    fn stages_follow_the_replay_steps() {
        let steps: Vec<usize> = [
            Stage::Lock,
            Stage::Copy,
            Stage::Protect,
            Stage::Check,
            Stage::Seal,
        ]
        .iter()
        .map(|s| s.step())
        .collect();
        assert_eq!(steps, vec![2, 3, 4, 5, 6]);
    }
}
