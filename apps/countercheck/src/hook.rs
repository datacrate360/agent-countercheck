//! Agent hooks: run `countercheck verify` when an agent tries to finish, and tell it to keep
//! going when the work fails. Hooks fail open: any error is logged to stderr and the agent
//! carries on, because a broken referee must never trap an agent in a loop.

use crate::{git, runner, verify};
use countercheck_core::claim::reports_blocker;
use countercheck_core::receipt::{
    MAX_MODEL_CHARS, MAX_ROLE_CHARS, MAX_SESSION_CHARS, Receipt, Verdict, Worker, clean_label,
};
use std::io::Read;
use std::path::{Path, PathBuf};

/// Longest nudge message sent back to the agent.
const MAX_MESSAGE_CHARS: usize = 2000;

/// What the hook decided, before it is printed.
pub enum Decision {
    /// Say nothing: the agent may finish.
    Silent,
    /// The checks FAILED, but the agent already reports that the work isn't done (and changed no
    /// protected file): it is telling the truth, so it may stop. The FAILED receipt is kept.
    HonestBlocker,
    /// Tell the agent to keep working, with this message.
    Continue(String),
}

/// What one agent's hook input says, after parsing.
struct HookInput {
    cwd: PathBuf,
    claim: Option<String>,
    agent: &'static str,
    model: Option<String>,
    session: String,
}

/// Read the hook JSON from stdin (8 MiB cap). `None` means the hook is switched off.
fn read_payload() -> Result<Option<serde_json::Value>, String> {
    let mut input = String::new();
    std::io::stdin()
        .take(8 * 1024 * 1024)
        .read_to_string(&mut input)
        .map_err(|e| format!("reading hook input: {e}"))?;
    if std::env::var_os("COUNTERCHECK_HOOK_DISABLED").is_some_and(|v| v == "1") {
        return Ok(None);
    }
    serde_json::from_str(&input)
        .map(Some)
        .map_err(|e| format!("hook input is not JSON: {e}"))
}

/// Run the checks for one hook call: the shared core behind every agent adapter.
fn check(input: HookInput) -> Result<(Receipt, PathBuf, Decision), String> {
    let blocker = input.claim.as_deref().is_some_and(reports_blocker);
    let sandbox = if std::env::var("COUNTERCHECK_SANDBOX").is_ok_and(|v| v == "none") {
        runner::Sandbox::None
    } else {
        runner::auto_sandbox()
    };
    let base = session_base(&input.cwd, &input.session);
    // Hook settings are compared with the session's start, never with `COUNTERCHECK_BASE`: a
    // settings file can set that variable for the hook, and point it past its own change.
    let settings_base = recorded_base(&input.cwd, &input.session);
    let opts = verify::Options {
        repo: input.cwd,
        base,
        settings_base,
        claim: input.claim,
        worker: Worker {
            agent: Some(input.agent.to_string()),
            model: input.model.and_then(|m| clean_label(&m, MAX_MODEL_CHARS)),
            cost_usd: None,
            // Several agents on one project: each one's environment says what it is for.
            role: std::env::var("COUNTERCHECK_ROLE")
                .ok()
                .and_then(|r| clean_label(&r, MAX_ROLE_CHARS)),
            session: clean_label(&input.session, MAX_SESSION_CHARS),
        },
        sandbox,
        sandbox_required: false,
        out: std::env::var_os("COUNTERCHECK_RECEIPTS").map(PathBuf::from),
        checks_from: None,
        checks_sha256: None,
    };
    let (receipt, path) = verify::verify(&opts)?;
    let decision = decide(&receipt, blocker);
    Ok((receipt, path, decision))
}

