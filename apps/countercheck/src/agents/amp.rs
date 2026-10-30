//! Amp (ampcode.com): its hooks are TypeScript plugins in `.amp/plugins/` (project) or
//! `~/.config/amp/plugins/` (user), long-lived processes run with Bun. Countercheck installs one
//! file, `countercheck.ts` (the template next to this file), which on `agent.end` runs
//! `countercheck hook amp-stop` with the turn on stdin and returns Amp's `{action: 'continue',
//! userMessage}` when the answer says so. On `session.start` it runs `hook session-start`.

use super::Target;
use crate::hook::{self, Decision, HookInput};
use std::path::PathBuf;

pub fn target() -> Target {
    Target {
        agent: "amp",
        label: "Amp",
        project_file: ".amp/plugins/countercheck.ts",
        // `$XDG_CONFIG_HOME/amp/plugins/` when that is set: see the docs page.
        user_file: Some(".config/amp/plugins/countercheck.ts"),
        top: Vec::new(),
        entries: |_| Vec::new(),
        // A project and a user plugin with the same name: Amp loads only the project one.
        also_reads: &[],
        after: "Amp loads plugins when a session starts: restart Amp in this project, or run \"plugins: reload\" \
                from the command palette (Ctrl+O). `amp plugins list` shows it with its agent.end and \
                session.start events. Plugins run with Bun, which Amp brings along.",
        docs: "docs/hooks/amp.md",
        plugin: Some(|bin| super::plugin_with_bin(include_str!("amp/plugin.ts"), bin)),
    }
}

/// Amp `agent.end`, from the plugin: stdin is `{cwd, thread_id, status, model,
/// last_assistant_message}`; stdout is `{"action":"continue","userMessage":...}` (the plugin returns
/// it to Amp, which starts another turn with it) or `{"message":...}` (the plugin shows it to the
/// user). Same env as `claude-stop`, including `COUNTERCHECK_MAX_NUDGES`.
pub fn stop() -> Result<(), String> {
    let Some(payload) = hook::read_payload()? else {
        return Ok(());
    };
    let text = |key: &str| payload.get(key).and_then(|v| v.as_str());
    // A turn the user cancelled, or that ended in an error, isn't the agent saying it is done.
    if matches!(text("status"), Some("cancelled" | "error")) {
        return Ok(());
    }
    let input = HookInput {
        cwd: text("cwd")
            .map(PathBuf::from)
            .or_else(|| std::env::current_dir().ok())
            .ok_or("no working directory in hook input")?,
        claim: text("last_assistant_message").map(str::to_string),
        agent: "amp",
        model: text("model").map(str::to_string),
        session: text("thread_id").unwrap_or("").to_string(),
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
            serde_json::json!({ "action": "continue", "userMessage": reason })
        }
        d => serde_json::json!({ "message": hook::stop_message(&receipt, &d, gave_up) }),
    };
    println!("{out}");
    Ok(())
}
