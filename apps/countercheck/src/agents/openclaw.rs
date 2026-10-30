//! OpenClaw: a native plugin (`openclaw/`: `index.ts`, `openclaw.plugin.json`, `package.json`) on
//! the typed `before_agent_finalize` hook, set up by hand (docs/hooks/openclaw.md).
//!
//! Not a `Target`: an OpenClaw plugin is a folder of files (the manifest is required even for a
//! single-file plugin, and `openclaw plugins install` needs a `package.json`), it is found in
//! OpenClaw's own workspace, `~/.openclaw/extensions` or a configured load path rather than the
//! git repository, and it runs `before_agent_finalize` only after the user grants
//! `plugins.entries.countercheck.hooks.allowConversationAccess` in `~/.openclaw/openclaw.json`.
//! The plugin passes the finalize event to `countercheck hook openclaw-stop` on stdin; this file
//! maps it, and the decision is the shared one.

use crate::hook::{self, Decision, HookInput};
use std::path::PathBuf;

/// The plugin's files, as the docs page shows them.
#[cfg(test)]
const PLUGIN: &str = include_str!("openclaw/index.ts");
#[cfg(test)]
const MANIFEST: &str = include_str!("openclaw/openclaw.plugin.json");
#[cfg(test)]
const PACKAGE: &str = include_str!("openclaw/package.json");

/// OpenClaw `before_agent_finalize`, relayed by the plugin: stdin is the event's `cwd`,
/// `sessionId`, `model` and `lastAssistantMessage`; stdout is `{"action":"revise","reason":...}`
/// (the plugin returns it, and OpenClaw runs one more model pass) or `{"action":"continue",
/// "message":...}` (the plugin logs the message). Same env as `claude-stop`, including
/// `COUNTERCHECK_MAX_NUDGES`. OpenClaw's embedded runner allows 3 revisions per run.
pub fn stop() -> Result<(), String> {
    let Some(payload) = hook::read_payload()? else {
        return Ok(());
    };
    let text = |key: &str| payload.get(key).and_then(|v| v.as_str());
    let input = HookInput {
        // No fallback to the current folder: the plugin runs in the Gateway, not the project.
        cwd: text("cwd")
            .filter(|c| !c.is_empty())
            .map(PathBuf::from)
            .ok_or("no cwd in OpenClaw's finalize event")?,
        claim: text("lastAssistantMessage").map(str::to_string),
        agent: "openclaw",
        model: text("model").map(str::to_string),
        session: text("sessionId").unwrap_or("").to_string(),
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
            serde_json::json!({ "action": "revise", "reason": reason })
        }
        d => serde_json::json!({
            "action": "continue",
            "message": hook::stop_message(&receipt, &d, gave_up),
        }),
    };
    println!("{out}");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_plugin_relays_on_stdin_and_never_builds_a_shell_command() {
        assert!(PLUGIN.contains("spawn(bin, [\"hook\", \"openclaw-stop\"]"));
        assert!(PLUGIN.contains("child.stdin.end(input)"));
        assert!(!PLUGIN.contains("shell: true") && !PLUGIN.contains("exec("));
        // OpenClaw's default 15 s budget would skip the hook on any real check run.
        assert!(PLUGIN.contains("timeoutMs: TIMEOUT_MS") && PLUGIN.contains("600_000"));
        let m: serde_json::Value = serde_json::from_str(MANIFEST).unwrap();
        assert_eq!(m["id"], "countercheck");
        assert!(
            m["configSchema"].is_object(),
            "OpenClaw requires a configSchema"
        );
        // `plugins install` keys the plugin by the manifest id; the package name must match it.
        let p: serde_json::Value = serde_json::from_str(PACKAGE).unwrap();
        assert_eq!(p["name"], m["id"]);
        assert_eq!(p["openclaw"]["extensions"][0], "./index.ts");
    }

    #[test]
    fn the_docs_page_shows_the_plugin_as_shipped() {
        let docs = include_str!("../../../../docs/hooks/openclaw.md");
        assert!(
            docs.contains(PLUGIN.trim_end()),
            "update docs/hooks/openclaw.md"
        );
        assert!(
            docs.contains(MANIFEST.trim_end()),
            "update docs/hooks/openclaw.md"
        );
        assert!(
            docs.contains(PACKAGE.trim_end()),
            "update docs/hooks/openclaw.md"
        );
    }
}