/// Hermes `pre_verify`: stdin is Hermes's hook JSON; stdout is `{"action":"continue",...}` or
/// nothing. Env: `COUNTERCHECK_BASE` (default: the commit the session started from, else HEAD), `COUNTERCHECK_HOOK_DISABLED=1` (no-op),
/// `COUNTERCHECK_HOOK_LOG` (append one JSON line per call), `COUNTERCHECK_SANDBOX=none`.
///
/// Which project: Hermes sends its own working directory as `cwd`, and the desktop app and gateway
/// run from the home folder, not the project. So when Hermes lists the files the turn changed
/// (`changed_paths`), the projects are the git repositories holding those files that have a
/// `.countercheck/checks.toml`; each is checked. Changed files outside any such project mean there
/// is nothing to check (silent). Without `changed_paths`, `cwd` is used as before.
pub fn hermes_pre_verify() -> Result<(), String> {
    let Some(payload) = read_payload()? else {
        return Ok(());
    };
    let field = |key: &str| -> Option<&serde_json::Value> {
        payload
            .get(key)
            .or_else(|| payload.get("extra").and_then(|e| e.get(key)))
    };
    let session = field("session_id")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let cwd = field("cwd")
        .and_then(|v| v.as_str())
        .map(PathBuf::from)
        .or_else(|| std::env::current_dir().ok())
        .ok_or("no working directory in hook input")?;
    let changed: Vec<&str> = field("changed_paths")
        .and_then(|v| v.as_array())
        .map(|a| a.iter().filter_map(|p| p.as_str()).collect())
        .unwrap_or_default();
    let repos = if changed.is_empty() {
        vec![cwd]
    } else {
        projects_for(&changed, &cwd, &session)
    };
    let claim = field("final_response")
        .and_then(|v| v.as_str())
        .map(str::to_string);
    let model = field("model").and_then(|v| v.as_str()).map(str::to_string);
    let several = repos.len() > 1;
    let mut messages = Vec::new();
    for repo in repos {
        let input = HookInput {
            cwd: repo.clone(),
            claim: claim.clone(),
            agent: "hermes",
            model: model.clone(),
            session: session.clone(),
        };
        let (receipt, path, decision) = match check(input) {
            Ok(r) => r,
            // one broken project must not hide another project's result
            Err(e) if several => {
                eprintln!("countercheck: {}: {e}", repo.display());
                continue;
            }
            Err(e) => return Err(e),
        };
        log_call(&session, &receipt, &path, &decision, None);
        if let Decision::Continue(message) = decision {
            messages.push(if several {
                format!("[{}]\n{message}", repo.display())
            } else {
                message
            });
        }
    }
    if !messages.is_empty() {
        let mut message = messages.join("\n\n");
        if message.chars().count() > MAX_MESSAGE_CHARS {
            let cut: String = message.chars().take(MAX_MESSAGE_CHARS - 20).collect();
            message = format!("{cut}\n[...truncated]");
        }
        let out = serde_json::json!({ "action": "continue", "message": message });
        println!("{out}");
    }
    Ok(())
}

/// Most projects one turn's changed files can span before the rest are ignored.
const MAX_PROJECTS: usize = 8;

/// The git repositories holding `paths` (absolute, or relative to `cwd`) that have a
/// `.countercheck/checks.toml`, in first-seen order. A deleted file counts through its nearest
/// folder that still exists.
///
/// "Has a check file" is decided from the base revision too, not only the working tree: an agent
/// that deletes (or renames) `.countercheck/checks.toml` must not turn its project into "nothing to
/// check". The checks themselves are then read from the base as always.
fn projects_for(paths: &[&str], cwd: &Path, session: &str) -> Vec<PathBuf> {
    let mut found: Vec<PathBuf> = Vec::new();
    let mut looked: Vec<PathBuf> = Vec::new();
    for p in paths.iter().take(10_000) {
        let p = Path::new(p);
        let abs = if p.is_absolute() {
            p.to_path_buf()
        } else {
            cwd.join(p)
        };
        let Some(dir) = abs.ancestors().skip(1).find(|d| d.is_dir()) else {
            continue;
        };
        // one `git` call per folder, not per file
        if found.iter().any(|r| dir.starts_with(r)) || looked.iter().any(|d| d == dir) {
            continue;
        }
        looked.push(dir.to_path_buf());
        let Ok(root) = git::toplevel(dir) else {
            continue;
        };
        // git prints C:/x/y on Windows; use the OS's own form so messages and comparisons agree
        let root = crate::snapshot::canonical(&root).unwrap_or(root);
        if found.contains(&root) || !has_checks(&root, session) {
            continue;
        }
        found.push(root);
        if found.len() == MAX_PROJECTS {
            break;
        }
    }
    found
}

