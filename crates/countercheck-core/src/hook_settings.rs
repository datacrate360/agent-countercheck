//! Agent hook settings: did the worker switch its own referee off?
//!
//! A coding agent reads its hooks from settings files inside the project (Claude Code
//! `.claude/settings.json` and `.claude/settings.local.json`, Cortex Code `.cortex/...`, and the
//! files `countercheck hook install` writes for Cursor, Codex, Copilot, Goose and Kiro), and the
//! worker can edit those files mid-session. One line (`"disableAllHooks": true`) turns every
//! hook off, a project setting overrides the user's, and the agent picks the edit up at once. Then
//! the Stop hook never runs and "done" is never checked.
//!
//! This module compares each file at the base revision with the working tree and names what
//! weakens the referee:
//! - `disableAllHooks` turned on;
//! - a Countercheck hook removed or changed in any way (a lower `timeout` makes the hook give up and
//!   the agent stop unchecked);
//! - a `COUNTERCHECK_*` variable set in the file's `env` block that changes what the hook does
//!   (`COUNTERCHECK_HOOK_DISABLED`, `_BASE`, `_SANDBOX`, `_MAX_NUDGES`, ...).
//!
//! Adding hooks, adding other settings or removing a weakening entry is fine. Files are hostile
//! input: size-capped, never followed through a symlink, and a file that can't be read as JSON
//! counts as changed. This module can only add a reason and turn the verdict into FAILED.

use crate::receipt::Verdict;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// The project-level hook settings files of the agents Countercheck hooks into (the project files
/// of `apps/countercheck/src/agents/`: a new agent's file goes here too).
pub const FILES: [&str; 11] = [
    ".claude/settings.json",
    ".claude/settings.local.json",
    ".cortex/settings.json",
    ".cortex/settings.local.json",
    ".cursor/hooks.json",
    ".codex/hooks.json",
    ".github/hooks/countercheck.json",
    ".agents/plugins/countercheck/hooks/hooks.json",
    ".kiro/hooks/countercheck.json",
    // Goose's project settings, where `disabledPlugins` can switch the countercheck plugin off.
    ".config/goose/settings.json",
    ".config/goose/settings.local.json",
];

/// Agents whose hook is a whole program file Countercheck writes (a TypeScript plugin), not JSON
/// settings: compared byte for byte with the base by [`compare_plugin`]. Each plugin agent in
/// `apps/countercheck/src/agents/` adds its project file here.
pub const PLUGIN_FILES: &[&str] = &[".amp/plugins/countercheck.ts"];

/// Whether the tamper check compares this project file (a settings file or a plugin file).
pub fn is_guarded(rel: &str) -> bool {
    FILES.contains(&rel) || PLUGIN_FILES.contains(&rel)
}

/// Larger files are not parsed (and count as changed when they differ from the base).
pub const MAX_BYTES: usize = 1024 * 1024;

/// Variables the worker may set: labels and logging, nothing that changes a check.
const HARMLESS_ENV: [&str; 2] = ["COUNTERCHECK_ROLE", "COUNTERCHECK_HOOK_LOG"];

/// One settings file as found on disk or in the base revision.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum File {
    Absent,
    /// A regular file's bytes.
    Regular(Vec<u8>),
    /// A symlink (its target as bytes, as git stores it). Never followed.
    Symlink(Vec<u8>),
}

/// What weakens the referee in one file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HookSettingsChange {
    pub path: String,
    pub what: String,
}

/// What matters in one settings file.
#[derive(Debug, Default, PartialEq, Eq)]
struct Summary {
    disable_all: bool,
    /// The countercheck plugin listed in `disabledPlugins` (Goose).
    plugin_off: bool,
    /// Event name -> every Countercheck handler under it, as canonical JSON.
    hooks: BTreeMap<String, BTreeSet<String>>,
    /// `COUNTERCHECK_*` variables that change what the hook does.
    env: BTreeMap<String, String>,
}

