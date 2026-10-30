//! OpenAI Codex (CLI and app): hooks in `.codex/hooks.json`, next to each config layer.
//!
//! Codex's `Stop` input and answer use Claude Code's field names (`cwd`, `session_id`, `model`,
//! `last_assistant_message`; `{"decision":"block","reason":...}`, `systemMessage`), but every Codex
//! input carries a `transcript_path` to Codex's own session file, which `claude-stop` would take for
//! a Claude Code transcript and label the receipt `claude-code`. So Codex has its own stop hook
//! (`countercheck hook codex-stop`) that only maps the input; the decision is the shared one.

use super::{Entry, Target};
use crate::hook::{self, Decision, HookInput};
use std::path::PathBuf;

/// `SessionStart` (locks the checks to where the session began) and `Stop` (re-runs the checks),
/// in the shape of the docs' `hooks.json` example: event, matcher group, handlers.
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
            value: format!(
                r#"{{"hooks": [{{"type": "command", "command": {}, "timeout": 600}}]}}"#,
                super::command(bin, "codex-stop")
            ),
            needles: vec!["hook codex-stop"],
        },
    ]
}

pub fn target() -> Target {
    Target {
        agent: "codex",
        label: "Codex",
        project_file: ".codex/hooks.json",
        user_file: Some(".codex/hooks.json"),
        top: Vec::new(),
        entries,
        // Codex also reads `[hooks]` in config.toml in the same folder; install writes hooks.json only.
        also_reads: &[".codex/config.toml"],
        after: "Codex runs a hook only after you trust it. Start `codex` in this project: if it asks whether \
                you trust this folder, say yes (project hooks load only in a trusted folder); at \"Hooks need \
                review\" pick \"Review hooks\", check that both commands run countercheck, and trust them. Or \
                type /hooks in a session and press t to trust. Editing the hooks later asks again.",
        docs: "docs/hooks/codex.md",
    }
}

/// Codex `Stop`: stdin is the Stop hook JSON; stdout is `{"decision":"block","reason":...}` (Codex
/// continues with the reason as a new prompt) or `{"systemMessage":...}` (shown as a warning). Same
/// env as `claude-stop`, including `COUNTERCHECK_MAX_NUDGES`.
pub fn stop() -> Result<(), String> {
    let Some(payload) = hook::read_payload()? else {
        return Ok(());
    };
    let text = |key: &str| payload.get(key).and_then(|v| v.as_str());
    let input = HookInput {
        cwd: text("cwd")
            .map(PathBuf::from)
            .or_else(|| std::env::current_dir().ok())
            .ok_or("no working directory in hook input")?,
        // `null` when Codex has no final text; then there is no claim to read (the transcript is
        // Codex's own format).
        claim: text("last_assistant_message").map(str::to_string),
        agent: "codex",
        model: text("model").map(str::to_string),
        session: text("session_id").unwrap_or("").to_string(),
    };
    let session = input.session.clone();
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
        d => serde_json::json!({ "systemMessage": hook::stop_message(&receipt, &d, gave_up) }),
    };
    println!("{out}");
    Ok(())
}