/// True when the project has a check file in its working tree, at the session's base, or at HEAD.
fn has_checks(root: &Path, session: &str) -> bool {
    const FILE: &str = ".countercheck/checks.toml";
    if root.join(FILE).is_file() {
        return true;
    }
    let base = session_base(root, session);
    [base.as_str(), "HEAD"].iter().any(|rev| {
        git::resolve_commit(root, rev)
            .and_then(|c| git::Revision::load(root, &c))
            .is_ok_and(|r| r.files().contains_key(FILE))
    })
}

/// Claude Code `Stop` (Cortex Code sends the same input): stdin is the Stop hook JSON; stdout is
/// `{"decision":"block","reason":...}` or nothing. Same env as the Hermes hook, plus
/// `COUNTERCHECK_MAX_NUDGES` (default 3): after that many push-backs in one session the agent may
/// stop, so the hook never blocks forever.
pub fn claude_stop() -> Result<(), String> {
    let Some(payload) = read_payload()? else {
        return Ok(());
    };
    let text = |key: &str| payload.get(key).and_then(|v| v.as_str());
    // The final message is given directly; older versions only give the transcript.
    let claim = text("last_assistant_message")
        .map(str::to_string)
        .or_else(|| text("transcript_path").and_then(last_assistant_text));
    let input = HookInput {
        cwd: text("cwd")
            .map(PathBuf::from)
            .or_else(|| std::env::current_dir().ok())
            .ok_or("no working directory in hook input")?,
        claim,
        agent: agent_from_transcript(text("transcript_path").unwrap_or("")),
        // Claude Code's Stop input has no model (only SessionStart's does): read it from the last
        // assistant message (right after a `/model` switch too), else what SessionStart recorded.
        model: text("model")
            .map(str::to_string)
            .or_else(|| text("transcript_path").and_then(last_model))
            .or_else(|| {
                let cwd = text("cwd").map(PathBuf::from)?;
                recorded_model(&cwd, text("session_id")?)
            }),
        session: text("session_id").unwrap_or("").to_string(),
    };
    let session = input.session.clone();
    let (receipt, path, decision) = check(input)?;
    let gave_up = match &decision {
        Decision::Continue(_) => !take_nudge(&path, &session),
        _ => false,
    };
    log_call(&session, &receipt, &path, &decision, Some(gave_up));
    if let Decision::Continue(reason) = decision
        && !gave_up
    {
        let out = serde_json::json!({ "decision": "block", "reason": reason });
        println!("{out}");
    }
    Ok(())
}

/// Claude Code and Cortex Code send the same Stop input; only where the transcript lives tells
/// them apart (Cortex Code keeps sessions in `~/.snowflake/cortex/conversations/`).
fn agent_from_transcript(path: &str) -> &'static str {
    if path.replace('\\', "/").contains("/.snowflake/cortex/") {
        "cortex-code"
    } else {
        "claude-code"
    }
}

/// The commit the checks are locked to: `COUNTERCHECK_BASE` if set, else the commit this session
/// started from (recorded by `countercheck hook session-start`), else HEAD. Without a recorded
/// start, an agent that commits its own edits to the checks or protected files moves HEAD, and
/// with it the "locked" rules.
fn session_base(cwd: &Path, session: &str) -> String {
    if let Ok(b) = std::env::var("COUNTERCHECK_BASE") {
        return b;
    }
    recorded_base(cwd, session).unwrap_or_else(|| "HEAD".to_string())
}