/// `None` when the bytes aren't a JSON object (Claude Code wouldn't read it either, but
/// Countercheck can't tell what it holds).
fn summarize(bytes: &[u8]) -> Option<Summary> {
    if bytes.len() > MAX_BYTES {
        return None;
    }
    let v: serde_json::Value = serde_json::from_slice(bytes).ok()?;
    let obj = v.as_object()?;
    let mut s = Summary {
        disable_all: obj.get("disableAllHooks") == Some(&serde_json::Value::Bool(true)),
        plugin_off: obj
            .get("disabledPlugins")
            .and_then(|p| p.as_array())
            .is_some_and(|p| p.iter().any(|n| n.as_str() == Some("countercheck"))),
        ..Summary::default()
    };
    match obj.get("hooks") {
        // Event name -> handlers (Claude Code, Cortex, Cursor, Codex, Copilot, Goose).
        Some(serde_json::Value::Object(events)) => {
            for (event, groups) in events {
                let mut found = BTreeSet::new();
                collect_handlers(groups, &mut found);
                if !found.is_empty() {
                    s.hooks.insert(event.clone(), found);
                }
            }
        }
        // A list of hook definitions, each naming its event in `trigger` (Kiro).
        Some(serde_json::Value::Array(defs)) => {
            for def in defs {
                let event = def
                    .get("trigger")
                    .and_then(|t| t.as_str())
                    .unwrap_or("(no trigger)");
                let mut found = BTreeSet::new();
                collect_handlers(def, &mut found);
                if !found.is_empty() {
                    s.hooks.entry(event.to_string()).or_default().extend(found);
                }
            }
        }
        _ => {}
    }
    if let Some(env) = obj.get("env").and_then(|e| e.as_object()) {
        for (k, val) in env {
            if k.starts_with("COUNTERCHECK_") && !HARMLESS_ENV.contains(&k.as_str()) {
                s.env.insert(k.clone(), val.to_string());
            }
        }
    }
    Some(s)
}

/// Whether this object runs Countercheck: a command field naming it (`command`; Copilot's `bash` /
/// `powershell`), directly or in its `action` (Kiro), so the whole definition, timeout and any
/// on/off flag included, is what gets compared.
fn runs_countercheck(map: &serde_json::Map<String, serde_json::Value>) -> bool {
    let names_us = |m: &serde_json::Map<String, serde_json::Value>| {
        ["command", "bash", "powershell"].iter().any(|k| {
            m.get(*k)
                .and_then(|c| c.as_str())
                .is_some_and(|c| c.contains("countercheck"))
        })
    };
    names_us(map)
        || map
            .get("action")
            .and_then(|a| a.as_object())
            .is_some_and(names_us)
}

/// Every object under `v` that runs Countercheck, as canonical JSON, so any change to it
/// (command, timeout, type, enabled) shows. Depth-limited: settings are small.
fn collect_handlers(v: &serde_json::Value, out: &mut BTreeSet<String>) {
    fn walk(v: &serde_json::Value, out: &mut BTreeSet<String>, depth: usize) {
        if depth > 8 {
            return;
        }
        match v {
            serde_json::Value::Array(items) => {
                for i in items {
                    walk(i, out, depth + 1);
                }
            }
            serde_json::Value::Object(map) => {
                if runs_countercheck(map) {
                    // serde_json's Map is sorted, so this is canonical.
                    out.insert(v.to_string());
                } else {
                    for val in map.values() {
                        walk(val, out, depth + 1);
                    }
                }
            }
            _ => {}
        }
    }
    walk(v, out, 0);
}

/// What the worker's version of `path` does to the referee, compared with the base.
pub fn compare(path: &str, base: &File, now: &File) -> Vec<HookSettingsChange> {
    let finding = |what: String| HookSettingsChange {
        path: path.to_string(),
        what,
    };
    if base == now {
        return Vec::new();
    }
    let base_summary = match base {
        File::Absent => Some(Summary::default()),
        File::Regular(b) => summarize(b),
        // A symlink in the base: Countercheck never reads through it, so it can't hold our hook.
        File::Symlink(_) => Some(Summary::default()),
    };
    let now_bytes = match now {
        File::Absent => None,
        File::Regular(b) => Some(b),
        File::Symlink(_) => {
            return vec![finding(
                "is now a symlink; Countercheck doesn't follow it, so it can't tell whether the agent's hooks still run"
                    .to_string(),
            )];
        }
    };
    let now_summary = match now_bytes {
        None => Summary::default(),
        Some(b) => match summarize(b) {
            Some(s) => s,
            None => {
                return vec![finding(
                    "was changed and can't be read as settings (not a JSON object, or larger than 1 MiB)"
                        .to_string(),
                )];
            }
        },
    };
    let base_summary = base_summary.unwrap_or_default();
    let mut out = Vec::new();
    if now_summary.disable_all && !base_summary.disable_all {
        out.push(finding(
            "turns every hook off (\"disableAllHooks\": true)".to_string(),
        ));
    }
    if now_summary.plugin_off && !base_summary.plugin_off {
        out.push(finding(
            "switches the countercheck plugin off (\"disabledPlugins\")".to_string(),
        ));
    }
    for (event, handlers) in &base_summary.hooks {
        match now_summary.hooks.get(event) {
            None => out.push(finding(format!("removes the Countercheck {event} hook"))),
            Some(now_handlers) if now_handlers != handlers => out.push(finding(format!(
                "changes the Countercheck {event} hook (its command, timeout or type)"
            ))),
            Some(_) => {}
        }
    }
    for (key, val) in &now_summary.env {
        if base_summary.env.get(key) != Some(val) {
            out.push(finding(format!(
                "sets {key}, which changes what the hook checks"
            )));
        }
    }
    out
}

