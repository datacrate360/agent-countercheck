//! Sessions that started but have no receipt: "Not checked".
//!
//! `countercheck hook session-start` records every agent session that starts in a project. The stop
//! hook then writes a receipt each time the agent finishes. A session with a start record and no
//! receipt was never checked: its hook was switched off or removed, it timed out, or the session
//! is still on its first task. Without this list a project page with no FAILED looks clean even
//! when nothing was checked at all.
//!
//! Display only: a start record is a hostile file in the git folder, so it never changes a verdict.
//! It only adds a line to the page.

use crate::receipt::{MAX_MODEL_CHARS, MAX_SESSION_CHARS, clean_label};
use std::collections::BTreeSet;

/// Records larger than this are skipped (a real one is about 200 bytes).
pub const MAX_BYTES: u64 = 4096;
/// Most start records read from the folder.
pub const MAX_RECORDS: usize = 500;
/// Most sessions listed on the page (newest first).
pub const MAX_SHOWN: usize = 20;

/// One session's start, as recorded by the hook.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionStart {
    /// The session id, cleaned like a receipt's `worker.session`, so the two compare equal.
    pub id: String,
    pub started_unix: u64,
    pub agent: Option<String>,
    pub model: Option<String>,
}

/// Agent labels are short words (`claude-code`, `cortex-code`, `hermes`).
const MAX_AGENT_CHARS: usize = 40;

/// Read one start record; `None` when it isn't one (or is damaged).
pub fn parse(text: &str) -> Option<SessionStart> {
    let v: serde_json::Value = serde_json::from_str(text).ok()?;
    let s = |k: &str| v.get(k).and_then(|x| x.as_str());
    let started_unix = v.get("recorded_at_unix")?.as_u64().filter(|t| *t > 0)?;
    Some(SessionStart {
        id: clean_label(s("session_id")?, MAX_SESSION_CHARS)?,
        started_unix,
        agent: s("agent").and_then(|a| clean_label(a, MAX_AGENT_CHARS)),
        model: s("model").and_then(|m| clean_label(m, MAX_MODEL_CHARS)),
    })
}

/// `unix` seconds as RFC 3339 UTC (`2026-10-29T22:05:00Z`), like a receipt's `created_at`.
pub fn rfc3339(unix: u64) -> String {
    let (days, secs) = (unix / 86_400, unix % 86_400);
    // Civil date from days since 1970-01-01 (H. Hinnant's algorithm), in unsigned arithmetic.
    let z = days + 719_468;
    let era = z / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + u64::from(m <= 2);
    format!(
        "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z",
        secs / 3600,
        secs % 3600 / 60,
        secs % 60
    )
}

/// The sessions with no receipt and no run in progress, newest first, at most [`MAX_SHOWN`].
/// `checked` holds every session id found on a receipt; `running`, those of runs in progress.
/// Sessions that started less than `grace_secs` before `now_unix` are left out: their agent may
/// still be on its first task.
pub fn unchecked(
    starts: &[SessionStart],
    checked: &BTreeSet<String>,
    running: &BTreeSet<String>,
    now_unix: u64,
    grace_secs: u64,
) -> Vec<SessionStart> {
    let mut seen = BTreeSet::new();
    let mut out: Vec<SessionStart> = starts
        .iter()
        .filter(|s| !checked.contains(&s.id) && !running.contains(&s.id))
        .filter(|s| now_unix.saturating_sub(s.started_unix) >= grace_secs)
        .filter(|s| seen.insert(s.id.clone()))
        .cloned()
        .collect();
    out.sort_by(|a, b| b.started_unix.cmp(&a.started_unix).then(a.id.cmp(&b.id)));
    out.truncate(MAX_SHOWN);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn start(id: &str, t: u64) -> SessionStart {
        SessionStart {
            id: id.into(),
            started_unix: t,
            agent: Some("claude-code".into()),
            model: None,
        }
    }

    #[test]
    fn parse_reads_a_real_record_and_refuses_junk() {
        let r = parse(r#"{"session_id":"abc-1","base":"0123","recorded_at_unix":1761700000,"model":"claude-opus-5-5","agent":"claude-code"}"#).unwrap();
        assert_eq!(r.id, "abc-1");
        assert_eq!(r.agent.as_deref(), Some("claude-code"));
        assert_eq!(r.model.as_deref(), Some("claude-opus-5-5"));
        // Records written before the agent was recorded.
        assert_eq!(
            parse(r#"{"session_id":"x","recorded_at_unix":5,"model":null}"#)
                .unwrap()
                .agent,
            None
        );
        for junk in [
            "not json",
            "[]",
            r#"{"session_id":"x"}"#,
            r#"{"recorded_at_unix":5}"#,
            r#"{"session_id":"","recorded_at_unix":5}"#,
            r#"{"session_id":"x","recorded_at_unix":0}"#,
            r#"{"session_id":"x","recorded_at_unix":"5"}"#,
        ] {
            assert!(parse(junk).is_none(), "{junk}");
        }
    }

    #[test]
    fn only_sessions_with_no_receipt_and_no_run_count() {
        let starts = [
            start("a", 100),
            start("b", 200),
            start("c", 300),
            start("d", 950),
        ];
        let checked: BTreeSet<String> = ["a".to_string()].into();
        let running: BTreeSet<String> = ["b".to_string()].into();
        let out = unchecked(&starts, &checked, &running, 1000, 600);
        // a has a receipt, b is running, d started 50 s ago (grace).
        assert_eq!(out.iter().map(|s| s.id.as_str()).collect::<Vec<_>>(), ["c"]);
    }

    #[test]
    fn rfc3339_matches_known_dates() {
        assert_eq!(rfc3339(0), "1970-01-01T00:00:00Z");
        assert_eq!(rfc3339(951_782_400), "2000-02-29T00:00:00Z");
        assert_eq!(rfc3339(1_793_311_500), "2026-10-29T22:05:00Z");
        assert_eq!(rfc3339(4_107_542_399), "2100-02-28T23:59:59Z");
    }

    #[test]
    fn newest_first_capped_and_deduplicated() {
        let mut starts: Vec<SessionStart> =
            (0..30).map(|i| start(&format!("s{i}"), 10 + i)).collect();
        starts.push(start("s29", 10));
        let out = unchecked(&starts, &BTreeSet::new(), &BTreeSet::new(), 1000, 0);
        assert_eq!(out.len(), MAX_SHOWN);
        assert_eq!(out[0].id, "s29");
        assert_eq!(out.iter().filter(|s| s.id == "s29").count(), 1);
    }
}
