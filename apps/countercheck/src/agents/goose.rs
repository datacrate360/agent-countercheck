//! Goose (aaif-goose/goose): hooks come from plugins, so Countercheck is its own plugin,
//! `.agents/plugins/countercheck/` (project) or `~/.agents/plugins/countercheck/` (user), with only a
//! `hooks/hooks.json`. Its Stop input names the folder `working_dir`, and a hook allows a stop with
//! empty stdout or `{"decision":"allow"}` only: anything else (Claude Code's `systemMessage`) is a
//! hook failure in its log, so it gets its own stop hook.

use super::{Entry, Target};
use crate::hook::{self, Decision, HookInput};
use std::path::PathBuf;

/// `SessionStart` (locks the checks to where the session began) and `Stop` (re-runs the checks),
/// in the shape of Goose's `hooks/hooks.json`.
fn entries(bin: &str) -> Vec<Entry> {
    vec![
        Entry {
            path: vec!["hooks", "SessionStart"],
            value: format!(
                r#"{{"hooks": [{{"type": "command", "command": {}, "timeout": 10}}]}}"#,
                super::command(bin, "session-start")
            ),
            needles: vec!["hook session-start"],
        },
        Entry {
            path: vec!["hooks", "Stop"],
            // Goose's default is 30 seconds; a timed-out Stop hook lets the turn end.
            value: format!(
                r#"{{"hooks": [{{"type": "command", "command": {}, "timeout": 600}}]}}"#,
                super::command(bin, "goose-stop")
            ),
            needles: vec!["hook goose-stop"],
        },
    ]
}

pub fn target() -> Target {
    Target {
        agent: "goose",
        label: "Goose",
        project_file: ".agents/plugins/countercheck/hooks/hooks.json",
        // Goose keeps one plugin per name, the project's first: both scopes never run twice.
        user_file: Some(".agents/plugins/countercheck/hooks/hooks.json"),
        top: Vec::new(),
        entries,
        also_reads: &[],
        after: "Goose loads plugin hooks when it starts, and project plugins from the folder it starts in: start a new goose session from the project's root folder.",
        docs: "docs/hooks/goose.md",
    }
}

/// Goose `Stop`: stdin is `{"event":"Stop","session_id":...,"last_assistant_message":...,
/// "working_dir":...}`; stdout is `{"decision":"block","reason":...}` (the reason goes to the model)
/// or `{"decision":"allow"}`. Goose shows the user nothing for an allowed stop, so the one line
/// Claude Code users see goes to stderr (Goose's log) and to `COUNTERCHECK_HOOK_LOG`.
pub fn stop() -> Result<(), String> {
    let Some(payload) = hook::read_payload()? else {
        return Ok(());
    };
    let text = |key: &str| {
        payload
            .get(key)
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
    };
    // Older Goose versions send no folder; the hook then runs in Goose's own working directory,
    // the folder `goose session` started in.
    let cwd = text("working_dir")
        .or_else(|| text("cwd"))
        .map(PathBuf::from)
        .or_else(|| std::env::current_dir().ok())
        .ok_or("no working directory in hook input")?;
    let session = text("session_id").unwrap_or("").to_string();
    let input = HookInput {
        cwd,
        claim: text("last_assistant_message").map(str::to_string),
        agent: "goose",
        // Goose's hook input has no model.
        model: None,
        session: session.clone(),
    };
    let (receipt, path, decision) = hook::check(input)?;
    let gave_up = match &decision {
        Decision::Continue(_) => !hook::take_nudge(&path, &session),
        _ => false,
    };
    hook::log_call(&session, &receipt, &path, &decision, Some(gave_up));
    let out = match decision {
        Decision::Continue(reason) if !gave_up => {
            serde_json::json!({ "decision": "block", "reason": reason })
        }
        d => {
            eprintln!("{}", hook::stop_message(&receipt, &d, gave_up));
            serde_json::json!({ "decision": "allow" })
        }
    };
    println!("{out}");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agents::{add, json, remove};

    #[test]
    fn writes_the_documented_shape_into_an_empty_file() {
        let t = target();
        let mut n = json::Node::Obj(Vec::new());
        assert_eq!(add(&t, &mut n, "countercheck").unwrap(), 2);
        let v: serde_json::Value = serde_json::from_str(&json::pretty(&n)).unwrap();
        assert_eq!(
            v,
            serde_json::json!({"hooks": {
                "SessionStart": [{"hooks": [{"type": "command", "command": "countercheck hook session-start", "timeout": 10}]}],
                "Stop": [{"hooks": [{"type": "command", "command": "countercheck hook goose-stop", "timeout": 600}]}],
            }})
        );
        assert_eq!(remove(&t, &mut n, "countercheck"), 2);
        assert_eq!(json::pretty(&n).trim(), "{}");
    }

    #[test]
    fn both_scopes_name_the_same_plugin_folder() {
        let t = target();
        assert_eq!(Some(t.project_file), t.user_file);
        assert!(t.project_file.starts_with(".agents/plugins/countercheck/"));
    }
}