/// What the worker did to a Countercheck plugin file (see [`PLUGIN_FILES`]). It is code, not
/// settings, so it is compared byte for byte: when the base holds a Countercheck plugin, removing
/// it, replacing it with a symlink or changing it in any way is a finding. A plugin that wasn't in
/// the base (or isn't Countercheck's) is not: adding hooks is allowed.
pub fn compare_plugin(path: &str, base: &File, now: &File) -> Vec<HookSettingsChange> {
    let File::Regular(was) = base else {
        return Vec::new();
    };
    if base == now || !String::from_utf8_lossy(was).contains("countercheck") {
        return Vec::new();
    }
    let what = match now {
        File::Absent => "removes the Countercheck plugin",
        File::Symlink(_) => {
            "is now a symlink; Countercheck doesn't follow it, so it can't tell whether the plugin still runs"
        }
        File::Regular(_) => "changes the Countercheck plugin (any edit to its code counts)",
    };
    vec![HookSettingsChange {
        path: path.to_string(),
        what: what.to_string(),
    }]
}

/// Hooks switched off in the files as they are now, with no base to compare against (for the
/// report and live UI): `disableAllHooks` or a `COUNTERCHECK_HOOK_DISABLED` in `env`.
pub fn switched_off(path: &str, now: &File) -> Option<HookSettingsChange> {
    let File::Regular(bytes) = now else {
        return None;
    };
    let s = summarize(bytes)?;
    let what = if s.disable_all {
        "turns every hook off (\"disableAllHooks\": true)"
    } else if s.plugin_off {
        "switches the countercheck plugin off (\"disabledPlugins\")"
    } else if s.env.contains_key("COUNTERCHECK_HOOK_DISABLED") {
        "turns the Countercheck hook off (COUNTERCHECK_HOOK_DISABLED)"
    } else {
        return None;
    };
    Some(HookSettingsChange {
        path: path.to_string(),
        what: what.to_string(),
    })
}

/// Add the findings to the verdict: any finding means the referee may have been switched off, so
/// the run is FAILED whatever the checks said. Never makes a verdict better.
pub fn apply(
    verdict: Verdict,
    mut reasons: Vec<String>,
    found: &[HookSettingsChange],
) -> (Verdict, Vec<String>) {
    if found.is_empty() {
        return (verdict, reasons);
    }
    for f in found {
        reasons.push(format!(
            "the agent changed its own hook settings: {} {}; its \"done\" may never have been checked (if you made this change, commit it)",
            f.path, f.what
        ));
    }
    // A VERIFIED's "all checks passed" stays: the checks did pass; the session wasn't refereed.
    let _ = verdict;
    (Verdict::Failed, reasons)
}

#[cfg(test)]
mod tests {
    use super::*;