/// Where a session's starting commit is kept: inside the git directory, outside the work tree, so
/// it is never copied into the clean copy, never committed, and not in the agent's file list.
fn session_file(cwd: &Path, session: &str) -> Option<PathBuf> {
    if safe_id(session) == "unknown" {
        return None;
    }
    let root = git::toplevel(cwd).ok()?;
    let dir = git::git_dir(&root).ok()?;
    Some(
        dir.join("countercheck")
            .join("sessions")
            .join(format!("{}.json", safe_id(session))),
    )
}

/// The model SessionStart recorded for this session, if any.
fn recorded_model(cwd: &Path, session: &str) -> Option<String> {
    let text = std::fs::read_to_string(session_file(cwd, session)?).ok()?;
    let v: serde_json::Value = serde_json::from_str(&text).ok()?;
    v.get("model")?.as_str().map(str::to_string)
}

/// The recorded start commit, if there is one and it still exists.
fn recorded_base(cwd: &Path, session: &str) -> Option<String> {
    let text = std::fs::read_to_string(session_file(cwd, session)?).ok()?;
    let v: serde_json::Value = serde_json::from_str(&text).ok()?;
    let base = v.get("base")?.as_str()?;
    if base.len() < 40 || !base.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let root = git::toplevel(cwd).ok()?;
    git::resolve_commit(&root, base).ok()
}

/// Session start (Claude Code / Cortex Code `SessionStart`, Hermes `on_session_start`): remember
/// the commit the session starts from, so the stop hook locks the checks to it even if the agent
/// commits. Prints nothing. A resumed session keeps its first record. Files older than 30 days
/// are removed.
pub fn session_start() -> Result<(), String> {
    let Some(payload) = read_payload()? else {
        return Ok(());
    };
    let text = |k: &str| payload.get(k).and_then(|v| v.as_str()).unwrap_or("");
    let cwd = PathBuf::from(text("cwd"));
    let session = text("session_id");
    let Some(file) = session_file(&cwd, session) else {
        return Ok(()); // no session id, or not a git repository: nothing to lock
    };
    if file.exists() {
        return Ok(());
    }
    let root = git::toplevel(&cwd)?;
    let Ok(head) = git::resolve_commit(&root, "HEAD") else {
        return Ok(()); // no commit yet
    };
    if let Some(dir) = file.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        prune_old(dir);
    }
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    std::fs::write(
        &file,
        serde_json::json!({
            "session_id": session,
            "base": head,
            "recorded_at_unix": now,
            // Shown on the project page if this session never gets a receipt.
            "agent": match payload.get("transcript_path").and_then(|v| v.as_str()) {
                Some(t) => Some(agent_from_transcript(t)),
                None if text("hook_event_name") == "SessionStart" => Some("claude-code"),
                None => None,
            },
            // Claude Code gives the model only here; the stop hook falls back to it.
            "model": payload
                .get("model")
                .and_then(|v| v.as_str())
                .and_then(|m| clean_label(m, MAX_MODEL_CHARS)),
        })
        .to_string(),
    )
    .map_err(|e| format!("{}: {e}", file.display()))
}

/// Remove files in `dir` (session records, hook counters) not changed for 30 days.
fn prune_old(dir: &Path) {
    const MAX_AGE: std::time::Duration = std::time::Duration::from_secs(30 * 24 * 3600);
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for e in entries.flatten() {
        let old = e
            .metadata()
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| t.elapsed().ok())
            .is_some_and(|age| age > MAX_AGE);
        if old {
            let _ = std::fs::remove_file(e.path());
        }
    }
}

/// Count one push-back for this session; false once the session has used them all. State lives
/// next to the receipts (already git-ignored) in `hook-state/<session>.json`. If the state can't be
/// read or written the push-back is still allowed: Claude Code's own cap stops a runaway loop.
fn take_nudge(receipt: &Path, session: &str) -> bool {
    let max: u64 = std::env::var("COUNTERCHECK_MAX_NUDGES")
        .ok()
        .and_then(|v| v.trim().parse().ok())
        .unwrap_or(3);
    let Some(dir) = receipt.parent().map(|d| d.join("hook-state")) else {
        return true;
    };
    // Counters of sessions nobody has used for 30 days go (like session records).
    prune_old(&dir);
    let file = dir.join(format!("{}.json", safe_id(session)));
    let used = std::fs::read_to_string(&file)
        .ok()
        .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok())
        .and_then(|v| v.get("nudges").and_then(|n| n.as_u64()))
        .unwrap_or(0);
    if used >= max {
        return false;
    }
    let _ = std::fs::create_dir_all(&dir);
    let _ = std::fs::write(
        &file,
        serde_json::json!({ "session_id": session, "nudges": used + 1 }).to_string(),
    );
    true
}

