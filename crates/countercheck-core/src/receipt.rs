//! A receipt: what a worker claimed, what countercheck checked itself, and the verdict.

use crate::protect::ProtectedChange;
use serde::{Deserialize, Serialize};

pub const SCHEMA: &str = "countercheck.receipt/v0";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Verdict {
    /// Every check passed, and the checks came from the base revision.
    Verified,
    /// At least one check failed or timed out.
    Failed,
    /// Nothing failed, but countercheck can't vouch for it (no locked checks, or none ran).
    Unverified,
}

impl Verdict {
    /// Process exit code: 0 verified, 2 failed, 3 unverified (1 is reserved for errors).
    pub fn exit_code(self) -> i32 {
        match self {
            Verdict::Verified => 0,
            Verdict::Failed => 2,
            Verdict::Unverified => 3,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Verdict::Verified => "VERIFIED",
            Verdict::Failed => "FAILED",
            Verdict::Unverified => "UNVERIFIED",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClaimStatus {
    NoClaim,
    Supported,
    Contradicted,
    NotChecked,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CheckResult {
    pub name: String,
    pub run: String,
    pub exit_code: Option<i32>,
    pub timed_out: bool,
    pub duration_ms: u64,
    pub passed: bool,
    /// Last lines of combined stdout+stderr.
    pub output_tail: String,
    /// SHA-256 of the full combined output.
    pub output_sha256: String,
    /// The check ran with internet access (`network = true`). Absent on older receipts.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub network: bool,
    /// Extra locations the sandbox let this check write to (`writes = [...]`), resolved. Absent on
    /// older receipts.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub writes: Vec<String>,
    /// `Some(false)` when this check ran outside the run's sandbox (Windows: Git for Windows'
    /// `sh` can't start in it), with the reason below. Absent when the check ran in the run's
    /// sandbox (as the run's `sandbox` says) and on older receipts.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sandboxed: Option<bool>,
    /// Why the check ran outside the run's sandbox.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unsandboxed_reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RepoInfo {
    pub base_rev: String,
    pub base_commit: String,
    pub head_commit: String,
    /// The working tree had uncommitted changes when verified.
    pub dirty: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Worker {
    pub agent: Option<String>,
    pub model: Option<String>,
    /// Reported by the caller; countercheck doesn't measure spend yet.
    pub cost_usd: Option<f64>,
    /// What this agent is for, e.g. "UI design" or "security review" (`--role`, or
    /// `COUNTERCHECK_ROLE` in the agent's environment), so several agents on one project can be
    /// told apart. Absent on older receipts (their seals don't change).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    /// The agent session that made the run (from the hook's input), so one session's runs group
    /// together.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session: Option<String>,
}

/// Longest role kept on a receipt (characters).
pub const MAX_ROLE_CHARS: usize = 60;
/// Longest session id kept on a receipt (characters).
pub const MAX_SESSION_CHARS: usize = 128;

/// A label from the caller's environment or hook input, made safe to store and show: control
/// characters removed, whitespace trimmed and collapsed, at most `max` characters. Empty means none.
pub fn clean_label(s: &str, max: usize) -> Option<String> {
    let words: Vec<&str> = s
        .split(|c: char| c.is_whitespace() || c.is_control())
        .filter(|w| !w.is_empty())
        .collect();
    let joined = words.join(" ");
    let cut: String = joined.chars().take(max).collect();
    let cut = cut.trim_end().to_string();
    (!cut.is_empty()).then_some(cut)
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Receipt {
    pub schema: String,
    /// First 16 hex chars of the SHA-256 of this receipt with `id` and `receipt_sha256` empty.
    pub id: String,
    pub receipt_sha256: String,
    pub created_at: String,
    pub countercheck_version: String,
    pub repo: RepoInfo,
    pub worker: Worker,
    pub claim: Option<String>,
    pub claim_status: ClaimStatus,
    /// True when the check file came from the base revision (the worker couldn't edit it).
    pub checks_locked: bool,
    pub checks_source: String,
    /// Where the checks came from when they were outside the repository (`--checks-from`).
    /// Absent otherwise and on older receipts, so their seals still verify.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub checks_external: Option<ExternalChecks>,
    pub sandbox: String,
    /// SHA-256 over the exact files that were checked (path + content hash, sorted).
    pub tree_sha256: String,
    /// Folders linked from the worker's tree (not part of the checked snapshot).
    pub linked: Vec<String>,
    pub protected_changes: Vec<ProtectedChange>,
    pub checks: Vec<CheckResult>,
    /// Changed files no check covers (only when a check declares `covers`). Absent on older
    /// receipts, so their seals still verify.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uncovered: Option<crate::uncovered::Uncovered>,
    /// Changes to watched files (only when the check file has `watched`). Absent on older
    /// receipts, so their seals still verify.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub watched: Option<crate::watched::Watched>,
    pub verdict: Verdict,
    pub reasons: Vec<String>,
}

/// Checks read from outside the repository (`--checks-from`, ADR-0002).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExternalChecks {
    /// `git` (a commit of another repository) or `file` (a file with a SHA-256 pin).
    pub kind: String,
    /// `<repo>@<commit>:<path>` or the file's path.
    pub source: String,
    /// SHA-256 of the check file that was used.
    pub sha256: String,
    /// Pinned by a commit id or `--checks-sha256`. Unpinned checks are not locked.
    pub pinned: bool,
}

/// Decide the verdict and give plain reasons for it.
pub fn decide(
    checks_locked: bool,
    results: &[CheckResult],
    protected: &[ProtectedChange],
) -> (Verdict, Vec<String>) {
    let mut reasons = Vec::new();
    let failed: Vec<&CheckResult> = results.iter().filter(|r| !r.passed).collect();
    let restored = protected
        .iter()
        .filter(|p| p.action == "restored_from_base")
        .count();
    if restored > 0 {
        reasons.push(format!(
            "the worker changed {restored} protected file(s); countercheck ran the checks with the base version"
        ));
    }
    let verdict = if results.is_empty() {
        reasons.push("no checks ran".to_string());
        Verdict::Unverified
    } else if !failed.is_empty() {
        for r in failed {
            if r.timed_out {
                reasons.push(format!("check \"{}\" timed out", r.name));
            } else {
                let code = r.exit_code.map_or("killed".to_string(), |c| c.to_string());
                // Exit 0 but not passed: the command ran fine, its output didn't (golden, budget).
                let why = if r.exit_code == Some(0) {
                    "; its output did not pass"
                } else {
                    ""
                };
                reasons.push(format!("check \"{}\" failed (exit {code}{why})", r.name));
            }
        }
        Verdict::Failed
    } else if !checks_locked {
        reasons.push(
            "the checks are not in the base revision, so the worker could have written them"
                .to_string(),
        );
        Verdict::Unverified
    } else {
        reasons.push(format!(
            "all {} check(s) passed in a clean copy",
            results.len()
        ));
        Verdict::Verified
    };
    (verdict, reasons)
}

pub fn claim_status(claim: Option<&str>, verdict: Verdict) -> ClaimStatus {
    match (claim.map(str::trim).filter(|c| !c.is_empty()), verdict) {
        (None, _) => ClaimStatus::NoClaim,
        (Some(_), Verdict::Verified) => ClaimStatus::Supported,
        (Some(_), Verdict::Failed) => ClaimStatus::Contradicted,
        (Some(_), Verdict::Unverified) => ClaimStatus::NotChecked,
    }
}

impl Receipt {
    /// Fill `id` and `receipt_sha256` from the receipt's own content.
    pub fn seal(mut self) -> Result<Self, String> {
        self.id = String::new();
        self.receipt_sha256 = String::new();
        let body = serde_json::to_vec(&self).map_err(|e| format!("receipt: {e}"))?;
        let hash = crate::sha256_hex(&body);
        self.id = hash.get(..16).unwrap_or(&hash).to_string();
        self.receipt_sha256 = hash;
        Ok(self)
    }

    /// Recompute the seal and compare: true when the receipt hasn't been edited.
    pub fn seal_is_valid(&self) -> bool {
        match self.clone().seal() {
            Ok(r) => r.receipt_sha256 == self.receipt_sha256 && r.id == self.id,
            Err(_) => false,
        }
    }

    /// Human summary for the terminal.
    pub fn render_text(&self) -> String {
        let mut s = String::new();
        s.push_str(&format!(
            "countercheck receipt {}  {}\n",
            self.id,
            self.verdict.label()
        ));
        if let Some(c) = &self.claim {
            let status = match self.claim_status {
                ClaimStatus::Supported => "supported by evidence",
                ClaimStatus::Contradicted => "CONTRADICTED by evidence",
                ClaimStatus::NotChecked => "not proven",
                ClaimStatus::NoClaim => "",
            };
            s.push_str(&format!("  claim:    \"{c}\" -> {status}\n"));
        }
        s.push_str(&format!(
            "  base:     {} ({})   worker tree: {}{}\n",
            self.repo.base_rev,
            short(&self.repo.base_commit),
            short(&self.repo.head_commit),
            if self.repo.dirty {
                " + uncommitted changes"
            } else {
                ""
            }
        ));
        s.push_str(&format!(
            "  checks:   {} ({}), sandbox: {}\n",
            self.checks_source,
            if self.checks_locked {
                "locked"
            } else {
                "NOT locked"
            },
            self.sandbox
        ));
        if !self.linked.is_empty() {
            s.push_str(&format!(
                "  linked:   {} (from the worker's tree, read-only in the sandbox)\n",
                self.linked.join(", ")
            ));
        }
        for p in &self.protected_changes {
            s.push_str(&format!(
                "  protected {:?}: {} -> {}\n",
                p.change, p.path, p.action
            ));
        }
        for r in &self.checks {
            let mark = if r.passed { "pass" } else { "FAIL" };
            let net = if r.network { ", internet allowed" } else { "" };
            let writes = if r.writes.is_empty() {
                String::new()
            } else {
                format!(", writes allowed: {}", r.writes.join(", "))
            };
            let unsandboxed = if r.sandboxed == Some(false) {
                ", NOT sandboxed"
            } else {
                ""
            };
            s.push_str(&format!(
                "  [{mark}] {}  ({} ms{net}{writes}{unsandboxed})  $ {}\n",
                r.name, r.duration_ms, r.run
            ));
            if !r.passed {
                for line in r
                    .output_tail
                    .lines()
                    .rev()
                    .take(8)
                    .collect::<Vec<_>>()
                    .iter()
                    .rev()
                {
                    s.push_str(&format!("         | {line}\n"));
                }
            }
        }
        if let Some(u) = self.uncovered.as_ref().filter(|u| u.total > 0) {
            let more = u.total.saturating_sub(u.paths.len());
            s.push_str(&format!(
                "  uncovered: {} changed file(s) no check covers: {}{}\n",
                u.total,
                u.paths.join(", "),
                if more > 0 {
                    format!(", and {more} more")
                } else {
                    String::new()
                }
            ));
        }
        if let Some(w) = self.watched.as_ref().filter(|w| w.total > 0) {
            s.push_str(&format!(
                "  watched:  {} file(s) changed ({} existing modified or deleted); review them:\n",
                w.total, w.existing_changed
            ));
            for c in &w.changes {
                s.push_str(&format!("            {}\n", c.describe()));
            }
            let more = w.total.saturating_sub(w.changes.len());
            if more > 0 {
                s.push_str(&format!("            and {more} more\n"));
            }
        }
        if let Some(role) = &self.worker.role {
            s.push_str(&format!("  role:     {role}\n"));
        }
        if let Some(cost) = self.worker.cost_usd {
            s.push_str(&format!("  cost:     ${cost:.4} (reported)\n"));
        }
        for r in &self.reasons {
            s.push_str(&format!("  - {r}\n"));
        }
        s
    }
}

fn short(sha: &str) -> &str {
    sha.get(..10).unwrap_or(sha)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protect::{Change, ProtectedChange};

    fn result(passed: bool) -> CheckResult {
        CheckResult {
            name: "t".into(),
            run: "x".into(),
            exit_code: Some(if passed { 0 } else { 1 }),
            timed_out: false,
            duration_ms: 1,
            passed,
            output_tail: String::new(),
            output_sha256: String::new(),
            network: false,
            writes: Vec::new(),
            sandboxed: None,
            unsandboxed_reason: None,
        }
    }

    fn receipt() -> Receipt {
        Receipt {
            schema: SCHEMA.into(),
            id: String::new(),
            receipt_sha256: String::new(),
            created_at: "2026-10-18T00:00:00Z".into(),
            countercheck_version: "0".into(),
            repo: RepoInfo {
                base_rev: "HEAD".into(),
                base_commit: "0123456789abcdef".into(),
                head_commit: "fedcba9876543210".into(),
                dirty: false,
            },
            worker: Worker::default(),
            claim: None,
            claim_status: ClaimStatus::NoClaim,
            checks_locked: true,
            checks_source: "src".into(),
            checks_external: None,
            sandbox: "none".into(),
            tree_sha256: "t".into(),
            linked: vec![],
            protected_changes: vec![],
            uncovered: None,
            watched: None,
            checks: vec![],
            verdict: Verdict::Unverified,
            reasons: vec![],
        }
    }

    #[test]
    fn exit_codes_and_labels() {
        assert_eq!(Verdict::Verified.exit_code(), 0);
        assert_eq!(Verdict::Failed.exit_code(), 2);
        assert_eq!(Verdict::Unverified.exit_code(), 3);
        assert_eq!(Verdict::Verified.label(), "VERIFIED");
        assert_eq!(Verdict::Failed.label(), "FAILED");
        assert_eq!(Verdict::Unverified.label(), "UNVERIFIED");
    }

    #[test]
    fn only_restored_files_are_reported_as_protected_changes() {
        for p in [
            vec![],
            vec![ProtectedChange::decide("tests/new", Change::Added)],
        ] {
            let (v, reasons) = decide(true, &[result(true)], &p);
            assert_eq!(v, Verdict::Verified);
            assert_eq!(reasons, vec!["all 1 check(s) passed in a clean copy"]);
        }
    }

    #[test]
    fn seal_checks_the_id_too() {
        let r = receipt().seal().unwrap();
        let mut edited = r.clone();
        edited.id = "0000000000000000".into();
        assert!(!edited.seal_is_valid());
        let mut edited = r.clone();
        edited.receipt_sha256 = "0".repeat(64);
        assert!(!edited.seal_is_valid());
    }

    #[test]
    fn text_shows_the_evidence() {
        let mut r = receipt();
        r.verdict = Verdict::Failed;
        let mut failing = result(false);
        failing.name = "unit".into();
        failing.output_tail = "line one\nAssertionError: boom".into();
        let mut passing = result(true);
        passing.name = "lint".into();
        passing.output_tail = "quiet on success".into();
        r.checks = vec![failing, passing];
        let text = r.clone().seal().unwrap().render_text();
        assert!(text.starts_with("countercheck receipt "), "{text}");
        assert!(text.contains("FAILED"), "{text}");
        assert!(text.contains("HEAD (0123456789)"), "{text}");
        assert!(text.contains("worker tree: fedcba9876\n"), "{text}");
        assert!(text.contains("[FAIL] unit"), "{text}");
        assert!(text.contains("| AssertionError: boom"), "{text}");
        assert!(text.contains("[pass] lint"), "{text}");
        assert!(!text.contains("quiet on success"), "{text}");
        assert!(!text.contains("linked:"), "{text}");
        r.linked = vec!["node_modules".into()];
        assert!(r.render_text().contains("linked:   node_modules"));
    }

    #[test]
    fn verdicts() {
        assert_eq!(decide(true, &[result(true)], &[]).0, Verdict::Verified);
        assert_eq!(
            decide(true, &[result(true), result(false)], &[]).0,
            Verdict::Failed
        );
        assert_eq!(decide(false, &[result(true)], &[]).0, Verdict::Unverified);
        assert_eq!(decide(true, &[], &[]).0, Verdict::Unverified);
        assert_eq!(decide(false, &[result(false)], &[]).0, Verdict::Failed);
    }

    #[test]
    fn a_failed_output_check_says_so() {
        let mut r = result(false);
        r.exit_code = Some(0);
        let (v, reasons) = decide(true, &[r], &[]);
        assert_eq!(v, Verdict::Failed);
        assert_eq!(
            reasons,
            vec!["check \"t\" failed (exit 0; its output did not pass)"]
        );
        let (_, reasons) = decide(true, &[result(false)], &[]);
        assert_eq!(reasons, vec!["check \"t\" failed (exit 1)"]);
    }

    #[test]
    fn restored_protected_files_are_reported() {
        let p = vec![ProtectedChange::decide("tests/a", Change::Modified)];
        let (_, reasons) = decide(true, &[result(true)], &p);
        assert!(reasons[0].contains("1 protected file"));
    }

    #[test]
    fn claims_follow_the_verdict() {
        assert_eq!(claim_status(None, Verdict::Verified), ClaimStatus::NoClaim);
        assert_eq!(
            claim_status(Some("  "), Verdict::Failed),
            ClaimStatus::NoClaim
        );
        assert_eq!(
            claim_status(Some("done"), Verdict::Failed),
            ClaimStatus::Contradicted
        );
        assert_eq!(
            claim_status(Some("done"), Verdict::Verified),
            ClaimStatus::Supported
        );
    }

    #[test]
    fn seal_detects_edits() {
        let r = Receipt {
            schema: SCHEMA.into(),
            id: String::new(),
            receipt_sha256: String::new(),
            created_at: "2026-10-18T00:00:00Z".into(),
            countercheck_version: "0".into(),
            repo: RepoInfo {
                base_rev: "HEAD".into(),
                base_commit: "a".into(),
                head_commit: "a".into(),
                dirty: true,
            },
            worker: Worker::default(),
            claim: Some("done".into()),
            claim_status: ClaimStatus::Contradicted,
            checks_locked: true,
            checks_source: "x".into(),
            checks_external: None,
            sandbox: "none".into(),
            tree_sha256: "t".into(),
            linked: vec![],
            protected_changes: vec![],
            uncovered: None,
            watched: None,
            checks: vec![result(false)],
            verdict: Verdict::Failed,
            reasons: vec![],
        }
        .seal()
        .unwrap();
        assert_eq!(r.id.len(), 16);
        assert!(r.seal_is_valid());
        let mut edited = r.clone();
        edited.verdict = Verdict::Verified;
        assert!(!edited.seal_is_valid());
    }

    #[test]
    fn seal_survives_reading_the_receipt_back() {
        // Costs with many digits once read back one bit off, so `show` called a fresh receipt BROKEN.
        for cost in [
            0.000_108_000_000_000_000_01,
            5.669_748_844_129_758e178,
            0.1 + 0.2,
        ] {
            let w = Worker {
                cost_usd: Some(cost),
                ..Worker::default()
            };
            let r = Receipt {
                schema: SCHEMA.into(),
                id: String::new(),
                receipt_sha256: String::new(),
                created_at: String::new(),
                countercheck_version: String::new(),
                repo: RepoInfo {
                    base_rev: String::new(),
                    base_commit: String::new(),
                    head_commit: String::new(),
                    dirty: false,
                },
                worker: w,
                claim: None,
                claim_status: ClaimStatus::NoClaim,
                checks_locked: false,
                checks_source: String::new(),
                checks_external: None,
                sandbox: String::new(),
                tree_sha256: String::new(),
                linked: vec![],
                protected_changes: vec![],
                uncovered: None,
                watched: None,
                checks: vec![],
                verdict: Verdict::Unverified,
                reasons: vec![],
            }
            .seal()
            .unwrap();
            let back: Receipt =
                serde_json::from_str(&serde_json::to_string_pretty(&r).unwrap()).unwrap();
            assert!(back.seal_is_valid(), "cost {cost}");
        }
    }

    #[test]
    fn labels_are_trimmed_flattened_and_capped() {
        assert_eq!(
            clean_label("  UI   design\n", 60).as_deref(),
            Some("UI design")
        );
        assert_eq!(clean_label("a\u{1b}[31mb", 60).as_deref(), Some("a [31mb"));
        assert_eq!(clean_label(" \t\n", 60), None);
        assert_eq!(clean_label("abcdef", 3).as_deref(), Some("abc"));
        assert_eq!(clean_label("ab cd", 3).as_deref(), Some("ab"));
        assert_eq!(MAX_ROLE_CHARS, 60);
        assert_eq!(MAX_SESSION_CHARS, 128);
    }

    #[test]
    fn a_role_is_printed_and_receipts_without_one_keep_their_old_json() {
        let mut r = receipt();
        let before = serde_json::to_string(&r).unwrap_or_default();
        assert!(
            !before.contains("\"role\"") && !before.contains("\"session\""),
            "{before}"
        );
        assert!(!r.render_text().contains("role:"));
        r.worker.role = Some("Security review".into());
        assert!(r.render_text().contains("  role:     Security review\n"));
    }
}
