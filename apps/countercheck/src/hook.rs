//! Agent hooks: run `countercheck verify` when an agent tries to finish, and tell it to keep
//! going when the work fails. Hooks fail open: any error is logged to stderr and the agent
//! carries on, because a broken referee must never trap an agent in a loop.

use crate::{runner, verify};
use countercheck_core::claim::reports_blocker;
use countercheck_core::receipt::{Receipt, Verdict, Worker};
use std::io::Read;
use std::path::PathBuf;

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

/// Hermes `pre_verify`: stdin is Hermes's hook JSON; stdout is `{"action":"continue",...}` or
/// nothing. Env: `COUNTERCHECK_BASE` (default HEAD), `COUNTERCHECK_HOOK_DISABLED=1` (no-op),
/// `COUNTERCHECK_HOOK_LOG` (append one JSON line per call), `COUNTERCHECK_SANDBOX=none`.
pub fn hermes_pre_verify() -> Result<(), String> {
    let mut input = String::new();
    std::io::stdin()
        .take(8 * 1024 * 1024)
        .read_to_string(&mut input)
        .map_err(|e| format!("reading hook input: {e}"))?;
    if std::env::var_os("COUNTERCHECK_HOOK_DISABLED").is_some_and(|v| v == "1") {
        return Ok(());
    }
    let payload: serde_json::Value =
        serde_json::from_str(&input).map_err(|e| format!("hook input is not JSON: {e}"))?;
    let field = |key: &str| -> Option<&serde_json::Value> {
        payload
            .get(key)
            .or_else(|| payload.get("extra").and_then(|e| e.get(key)))
    };
    let cwd = field("cwd")
        .and_then(|v| v.as_str())
        .map(PathBuf::from)
        .or_else(|| std::env::current_dir().ok())
        .ok_or("no working directory in hook input")?;
    let claim = field("final_response")
        .and_then(|v| v.as_str())
        .map(str::to_string);
    let blocker = claim.as_deref().is_some_and(reports_blocker);
    let model = field("model").and_then(|v| v.as_str()).map(str::to_string);
    let session = field("session_id")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let sandbox = if std::env::var("COUNTERCHECK_SANDBOX").is_ok_and(|v| v == "none")
        || !runner::seatbelt_works()
    {
        runner::Sandbox::None
    } else {
        runner::Sandbox::Seatbelt
    };
    let opts = verify::Options {
        repo: cwd,
        base: std::env::var("COUNTERCHECK_BASE").unwrap_or_else(|_| "HEAD".to_string()),
        claim,
        worker: Worker {
            agent: Some("hermes".to_string()),
            model,
            cost_usd: None,
        },
        sandbox,
        out: std::env::var_os("COUNTERCHECK_RECEIPTS").map(PathBuf::from),
    };
    let (receipt, path) = verify::verify(&opts)?;
    let decision = decide(&receipt, blocker);
    log_call(&session, &receipt, &path, &decision);
    if let Decision::Continue(message) = decision {
        let out = serde_json::json!({ "action": "continue", "message": message });
        println!("{out}");
    }
    Ok(())
}

/// Only a FAILED verdict keeps the agent going; VERIFIED and UNVERIFIED stay silent. A FAILED
/// verdict from an agent that already reports a blocker lets it stop, unless it changed protected
/// files: pushing an honest agent to "keep going" on impossible work wastes time and invites gaming
/// (see docs/eval/pilot-results.md).
pub fn decide(r: &Receipt, agent_reported_blocker: bool) -> Decision {
    if r.verdict != Verdict::Failed {
        return Decision::Silent;
    }
    let tampered = !r.protected_changes.is_empty();
    if agent_reported_blocker && !tampered {
        return Decision::HonestBlocker;
    }
    let mut m = String::from(
        "Countercheck re-ran this project's checks in a clean copy and they FAILED, so the task is not done yet.\n",
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

fn log_call(session: &str, r: &Receipt, path: &std::path::Path, d: &Decision) {
    let Some(log) = std::env::var_os("COUNTERCHECK_HOOK_LOG") else {
        return;
    };
    let line = serde_json::json!({
        "time": r.created_at,
        "session_id": session,
        "verdict": r.verdict,
        "receipt_id": r.id,
        "receipt": path,
        "nudged": matches!(d, Decision::Continue(_)),
        "agent_reported_blocker": matches!(d, Decision::HonestBlocker),
        "reasons": r.reasons,
    });
    use std::io::Write;
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(log)
    {
        let _ = writeln!(f, "{line}");
    }
}
