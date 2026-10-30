//! Claude Code and Snowflake Cortex Code: the same hook input and answer (`countercheck hook
//! claude-stop`, also `hook stop`), different settings folders.

use super::{Entry, Target};

/// `SessionStart` (locks the checks to where the session began, fingerprints linked folders) and
/// `Stop` (re-runs the checks), in the shape both apps read.
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
                super::command(bin, "claude-stop")
            ),
            needles: vec!["hook claude-stop", "hook stop"],
        },
    ]
}

pub fn claude_code() -> Target {
    Target {
        agent: "claude-code",
        label: "Claude Code",
        project_file: ".claude/settings.json",
        user_file: Some(".claude/settings.json"),
        top: Vec::new(),
        entries,
        also_reads: &[],
        after: "Claude Code loads hooks when a session starts: start a new session (or run /hooks) to use them.",
        docs: "docs/hooks/claude-code.md",
    }
}

pub fn cortex_code() -> Target {
    Target {
        agent: "cortex-code",
        label: "Cortex Code",
        project_file: ".cortex/settings.json",
        user_file: Some(".snowflake/cortex/settings.json"),
        top: Vec::new(),
        entries,
        // Cortex Code also reads Claude Code's project settings.
        also_reads: &[".claude/settings.json"],
        after: "Cortex Code loads hooks when a session starts: start a new session to use them.",
        docs: "docs/hooks/claude-code.md#cortex-code",
    }
}
