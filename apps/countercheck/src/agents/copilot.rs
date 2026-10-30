//! GitHub Copilot (the CLI, the cloud agent, and VS Code's Copilot agent, which share one hook
//! format): `sessionStart` and `agentStop` in our own `.github/hooks/countercheck.json`.
//!
//! The answer to a stop is Claude Code's (`{"decision":"block","reason":...}`), but the input isn't:
//! `agentStop` sends camelCase fields (`sessionId`, `transcriptPath`) and no final message, and the
//! transcript is Copilot's own event log (`events.jsonl`). So Copilot has its own stop hook, which
//! only translates; the decision is `hook::check`'s.

use super::{Entry, Target};
use crate::hook::{self, Decision, HookInput};
use countercheck_core::receipt::{MAX_MODEL_CHARS, clean_label};
use std::path::PathBuf;

/// `<bin> hook <sub>` for PowerShell: a quoted path needs the call operator, or it is just a string.
fn powershell(bin: &str, sub: &str) -> String {
    let bin = if bin.contains(char::is_whitespace) && !bin.starts_with('"') {
        format!("& \"{bin}\"")
    } else if bin.starts_with('"') {
        format!("& {bin}")
    } else {
        bin.to_string()
    };
    super::quoted(&format!("{bin} hook {sub}"))
}

/// One command entry, `bash` and `powershell` (Windows; the cloud agent runs `bash` only), with a
/// timeout in seconds (Copilot's default is 30).
fn entry(bin: &str, sub: &str, timeout: u32) -> String {
    format!(
        r#"{{"type": "command", "bash": {}, "powershell": {}, "timeoutSec": {timeout}}}"#,
        super::command(bin, sub),
        powershell(bin, sub)
    )
}

fn entries(bin: &str) -> Vec<Entry> {
    vec![
        Entry {
            path: vec!["hooks", "sessionStart"],
            value: entry(bin, "session-start", 10),
            needles: vec!["hook session-start"],
        },
        Entry {
            path: vec!["hooks", "agentStop"],
            value: entry(bin, "copilot-stop", 600),
            needles: vec!["hook copilot-stop"],
        },
    ]
}

pub fn target() -> Target {
    Target {
        agent: "copilot",
        label: "GitHub Copilot",
        project_file: ".github/hooks/countercheck.json",
        user_file: Some(".copilot/hooks/countercheck.json"),
        top: vec![("version", serde_json::Value::from(1))],
        entries,
        // The CLI also runs the hooks in Claude Code's project settings.
        also_reads: &[".claude/settings.json", ".claude/settings.local.json"],
        after: "Copilot loads hooks when a session starts: start a new session to use them. The cloud agent \
                reads .github/hooks from the default branch and needs countercheck installed by \
                copilot-setup-steps.yml (see the docs page).",
        docs: "docs/hooks/copilot.md",
    }
}

/// Copilot `agentStop`: stdin is `{"sessionId","cwd","transcriptPath","stopReason",...}` (or the
/// snake_case form a `Stop` entry gets); stdout blocks with `decision`/`reason` (and the same in
/// `hookSpecificOutput`, VS Code's form), or lets it stop with one `systemMessage` line.
pub fn stop() -> Result<(), String> {
    let Some(payload) = hook::read_payload()? else {
        return Ok(());
    };
    let text = |keys: &[&str]| {
        keys.iter()
            .find_map(|k| payload.get(*k).and_then(|v| v.as_str()))
            .filter(|s| !s.is_empty())
    };
    let transcript = text(&["transcriptPath", "transcript_path"]);
    let last = transcript.and_then(last_message);
    let session = text(&["sessionId", "session_id"]).unwrap_or("").to_string();
    let input = HookInput {
        cwd: text(&["cwd"])
            .map(PathBuf::from)
            .or_else(|| std::env::current_dir().ok())
            .ok_or("no working directory in hook input")?,
        // A `Stop` entry may give the final message directly.
        claim: text(&["last_assistant_message"])
            .map(str::to_string)
            .or_else(|| last.as_ref().map(|(t, _)| t.clone())),
        agent: "copilot",
        model: last.and_then(|(_, m)| m),
        session: session.clone(),
    };
    let (receipt, path, decision) = hook::check(input)?;
    let gave_up = match &decision {
        Decision::Continue(_) => !hook::take_nudge(&path, &session),
        _ => false,
    };
    hook::log_call(&session, &receipt, &path, &decision, Some(gave_up));
    let out = match decision {
        Decision::Continue(reason) if !gave_up => serde_json::json!({
            "decision": "block",
            "reason": reason,
            "hookSpecificOutput": { "hookEventName": "Stop", "decision": "block", "reason": reason },
        }),
        d => serde_json::json!({ "systemMessage": hook::stop_message(&receipt, &d, gave_up) }),
    };
    println!("{out}");
    Ok(())
}

/// The main agent's last text in Copilot's `events.jsonl` (`{"type":"assistant.message","data":
/// {"content":...,"model":...}}`; a sub-agent's carry `agentId`), and its model. Any problem means
/// no claim, never an error.
pub(crate) fn last_message(path: &str) -> Option<(String, Option<String>)> {
    let (text, _) = hook::read_tail(&hook::home_path(path))?;
    text.lines().rev().find_map(|line| {
        let v: serde_json::Value = serde_json::from_str(line).ok()?;
        if v.get("type")?.as_str()? != "assistant.message" || v.get("agentId").is_some() {
            return None;
        }
        let data = v.get("data")?;
        let content = data.get("content")?.as_str()?;
        if content.trim().is_empty() {
            return None;
        }
        let model = data
            .get("model")
            .and_then(|m| m.as_str())
            .and_then(|m| clean_label(m, MAX_MODEL_CHARS));
        Some((content.to_string(), model))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn powershell_calls_a_quoted_path() {
        assert_eq!(
            powershell("countercheck", "copilot-stop"),
            "\"countercheck hook copilot-stop\""
        );
        assert_eq!(
            powershell("C:/Program Files/cc/countercheck.exe", "copilot-stop"),
            r#""& \"C:/Program Files/cc/countercheck.exe\" hook copilot-stop""#
        );
    }

    #[test]
    fn the_main_agents_last_text_is_the_claim() {
        let d = tempfile::tempdir().unwrap();
        let f = d.path().join("events.jsonl");
        std::fs::write(
            &f,
            [
                r#"{"type":"session.start","data":{"sessionId":"s"}}"#,
                r#"{"type":"assistant.message","data":{"messageId":"1","content":"All tests pass.","model":"gpt-x"}}"#,
                r#"{"type":"assistant.message","data":{"messageId":"2","content":"","toolRequests":[]}}"#,
                r#"{"type":"assistant.message","agentId":"sub","data":{"messageId":"3","content":"sub done"}}"#,
                "not json",
            ]
            .join("\n"),
        )
        .unwrap();
        let (text, model) = last_message(f.to_str().unwrap()).unwrap();
        assert_eq!(text, "All tests pass.");
        assert_eq!(model.as_deref(), Some("gpt-x"));
        assert!(last_message(d.path().join("none").to_str().unwrap()).is_none());
    }
}
