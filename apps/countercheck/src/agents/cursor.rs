//! Cursor (editor agent, `cursor-agent` CLI, cloud agents): hooks in `.cursor/hooks.json`, a `stop`
//! input without a working folder or final message, and `{"followup_message": ...}` to keep going
//! (https://cursor.com/docs/hooks).

use super::{Entry, Target};
use crate::hook::{self, Decision, HookInput};
use std::path::PathBuf;

/// `sessionStart` (locks the checks to where the conversation began; it is fire-and-forget, and
/// cloud agents don't run it) and `stop` (re-runs the checks), in Cursor's flat entry shape.
fn entries(bin: &str) -> Vec<Entry> {
    vec![
        Entry {
            path: vec!["hooks", "sessionStart"],
            value: format!(
                r#"{{"command": {}, "timeout": 10}}"#,
                super::command(bin, "session-start")
            ),
            needles: vec!["hook session-start"],
        },
        Entry {
            path: vec!["hooks", "stop"],
            value: format!(
                r#"{{"command": {}, "timeout": 600}}"#,
                super::command(bin, "cursor-stop")
            ),
            needles: vec!["hook cursor-stop"],
        },
    ]
}

pub fn target() -> Target {
    Target {
        agent: "cursor",
        label: "Cursor",
        project_file: ".cursor/hooks.json",
        user_file: Some(".cursor/hooks.json"),
        top: vec![("version", serde_json::Value::from(1))],
        entries,
        // With "Include Third-Party Plugins, Skills, and Other Configs" on (the default), Cursor
        // also runs Claude Code's project hooks.
        also_reads: &[".claude/settings.json", ".claude/settings.local.json"],
        after: "Cursor reloads hooks.json when it changes: check the Hooks tab in Customize (or the Hooks output channel) that both hooks are listed.",
        docs: "docs/hooks/cursor.md",
        plugin: None,
    }
}

/// Cursor `stop`: stdin is the stop hook JSON (`conversation_id`, `status`, `loop_count`,
/// `workspace_roots`, `transcript_path`, `model`); stdout is `{"followup_message": ...}` to send
/// the agent back, else `{}`. Cursor's stop answer has no field the user sees, so the one-line
/// result goes to stderr (the Hooks output channel). Only a `completed` stop is checked: `aborted`
/// is the user pressing stop and `error` is the agent failing, neither a claim that it is done.
pub fn stop() -> Result<(), String> {
    let Some(payload) = hook::read_payload()? else {
        return Ok(());
    };
    let text = |key: &str| payload.get(key).and_then(|v| v.as_str());
    if let Some(status) = text("status").filter(|s| *s != "completed") {
        eprintln!("countercheck: Cursor stopped with status \"{status}\"; nothing checked");
        println!("{{}}");
        return Ok(());
    }
    let cwd = payload
        .get("workspace_roots")
        .and_then(|v| v.as_array())
        .and_then(|a| a.iter().find_map(|r| r.as_str()))
        .filter(|r| !r.is_empty())
        .map(PathBuf::from)
        .or_else(|| std::env::current_dir().ok())
        .ok_or("no working directory in hook input")?;
    let input = HookInput {
        cwd,
        // The stop input has no final message: the last assistant text in the transcript (null
        // when the user switched transcripts off).
        claim: text("transcript_path").and_then(last_assistant_text),
        agent: "cursor",
        model: text("model").map(str::to_string),
        session: text("conversation_id").unwrap_or("").to_string(),
    };
    let session = input.session.clone();
    let (receipt, path, decision) = hook::check(input)?;
    let gave_up = match &decision {
        Decision::Continue(_) => !hook::take_nudge(&path, &session),
        _ => false,
    };
    hook::log_call(&session, &receipt, &path, &decision, Some(gave_up));
    let out = match decision {
        Decision::Continue(message) if !gave_up => {
            serde_json::json!({ "followup_message": message })
        }
        d => {
            eprintln!("{}", hook::stop_message(&receipt, &d, gave_up));
            serde_json::json!({})
        }
    };
    println!("{out}");
    Ok(())
}

/// The last assistant text in a Cursor transcript: JSON Lines of
/// `{"role":"assistant","message":{"content":[{"type":"text","text":...}]}}`, or the plain-text
/// layout (`assistant:` starts a message) some Windows builds write. Any problem means no claim.
fn last_assistant_text(path: &str) -> Option<String> {
    let (text, _) = hook::read_tail(&hook::home_path(path))?;
    let from_json = text.lines().rev().find_map(|line| {
        let v: serde_json::Value = serde_json::from_str(line).ok()?;
        if v.get("role").and_then(|r| r.as_str()) != Some("assistant") {
            return None;
        }
        let content = v.get("message").unwrap_or(&v).get("content")?;
        let text = match content {
            serde_json::Value::String(s) => s.clone(),
            serde_json::Value::Array(blocks) => blocks
                .iter()
                .filter(|b| b.get("type").and_then(|t| t.as_str()) == Some("text"))
                .filter_map(|b| b.get("text").and_then(|t| t.as_str()))
                .collect::<Vec<_>>()
                .join("\n"),
            _ => return None,
        };
        (!text.trim().is_empty()).then_some(text)
    });
    from_json.or_else(|| {
        let (_, rest) = text
            .rsplit_once("\nassistant:")
            .or_else(|| text.strip_prefix("assistant:").map(|r| ("", r)))?;
        let msg = rest.split("\nuser:").next().unwrap_or(rest).trim();
        (!msg.is_empty()).then(|| msg.to_string())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_last_assistant_text_in_both_layouts() {
        let d = tempfile::tempdir().unwrap();
        let jsonl = d.path().join("t.jsonl");
        std::fs::write(
            &jsonl,
            "{\"role\":\"user\",\"message\":{\"content\":[{\"type\":\"text\",\"text\":\"fix it\"}]}}\n\
             {\"role\":\"assistant\",\"message\":{\"content\":[{\"type\":\"text\",\"text\":\"first\"}]}}\n\
             {\"role\":\"assistant\",\"message\":{\"content\":[{\"type\":\"tool_use\",\"name\":\"Shell\"},{\"type\":\"text\",\"text\":\"Done.\"}]}}\n",
        )
        .unwrap();
        assert_eq!(
            last_assistant_text(&jsonl.to_string_lossy()).as_deref(),
            Some("Done.")
        );
        let txt = d.path().join("t.txt");
        std::fs::write(
            &txt,
            "user:\nfix it\n\nassistant:\nI'm blocked: no network.\nuser:\nok\n",
        )
        .unwrap();
        assert_eq!(
            last_assistant_text(&txt.to_string_lossy()).as_deref(),
            Some("I'm blocked: no network.")
        );
        assert_eq!(last_assistant_text("/no/such/file"), None);
    }
}
