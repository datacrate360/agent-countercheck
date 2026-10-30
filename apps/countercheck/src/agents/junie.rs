//! JetBrains Junie CLI: hooks in the user's `~/.junie/config.json` (`$JUNIE_HOME/config.json`).
//!
//! Junie ignores hooks in a project's `.junie/config.json` (it reports them as a configuration
//! warning), so there is no project install: `hook install --agent junie` writes the user file, and
//! `--scope project` is refused with [`NO_PROJECT_HOOKS`]. Junie's `Stop` input has no transcript
//! and, per its docs, only `stop_hook_active` and `last_assistant_message` (Junie 26.11.3 also sends
//! `session_id`, `cwd` and `project_path`); its answer is Claude Code's `{"decision":"block",...}`,
//! but a plain exit with `additionalContext` would make it run one more step. So Junie has its own
//! stop hook (`countercheck hook junie-stop`) that only maps the input; the decision is the shared one.

use super::{Entry, Target};
use crate::hook::{self, Decision, HookInput};
use std::path::PathBuf;

/// Why `--scope project` is refused, and what to do instead.
pub const NO_PROJECT_HOOKS: &str = "Junie ignores hooks in a project's .junie/config.json (only \
     ~/.junie/config.json, files passed with --config-location, and installed extensions run hooks), \
     so a project install would do nothing. Use --scope user (the default for junie); to share the \
     hooks with a team, see docs/hooks/junie.md";

/// `SessionStart` (locks the checks to where the session began) and `Stop` (re-runs the checks),
/// in the shape of the docs' `config.json` example: event, matcher entry, command hooks. No
/// `blockOnError`: a broken referee must never trap the agent.
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
                super::command(bin, "junie-stop")
            ),
            needles: vec!["hook junie-stop"],
        },
    ]
}

pub fn target() -> Target {
    Target {
        agent: "junie",
        label: "Junie",
        // Never written: Junie skips hooks here (see NO_PROJECT_HOOKS).
        project_file: ".junie/config.json",
        // `$JUNIE_HOME/config.json` when JUNIE_HOME is set (agents::HOME_VARS).
        user_file: Some(".junie/config.json"),
        top: Vec::new(),
        entries,
        also_reads: &[],
        after: "Junie reads ~/.junie/config.json when it starts: start a new `junie` session (hooks need the Early \
                Access build: curl -fsSL https://junie.jetbrains.com/install-eap.sh | bash). The hook runs in every \
                project; in one without .countercheck/checks.toml it lets Junie finish as UNVERIFIED.",
        docs: "docs/hooks/junie.md",
    }
}

/// Junie `Stop`: stdin is `{"hook_event_name":"Stop","stop_hook_active":...,
/// "last_assistant_message":...}` (plus `session_id`, `cwd`, `project_path` when Junie sends them);
/// stdout is `{"decision":"block","reason":...}` (the reason goes back to the agent, which retries)
/// or `{"systemMessage":...}` (user-facing; Junie doesn't show it for Stop yet, so the same line
/// also goes to stderr, Junie's log). Same env as `claude-stop`, including `COUNTERCHECK_MAX_NUDGES`.
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
    // Without a folder in the input the hook runs where Junie runs it (its working directory).
    let cwd = text("cwd")
        .or_else(|| text("project_path"))
        .map(PathBuf::from)
        .or_else(|| std::env::current_dir().ok())
        .ok_or("no working directory in hook input")?;
    let session = text("session_id").unwrap_or("").to_string();
    // Push-backs are counted per session; without a session id, per task: Junie sets
    // `stop_hook_active` false on the first Stop of each task, so that one starts a new count.
    let counter = if session.is_empty() {
        "junie-task"
    } else {
        session.as_str()
    };
    let new_task = payload.get("stop_hook_active").and_then(|v| v.as_bool()) == Some(false);
    let input = HookInput {
        cwd,
        // An empty submission is sent as "": no claim.
        claim: text("last_assistant_message").map(str::to_string),
        agent: "junie",
        // Junie's hook input has no model.
        model: None,
        session: session.clone(),
    };
    let (receipt, path, decision) = hook::check(input)?;
    if session.is_empty() && new_task {
        hook::reset_nudges(&path, counter);
    }
    let gave_up = match &decision {
        Decision::Continue(_) => !hook::take_nudge(&path, counter),
        _ => false,
    };
    hook::log_call(&session, &receipt, &path, &decision, Some(gave_up));
    let out = match decision {
        Decision::Continue(reason) if !gave_up => {
            serde_json::json!({ "decision": "block", "reason": reason })
        }
        // Never `additionalContext` here: on a plain exit Junie would run one more step with it.
        d => {
            let msg = hook::stop_message(&receipt, &d, gave_up);
            eprintln!("{msg}");
            serde_json::json!({ "systemMessage": msg })
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
    fn writes_the_documented_shape_and_keeps_other_settings() {
        let t = target();
        let mut n = json::parse("{\n  \"model\": \"sonnet\"\n}\n").unwrap();
        assert_eq!(add(&t, &mut n, "countercheck").unwrap(), 2);
        let v: serde_json::Value = serde_json::from_str(&json::pretty(&n)).unwrap();
        assert_eq!(
            v,
            serde_json::json!({"model": "sonnet", "hooks": {
                "SessionStart": [{"hooks": [{"type": "command", "command": "countercheck hook session-start", "timeout": 10}]}],
                "Stop": [{"hooks": [{"type": "command", "command": "countercheck hook junie-stop", "timeout": 600}]}],
            }})
        );
        assert!(!json::pretty(&n).contains("blockOnError"));
        assert_eq!(remove(&t, &mut n, "countercheck"), 2);
        assert_eq!(json::pretty(&n), "{\n  \"model\": \"sonnet\"\n}\n");
    }
}