/// A session id as a file name: letters, digits, `-` and `_` only, at most 128 characters.
fn safe_id(session: &str) -> String {
    let id: String = session
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
        .take(128)
        .collect();
    if id.is_empty() {
        "unknown".to_string()
    } else {
        id
    }
}

/// The text of the last assistant message in the transcript. Any problem (missing file, bad lines)
/// means no claim, never an error. Two layouts:
/// - Claude Code: JSON Lines of `{"type":"assistant","message":{"content":...}}`.
/// - Cortex Code: `<id>.json` holds the session, with the messages in a `history` array (older
///   versions) or in `<id>.history.jsonl` next to it, one `{"role":"assistant","content":...}` per
///   line. `transcript_path` may name either file.
fn last_assistant_text(path: &str) -> Option<String> {
    let path = home_path(path);
    let (text, whole) = read_tail(&path)?;
    last_in_lines(&text)
        .or_else(|| {
            // a whole JSON document with a `history` array (only when the file was read in full)
            let doc: serde_json::Value = serde_json::from_str(&text).ok().filter(|_| whole)?;
            doc.get("history")?
                .as_array()?
                .iter()
                .rev()
                .find_map(assistant_text)
        })
        .or_else(|| {
            let name = path.file_name()?.to_str()?.strip_suffix(".json")?;
            let (text, _) = read_tail(&path.with_file_name(format!("{name}.history.jsonl")))?;
            last_in_lines(&text)
        })
}

/// `~/...` (or `~\...` on Windows, where the home folder is USERPROFILE when HOME isn't set).
fn home_path(path: &str) -> PathBuf {
    let home = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE"));
    let rest = path
        .strip_prefix("~/")
        .or_else(|| path.strip_prefix("~\\").filter(|_| cfg!(windows)));
    match (rest, home) {
        (Some(rest), Some(home)) => PathBuf::from(home).join(rest),
        _ => PathBuf::from(path),
    }
}

/// The last 8 MiB of a file, and whether that is the whole file.
fn read_tail(path: &Path) -> Option<(String, bool)> {
    use std::io::{Seek, SeekFrom};
    const TAIL: u64 = 8 * 1024 * 1024;
    let mut f = std::fs::File::open(path).ok()?;
    let len = f.metadata().ok()?.len();
    f.seek(SeekFrom::Start(len.saturating_sub(TAIL))).ok()?;
    let mut bytes = Vec::new();
    f.take(TAIL).read_to_end(&mut bytes).ok()?;
    Some((String::from_utf8_lossy(&bytes).into_owned(), len <= TAIL))
}

/// The last assistant text in JSON Lines.
fn last_in_lines(text: &str) -> Option<String> {
    text.lines().rev().find_map(|line| {
        let v: serde_json::Value = serde_json::from_str(line).ok()?;
        assistant_text(&v)
    })
}

/// The model of the last assistant message in a JSON Lines transcript (Claude Code writes
/// `{"type":"assistant","message":{"model":...}}`). `<synthetic>` marks messages Claude Code made up
/// itself (e.g. an API error notice), not a model, so those are skipped.
fn last_model(path: &str) -> Option<String> {
    let (text, _) = read_tail(&home_path(path))?;
    text.lines().rev().find_map(|line| {
        let v: serde_json::Value = serde_json::from_str(line).ok()?;
        let msg = v.get("message").unwrap_or(&v);
        let assistant = v.get("type").and_then(|t| t.as_str()) == Some("assistant")
            || msg.get("role").and_then(|r| r.as_str()) == Some("assistant");
        let model = msg.get("model")?.as_str()?;
        (assistant && !model.starts_with('<'))
            .then(|| clean_label(model, MAX_MODEL_CHARS))
            .flatten()
    })
}