    const HOOKED: &str = r#"{"hooks":{
        "SessionStart":[{"hooks":[{"type":"command","command":"countercheck hook session-start","timeout":10}]}],
        "Stop":[{"hooks":[{"type":"command","command":"countercheck hook claude-stop","timeout":600}]}]},
        "permissions":{"allow":["Bash(ls)"]}}"#;

    fn reg(s: &str) -> File {
        File::Regular(s.as_bytes().to_vec())
    }

    fn whats(base: &File, now: &File) -> Vec<String> {
        compare(".claude/settings.json", base, now)
            .into_iter()
            .map(|f| f.what)
            .collect()
    }

    #[test]
    fn unchanged_or_honest_edits_are_fine() {
        assert!(whats(&reg(HOOKED), &reg(HOOKED)).is_empty());
        // Other settings change, a hook is added, keys reorder: nothing to report.
        let more = r#"{"permissions":{"allow":["Bash(ls)","Bash(git status)"]},"hooks":{
            "Stop":[{"hooks":[{"timeout":600,"command":"countercheck hook claude-stop","type":"command"}]},
                    {"hooks":[{"type":"command","command":"./lint.sh"}]}],
            "SessionStart":[{"hooks":[{"type":"command","command":"countercheck hook session-start","timeout":10}]}],
            "PostToolUse":[{"hooks":[{"type":"command","command":"prettier"}]}]},
            "env":{"COUNTERCHECK_ROLE":"Docs"}}"#;
        assert!(
            whats(&reg(HOOKED), &reg(more)).is_empty(),
            "{:?}",
            whats(&reg(HOOKED), &reg(more))
        );
        // A file that never had our hook and doesn't switch anything off.
        assert!(whats(&File::Absent, &reg(r#"{"model":"opus"}"#)).is_empty());
        // Turning hooks back on is not a finding.
        assert!(whats(&reg(r#"{"disableAllHooks":true}"#), &reg("{}")).is_empty());
    }

    #[test]
    fn switching_the_referee_off_is_found() {
        let off = HOOKED.replacen('{', r#"{"disableAllHooks":true,"#, 1);
        assert_eq!(
            whats(&reg(HOOKED), &reg(&off)),
            ["turns every hook off (\"disableAllHooks\": true)"]
        );
        // In a new (often git-ignored) local file too.
        assert_eq!(
            whats(&File::Absent, &reg(r#"{"disableAllHooks": true}"#)).len(),
            1
        );
        let no_stop = r#"{"hooks":{"SessionStart":[{"hooks":[{"type":"command","command":"countercheck hook session-start","timeout":10}]}]}}"#;
        assert_eq!(
            whats(&reg(HOOKED), &reg(no_stop)),
            ["removes the Countercheck Stop hook"]
        );
        assert_eq!(whats(&reg(HOOKED), &File::Absent).len(), 2);
        let short = HOOKED.replace("\"timeout\":600", "\"timeout\":1");
        assert_eq!(
            whats(&reg(HOOKED), &reg(&short)),
            ["changes the Countercheck Stop hook (its command, timeout or type)"]
        );
        let wrapped = HOOKED.replace(
            "countercheck hook claude-stop",
            "countercheck hook claude-stop || true",
        );
        assert_eq!(whats(&reg(HOOKED), &reg(&wrapped)).len(), 1);
        let env = HOOKED.replacen('{', r#"{"env":{"COUNTERCHECK_SANDBOX":"none"},"#, 1);
        assert_eq!(
            whats(&reg(HOOKED), &reg(&env)),
            ["sets COUNTERCHECK_SANDBOX, which changes what the hook checks"]
        );
    }

    #[test]
    fn hostile_files_count_as_changed() {
        assert_eq!(whats(&reg(HOOKED), &reg("{ not json")).len(), 1);
        assert_eq!(whats(&reg(HOOKED), &reg("[1,2]")).len(), 1);
        let big = format!("{{\"x\":\"{}\"}}", "a".repeat(MAX_BYTES));
        assert_eq!(whats(&reg(HOOKED), &reg(&big)).len(), 1);
        assert_eq!(
            whats(&reg(HOOKED), &File::Symlink(b"/tmp/x.json".to_vec())).len(),
            1
        );
        // The same symlink as in the base is no change.
        let link = File::Symlink(b"shared.json".to_vec());
        assert!(whats(&link, &link).is_empty());
    }

    #[test]
    fn apply_fails_the_run_and_never_improves_it() {
        let f = compare(
            ".claude/settings.json",
            &File::Absent,
            &reg(r#"{"disableAllHooks":true}"#),
        );
        let (v, r) = apply(
            Verdict::Verified,
            vec!["all 1 check(s) passed in a clean copy".into()],
            &f,
        );
        assert_eq!(v, Verdict::Failed);
        assert!(
            r[1].contains(".claude/settings.json turns every hook off"),
            "{r:?}"
        );
        assert_eq!(apply(Verdict::Unverified, vec![], &f).0, Verdict::Failed);
        assert_eq!(apply(Verdict::Verified, vec![], &[]).0, Verdict::Verified);
    }

    #[test]
    fn every_agents_hook_file_is_guarded() {
        // Cursor: flat handlers.
        let cursor = r#"{"version":1,"hooks":{"stop":[{"command":"countercheck hook cursor-stop","timeout":600}]}}"#;
        let cursor_short = cursor.replace("600", "1");
        assert_eq!(
            compare(".cursor/hooks.json", &reg(cursor), &reg(&cursor_short)).len(),
            1
        );
        assert_eq!(
            compare(
                ".cursor/hooks.json",
                &reg(cursor),
                &reg(r#"{"version":1,"hooks":{}}"#)
            )[0]
            .what,
            "removes the Countercheck stop hook"
        );
        // Copilot: the command is under `bash` / `powershell`.
        let copilot = r#"{"version":1,"hooks":{"agentStop":[{"type":"command","bash":"countercheck hook copilot-stop","powershell":"countercheck hook copilot-stop","timeoutSec":600}]}}"#;
        assert_eq!(
            compare(
                ".github/hooks/countercheck.json",
                &reg(copilot),
                &File::Absent
            )[0]
            .what,
            "removes the Countercheck agentStop hook"
        );
        let wrapped = copilot.replacen(
            "\"bash\":\"countercheck hook copilot-stop\"",
            "\"bash\":\"countercheck hook copilot-stop || true\"",
            1,
        );
        assert_eq!(
            compare(
                ".github/hooks/countercheck.json",
                &reg(copilot),
                &reg(&wrapped)
            )
            .len(),
            1
        );
        // Kiro: a list of definitions; the timeout and any flag sit beside `action`.
        let kiro = r#"{"version":"v1","hooks":[{"name":"countercheck-stop","trigger":"Stop","action":{"type":"command","command":"countercheck hook kiro-stop"},"timeout":600}]}"#;
        assert_eq!(
            compare(
                ".kiro/hooks/countercheck.json",
                &reg(kiro),
                &reg(r#"{"version":"v1","hooks":[]}"#)
            )[0]
            .what,
            "removes the Countercheck Stop hook"
        );
        let off = kiro.replace("\"timeout\":600", "\"timeout\":600,\"enabled\":false");
        assert_eq!(
            compare(".kiro/hooks/countercheck.json", &reg(kiro), &reg(&off))[0].what,
            "changes the Countercheck Stop hook (its command, timeout or type)"
        );
        // Goose: the plugin switched off in the project's settings, even in a new local file.
        assert_eq!(
            compare(
                ".config/goose/settings.local.json",
                &File::Absent,
                &reg(r#"{"disabledPlugins":["countercheck"]}"#)
            )[0]
            .what,
            "switches the countercheck plugin off (\"disabledPlugins\")"
        );
        assert!(
            compare(
                ".config/goose/settings.json",
                &File::Absent,
                &reg(r#"{"disabledPlugins":["other"]}"#)
            )
            .is_empty()
        );
        // Honest edits stay fine: another hook added beside ours.
        let more = kiro.replace(
            "]}",
            r#",{"name":"lint","trigger":"Stop","action":{"type":"command","command":"npm run lint"}}]}"#,
        );
        assert!(compare(".kiro/hooks/countercheck.json", &reg(kiro), &reg(&more)).is_empty());
    }

    #[test]
    fn a_countercheck_plugin_is_compared_byte_for_byte() {
        let ours = reg("amp.on('agent.end', () => run('countercheck hook amp-stop'))\n");
        let p = ".amp/plugins/countercheck.ts";
        assert!(compare_plugin(p, &ours, &ours).is_empty());
        assert_eq!(
            compare_plugin(p, &ours, &File::Absent)[0].what,
            "removes the Countercheck plugin"
        );
        let edited = reg("amp.on('agent.end', () => {})\n");
        assert_eq!(compare_plugin(p, &ours, &edited).len(), 1);
        // One changed byte (a space) still counts: it is code.
        let spaced = reg("amp.on('agent.end', () => run('countercheck hook amp-stop')) \n");
        assert_eq!(compare_plugin(p, &ours, &spaced).len(), 1);
        assert_eq!(
            compare_plugin(p, &ours, &File::Symlink(b"/tmp/x.ts".to_vec())).len(),
            1
        );
        // A plugin added by the worker, or one that was never Countercheck's: not a finding.
        assert!(compare_plugin(p, &File::Absent, &ours).is_empty());
        assert!(compare_plugin(p, &reg("other plugin"), &File::Absent).is_empty());
    }

    #[test]
    fn switched_off_reads_the_current_files_only() {
        assert!(switched_off("p", &reg(r#"{"disableAllHooks":true}"#)).is_some());
        assert!(switched_off("p", &reg(r#"{"env":{"COUNTERCHECK_HOOK_DISABLED":"1"}}"#)).is_some());
        assert!(switched_off("p", &reg(HOOKED)).is_none());
        assert!(switched_off("p", &File::Absent).is_none());
    }
}
