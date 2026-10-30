//! Devin CLI (Cognition's terminal agent): hooks in `.devin/hooks.v1.json`, where the whole file is
//! the event map (no `"hooks"` key). Docs: <https://docs.devin.ai/cli/extensibility/hooks/overview>,
//! <https://docs.devin.ai/cli/extensibility/hooks/lifecycle-hooks>.
//!
//! Devin's hooks are Claude-style (`{"decision":"block","reason":...}` sends the agent back), but its
//! `Stop` input documents only `stop_hook_active`, `session_id`, `prompt_id` and (CLI changelog)
//! `last_assistant_message`; the project root is the `DEVIN_PROJECT_DIR` variable. The binary's hook
//! input type also names `cwd`, `model` and `transcript_path`, read here when present. Devin also
//! runs the hooks in `.claude/settings*.json`, where `claude-stop` would label the receipt
//! `claude-code`. So Devin has its own stop hook (`countercheck hook devin-stop`) that only maps the
//! input; the decision is the shared one.

use super::{Entry, Target};
use crate::hook::{self, Decision, HookInput};
use std::path::PathBuf;

/// `SessionStart` (locks the checks to where the session began) and `Stop` (re-runs the checks),
/// keys in the order of the docs' examples: event, matcher group (`""` for non-tool events),
/// handlers.
fn entries(bin: &str) -> Vec<Entry> {
    vec![
        Entry {
            path: vec!["SessionStart"],
            value: format!(
                r#"{{"matcher": "", "hooks": [{{"type": "command", "command": {}, "timeout": 10}}]}}"#,
                super::command(bin, "session-start")
            ),
            needles: vec!["hook session-start"],
        },
        Entry {
            path: vec!["Stop"],
            value: format!(
                r#"{{"matcher": "", "hooks": [{{"type": "command", "command": {}, "timeout": 600}}]}}"#,
                super::command(bin, "devin-stop")
            ),
            needles: vec!["hook devin-stop"],
        },
    ]
}

pub fn target() -> Target {
    Target {
        agent: "devin",
        label: "Devin CLI",
        project_file: ".devin/hooks.v1.json",
        // The user-level hooks sit under a `"hooks"` key in `~/.config/devin/config.json`
        // (`%APPDATA%\devin\config.json` on Windows), a different shape and folder per OS: the
        // docs page shows how to add them by hand.
        user_file: None,
        top: Vec::new(),
        entries,
        // Devin CLI also reads hooks from its config files and, by default, Claude Code's settings.
        also_reads: &[
            ".claude/settings.json",
            ".claude/settings.local.json",
            ".devin/config.json",
            ".devin/config.local.json",
        ],
        after: "Devin CLI loads hooks when a session starts: start a new `devin` session in this project. If it \
                asks \"Do you trust the authors of this directory?\", say yes. Then type /hooks and check that both Countercheck hooks are listed \
                from .devin/hooks.v1.json.",
        docs: "docs/hooks/devin.md",
        plugin: None,
    }
}

/// Devin CLI `Stop`: stdin is the Stop hook JSON; stdout is `{"decision":"block","reason":...}`
/// (Devin keeps going with the reason) or `{"systemMessage":...}` (the line for the user). Always
/// exits 0: exit 2 would also block, with no reason. Same env as `claude-stop`, including
/// `COUNTERCHECK_MAX_NUDGES`.
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
    // `cwd` if Devin sends it, else the project root it sets for every hook, else the hook's folder.
    let cwd = text("cwd")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("DEVIN_PROJECT_DIR").map(PathBuf::from))
        .or_else(|| std::env::current_dir().ok())
        .ok_or("no working directory in hook input")?;
    let session = text("session_id").unwrap_or("").to_string();
    let input = HookInput {
        // The final message if given; else the transcript, when it is in a format Countercheck
        // reads. Neither: no claim (never a pass).
        claim: text("last_assistant_message")
            .map(str::to_string)
            .or_else(|| text("transcript_path").and_then(hook::last_assistant_text)),
        agent: "devin",
        model: text("model")
            .map(str::to_string)
            .or_else(|| hook::recorded_model(&cwd, &session)),
        session,
        cwd,
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