/// The text of one transcript entry, if it is an assistant message with text: Claude Code's
/// `{"type":"assistant","message":{...}}` or Cortex Code's bare `{"role":"assistant",...}`.
fn assistant_text(v: &serde_json::Value) -> Option<String> {
    let msg = v.get("message").unwrap_or(v);
    if v.get("type").and_then(|t| t.as_str()) != Some("assistant")
        && msg.get("role").and_then(|r| r.as_str()) != Some("assistant")
    {
        return None;
    }
    let text = match msg.get("content")? {
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
}

/// Only a FAILED verdict keeps the agent going; VERIFIED and UNVERIFIED stay silent. A FAILED
/// verdict from an agent that already reports a blocker lets it stop, unless it changed protected
/// files: pushing an honest agent to "keep going" on impossible work wastes time and invites gaming
/// (see docs/eval/pilot-results.md).
pub fn decide(r: &Receipt, agent_reported_blocker: bool) -> Decision {
    if r.verdict != Verdict::Failed {
        return Decision::Silent;
    }
    // Switching off its own hook is never an honest stop: the next stop wouldn't be checked.
    let tampered = !r.protected_changes.is_empty() || !r.hook_settings.is_empty();
    if agent_reported_blocker && !tampered {
        return Decision::HonestBlocker;
    }
    let mut m = String::from(
        if r.checks.iter().all(|c| c.passed) && !r.hook_settings.is_empty() {
            "Countercheck re-ran this project's checks in a clean copy; they passed, but the run FAILED because you changed your own hook settings, so the task is not done yet.\n"
        } else {
            "Countercheck re-ran this project's checks in a clean copy and they FAILED, so the task is not done yet.\n"
        },
    );
    for reason in &r.reasons {
        m.push_str(&format!("- {reason}\n"));
    }
    if r.protected_changes
        .iter()
        .any(|p| p.action == "restored_from_base")
    {
        m.push_str(
            "The checks ran with the original versions of the protected test files. Changing or skipping \
             those tests does not count; fix the code instead.\n",
        );
    }
    if !r.hook_settings.is_empty() {
        m.push_str(
            "Undo your change to the hook settings file(s) above: Countercheck has to check every stop.\n",
        );
    }
    for c in r.checks.iter().filter(|c| !c.passed) {
        m.push_str(&format!(
            "\nFailed check \"{}\" ($ {}), last output:\n",
            c.name, c.run
        ));
        m.push_str(&c.output_tail);
        m.push('\n');
    }
    m.push_str("\nFix the problem, run the checks yourself, then finish.");
    if m.chars().count() > MAX_MESSAGE_CHARS {
        let cut: String = m.chars().take(MAX_MESSAGE_CHARS - 20).collect();
        m = format!("{cut}\n[...truncated]");
    }
    Decision::Continue(m)
}

/// `gave_up` is only logged by hooks with a loop guard (Claude Code).
fn log_call(session: &str, r: &Receipt, path: &Path, d: &Decision, gave_up: Option<bool>) {
    let Some(log) = std::env::var_os("COUNTERCHECK_HOOK_LOG") else {
        return;
    };
    let mut line = serde_json::json!({
        "time": r.created_at,
        "session_id": session,
        "verdict": r.verdict,
        "receipt_id": r.id,
        "receipt": path,
        "nudged": matches!(d, Decision::Continue(_)),
        "agent_reported_blocker": matches!(d, Decision::HonestBlocker),
        "reasons": r.reasons,
    });
    if let (Some(gave_up), Some(obj)) = (gave_up, line.as_object_mut()) {
        obj.insert("gave_up".to_string(), gave_up.into());
    }
    use std::io::Write;
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(log)
    {
        let _ = writeln!(f, "{line}");
    }
}
