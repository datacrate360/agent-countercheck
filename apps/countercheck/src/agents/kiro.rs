//! Kiro (IDE 1.0+ and CLI V3): standalone hook files in `.kiro/hooks/*.json` (and, for CLI V3,
//! `~/.kiro/hooks/`). Countercheck writes its own file there, so a user's hooks are never edited.
//!
//! Kiro's `Stop` input has `cwd` and `session_id` but no transcript, and a hook sends the agent
//! back with `{"decision":"block","reason":...}` (exit 0); exit 1 also starts another turn, so the
//! stop hook must only ever exit 0. Docs: <https://kiro.dev/docs/hooks.md>,
//! <https://kiro.dev/docs/hooks/types.md#agent-stop>.

use super::{Entry, Target};
use crate::hook::{self, Decision, HookInput};
use std::path::PathBuf;

/// `SessionStart` (locks the checks to where the session began) and `Stop` (re-runs the checks),
/// as hook definitions in a `"version": "v1"` file, keys in the order Kiro's docs show them. Stop
/// hooks take no matcher.
fn entries(bin: &str) -> Vec<Entry> {
    vec![
        Entry {
            path: vec!["hooks"],
            value: format!(
                r#"{{"name": "countercheck-session-start", "trigger": "SessionStart", "action": {{"type": "command", "command": {}}}, "timeout": 10}}"#,
                super::command(bin, "session-start")
            ),
            needles: vec!["hook session-start"],
        },
        Entry {
            path: vec!["hooks"],
            value: format!(
                r#"{{"name": "countercheck-stop", "trigger": "Stop", "action": {{"type": "command", "command": {}}}, "timeout": 600}}"#,
                super::command(bin, "kiro-stop")
            ),
            needles: vec!["hook kiro-stop"],
        },
    ]
}

pub fn target() -> Target {
    Target {
        agent: "kiro",
        label: "Kiro",
        project_file: ".kiro/hooks/countercheck.json",
        // Read by Kiro CLI V3 only; the IDE reads the project folder.
        user_file: Some(".kiro/hooks/countercheck.json"),
        top: vec![("version", serde_json::Value::String("v1".to_string()))],
        entries,
        also_reads: &[],
        after: "Kiro loads hooks when a session starts: start a new session. Kiro CLI asks once to trust the \
                workspace before it loads its hooks; in the IDE, check that both Countercheck hooks are enabled \
                (eye icon) in the Agent Hooks panel.",
        docs: "docs/hooks/kiro.md",
    }
}

/// Kiro `Stop`: stdin is `{"hook_event_name":"Stop","cwd":...,"session_id":...}`; stdout is
/// `{"decision":"block","reason":...}` to send the agent back, otherwise one plain line saying what
/// Countercheck found. Kiro sends no transcript; `assistant_response` (the final answer, named in the
/// CLI's own hook help but not in the public docs) is read when present, so an agent that reports
/// a blocker may stop. `COUNTERCHECK_MAX_NUDGES` (default 3) caps the push-backs per session.
pub fn stop() -> Result<(), String> {
    let Some(payload) = hook::read_payload()? else {
        return Ok(());
    };
    let text = |key: &str| payload.get(key).and_then(|v| v.as_str());
    let cwd = text("cwd")
        .filter(|c| !c.is_empty())
        .map(PathBuf::from)
        .or_else(|| std::env::current_dir().ok())
        .ok_or("no working directory in hook input")?;
    let session = text("session_id").unwrap_or("").to_string();
    let input = HookInput {
        model: text("model")
            .map(str::to_string)
            .or_else(|| hook::recorded_model(&cwd, &session)),
        cwd,
        claim: text("assistant_response").map(str::to_string),
        agent: "kiro",
        session: session.clone(),
    };
    let (receipt, path, decision) = hook::check(input)?;
    let gave_up = match &decision {
        Decision::Continue(_) => !hook::take_nudge(&path, &session),
        _ => false,
    };
    hook::log_call(&session, &receipt, &path, &decision, Some(gave_up));
    match decision {
        Decision::Continue(reason) if !gave_up => {
            println!(
                "{}",
                serde_json::json!({ "decision": "block", "reason": reason })
            );
        }
        d => println!("{}", hook::stop_message(&receipt, &d, gave_up)),
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agents::{add, json, remove};

    #[test]
    fn a_new_file_gets_the_version_and_both_hooks_and_loses_only_them() {
        let t = target();
        let mut n = json::parse("{}").unwrap();
        assert_eq!(add(&t, &mut n, "countercheck").unwrap(), 2);
        assert_eq!(add(&t, &mut n, "countercheck").unwrap(), 0);
        let v: serde_json::Value = serde_json::from_str(&json::pretty(&n)).unwrap();
        assert_eq!(v["version"], "v1");
        assert_eq!(v["hooks"][0]["trigger"], "SessionStart");
        assert_eq!(
            v["hooks"][1]["action"]["command"],
            "countercheck hook kiro-stop"
        );
        assert!(v["hooks"][1].get("matcher").is_none());
        assert_eq!(remove(&t, &mut n, "countercheck"), 2);
        assert_eq!(json::pretty(&n).trim(), "{\n  \"version\": \"v1\"\n}");
    }

    #[test]
    fn an_existing_version_is_kept() {
        let t = target();
        let mut n = json::parse("{\"version\": \"v2\", \"hooks\": []}").unwrap();
        add(&t, &mut n, "countercheck").unwrap();
        assert!(json::pretty(&n).contains("\"version\": \"v2\""));
    }
}
