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
    pub sandbox: String,
    /// SHA-256 over the exact files that were checked (path + content hash, sorted).
    pub tree_sha256: String,
    /// Folders linked from the worker's tree (not part of the checked snapshot).
    pub linked: Vec<String>,
    pub protected_changes: Vec<ProtectedChange>,
    pub checks: Vec<CheckResult>,
    pub verdict: Verdict,
    pub reasons: Vec<String>,
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
                reasons.push(format!("check \"{}\" failed (exit {code})", r.name));
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
            s.push_str(&format!(
                "  [{mark}] {}  ({} ms)  $ {}\n",
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
        }
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
            sandbox: "none".into(),
            tree_sha256: "t".into(),
            linked: vec![],
            protected_changes: vec![],
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
}
