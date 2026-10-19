//! Property tests for the verdict logic: the seal, `decide`, protected paths, the check-file parser
//! and the blocker reader. These run thousands of generated inputs against rules that must always hold.

use countercheck_core::claim::reports_blocker;
use countercheck_core::config::{self, ALWAYS_PROTECTED, MAX_CHECKS_BYTES, MAX_TIMEOUT_SECS};
use countercheck_core::protect::{Change, Protected, ProtectedChange};
use countercheck_core::receipt::{
    self, CheckResult, ClaimStatus, Receipt, RepoInfo, SCHEMA, Verdict, Worker,
};
use proptest::prelude::*;

// ---- generators ----

fn text() -> impl Strategy<Value = String> {
    // Mostly short, with unicode, quotes and control characters mixed in.
    prop_oneof![
        "[a-z0-9 ]{0,12}",
        any::<String>().prop_map(|s| s.chars().take(24).collect()),
        Just("\"\\\n\t\u{0}é漢".to_string()),
    ]
}

fn check_result() -> impl Strategy<Value = CheckResult> {
    (
        text(),
        text(),
        proptest::option::of(any::<i32>()),
        any::<bool>(),
        any::<u64>(),
        any::<bool>(),
        text(),
        "[0-9a-f]{0,64}",
    )
        .prop_map(
            |(name, run, exit_code, timed_out, duration_ms, passed, output_tail, output_sha256)| {
                CheckResult {
                    name,
                    run,
                    exit_code,
                    timed_out,
                    duration_ms,
                    passed,
                    output_tail,
                    output_sha256,
                }
            },
        )
}

fn change() -> impl Strategy<Value = Change> {
    prop_oneof![
        Just(Change::Added),
        Just(Change::Modified),
        Just(Change::Deleted)
    ]
}

fn protected_change() -> impl Strategy<Value = ProtectedChange> {
    ("[a-z/._]{1,16}", change()).prop_map(|(p, c)| ProtectedChange::decide(&p, c))
}

fn verdict() -> impl Strategy<Value = Verdict> {
    prop_oneof![
        Just(Verdict::Verified),
        Just(Verdict::Failed),
        Just(Verdict::Unverified)
    ]
}

fn claim_status() -> impl Strategy<Value = ClaimStatus> {
    prop_oneof![
        Just(ClaimStatus::NoClaim),
        Just(ClaimStatus::Supported),
        Just(ClaimStatus::Contradicted),
        Just(ClaimStatus::NotChecked)
    ]
}

/// Finite costs only: JSON has no NaN or infinity (serde_json writes them as `null`).
fn cost() -> impl Strategy<Value = Option<f64>> {
    proptest::option::of(prop_oneof![
        -1e9f64..1e9f64,
        Just(0.0),
        Just(-0.0),
        Just(f64::MAX),
        Just(f64::MIN_POSITIVE),
        any::<f64>().prop_filter("finite", |c| c.is_finite()),
    ])
}

fn receipt() -> impl Strategy<Value = Receipt> {
    (
        (text(), text(), text(), text(), text(), any::<bool>()),
        (
            proptest::option::of(text()),
            proptest::option::of(text()),
            cost(),
        ),
        (proptest::option::of(text()), claim_status(), any::<bool>()),
        (
            text(),
            text(),
            text(),
            proptest::collection::vec(text(), 0..3),
        ),
        (
            proptest::collection::vec(protected_change(), 0..3),
            proptest::collection::vec(check_result(), 0..3),
            verdict(),
            proptest::collection::vec(text(), 0..3),
        ),
    )
        .prop_map(
            |(
                (created_at, version, base_rev, base_commit, head_commit, dirty),
                (agent, model, cost_usd),
                (claim, claim_status, checks_locked),
                (checks_source, sandbox, tree_sha256, linked),
                (protected_changes, checks, verdict, reasons),
            )| Receipt {
                schema: SCHEMA.to_string(),
                id: String::new(),
                receipt_sha256: String::new(),
                created_at,
                countercheck_version: version,
                repo: RepoInfo {
                    base_rev,
                    base_commit,
                    head_commit,
                    dirty,
                },
                worker: Worker {
                    agent,
                    model,
                    cost_usd,
                },
                claim,
                claim_status,
                checks_locked,
                checks_source,
                sandbox,
                tree_sha256,
                linked,
                protected_changes,
                checks,
                verdict,
                reasons,
            },
        )
}

/// A string guaranteed to differ from `s`.
fn other(s: &str) -> String {
    format!("{s}x")
}

fn other_verdict(v: Verdict) -> Verdict {
    match v {
        Verdict::Verified => Verdict::Failed,
        Verdict::Failed => Verdict::Unverified,
        Verdict::Unverified => Verdict::Verified,
    }
}

fn other_status(s: ClaimStatus) -> ClaimStatus {
    match s {
        ClaimStatus::NoClaim => ClaimStatus::Supported,
        ClaimStatus::Supported => ClaimStatus::Contradicted,
        ClaimStatus::Contradicted => ClaimStatus::NotChecked,
        ClaimStatus::NotChecked => ClaimStatus::NoClaim,
    }
}

fn other_change(c: Change) -> Change {
    match c {
        Change::Added => Change::Modified,
        Change::Modified => Change::Deleted,
        Change::Deleted => Change::Added,
    }
}

fn other_opt(o: &Option<String>) -> Option<String> {
    match o {
        None => Some(String::new()),
        Some(s) => Some(other(s)),
    }
}

fn first_check(r: &mut Receipt) -> &mut CheckResult {
    if r.checks.is_empty() {
        r.checks.push(CheckResult {
            name: String::new(),
            run: String::new(),
            exit_code: None,
            timed_out: false,
            duration_ms: 0,
            passed: false,
            output_tail: String::new(),
            output_sha256: String::new(),
        });
        return &mut r.checks[0];
    }
    &mut r.checks[0]
}

/// Number of single-field edits `edit` knows how to make.
const EDITS: usize = 32;

/// Change exactly one field (or one part of a nested field) of a sealed receipt.
fn edit(r: &mut Receipt, which: usize) {
    match which {
        0 => r.schema = other(&r.schema),
        1 => r.id = other(&r.id),
        2 => r.receipt_sha256 = other(&r.receipt_sha256),
        3 => r.created_at = other(&r.created_at),
        4 => r.countercheck_version = other(&r.countercheck_version),
        5 => r.repo.base_rev = other(&r.repo.base_rev),
        6 => r.repo.base_commit = other(&r.repo.base_commit),
        7 => r.repo.head_commit = other(&r.repo.head_commit),
        8 => r.repo.dirty = !r.repo.dirty,
        9 => r.worker.agent = other_opt(&r.worker.agent),
        10 => r.worker.model = other_opt(&r.worker.model),
        11 => {
            r.worker.cost_usd = match r.worker.cost_usd {
                None => Some(0.0),
                Some(0.0) => Some(1.0),
                Some(c) => Some(c * 2.0).filter(|d| d.is_finite() && *d != c).or(None),
            }
        }
        12 => r.claim = other_opt(&r.claim),
        13 => r.claim_status = other_status(r.claim_status),
        14 => r.checks_locked = !r.checks_locked,
        15 => r.checks_source = other(&r.checks_source),
        16 => r.sandbox = other(&r.sandbox),
        17 => r.tree_sha256 = other(&r.tree_sha256),
        18 => r.linked.push(String::new()),
        19 => r
            .protected_changes
            .push(ProtectedChange::decide("p", Change::Added)),
        20 => match r.protected_changes.first_mut() {
            Some(p) => p.change = other_change(p.change),
            None => r
                .protected_changes
                .push(ProtectedChange::decide("p", Change::Deleted)),
        },
        21 => r.checks.push(first_check(&mut r.clone()).clone()),
        22 => first_check(r).name.push('x'),
        23 => first_check(r).run.push('x'),
        24 => {
            let c = first_check(r);
            c.exit_code = match c.exit_code {
                None => Some(0),
                Some(_) => None,
            }
        }
        25 => {
            let c = first_check(r);
            c.timed_out = !c.timed_out
        }
        26 => {
            let c = first_check(r);
            c.duration_ms = c.duration_ms.wrapping_add(1)
        }
        27 => {
            let c = first_check(r);
            c.passed = !c.passed
        }
        28 => first_check(r).output_tail.push('x'),
        29 => first_check(r).output_sha256.push('x'),
        30 => r.verdict = other_verdict(r.verdict),
        _ => r.reasons.push(String::new()),
    }
}

// ---- receipt: seal ----

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    #[test]
    fn a_sealed_receipt_is_valid(r in receipt()) {
        let sealed = r.seal().map_err(TestCaseError::fail)?;
        prop_assert!(sealed.seal_is_valid());
        prop_assert_eq!(sealed.receipt_sha256.len(), 64);
        prop_assert_eq!(&sealed.id, &sealed.receipt_sha256[..16]);
    }

    #[test]
    fn sealing_ignores_the_old_seal(r in receipt(), id in text(), sha in text()) {
        let a = r.clone().seal().map_err(TestCaseError::fail)?;
        let mut r = r;
        r.id = id;
        r.receipt_sha256 = sha;
        let b = r.seal().map_err(TestCaseError::fail)?;
        prop_assert_eq!(a, b);
    }

    #[test]
    fn any_edit_breaks_the_seal(r in receipt(), which in 0..EDITS) {
        let sealed = r.seal().map_err(TestCaseError::fail)?;
        let mut edited = sealed.clone();
        edit(&mut edited, which);
        // Field 11 can't always find a different finite cost; skip only that rare case.
        prop_assume!(edited != sealed);
        prop_assert!(!edited.seal_is_valid(), "edit {} kept the seal valid", which);
    }

    /// `countercheck show` reads a receipt back from JSON: the seal must survive the round trip.
    #[test]
    fn the_seal_survives_a_json_round_trip(r in receipt()) {
        let sealed = r.seal().map_err(TestCaseError::fail)?;
        let json = serde_json::to_string_pretty(&sealed).map_err(|e| TestCaseError::fail(e.to_string()))?;
        let back: Receipt = serde_json::from_str(&json).map_err(|e| TestCaseError::fail(e.to_string()))?;
        prop_assert!(back.seal_is_valid(), "{}", json);
    }
}

// ---- receipt: decide and claim_status ----

proptest! {
    #![proptest_config(ProptestConfig::with_cases(1024))]

    #[test]
    fn decide_follows_the_rules(
        locked in any::<bool>(),
        results in proptest::collection::vec(check_result(), 0..5),
        protected in proptest::collection::vec(protected_change(), 0..5),
    ) {
        let (v, reasons) = receipt::decide(locked, &results, &protected);
        let any_failed = results.iter().any(|r| !r.passed);
        let expected = if results.is_empty() {
            Verdict::Unverified
        } else if any_failed {
            Verdict::Failed
        } else if !locked {
            Verdict::Unverified
        } else {
            Verdict::Verified
        };
        prop_assert_eq!(v, expected);
        // Never VERIFIED when any check failed, the checks weren't locked, or none ran.
        if v == Verdict::Verified {
            prop_assert!(locked && !results.is_empty() && !any_failed);
        }
        // There is always a reason, and each failed check is named once.
        prop_assert!(!reasons.is_empty());
        if v == Verdict::Failed {
            let failed = results.iter().filter(|r| !r.passed).count();
            let named = reasons
                .iter()
                .filter(|r| r.starts_with("check \"") && (r.ends_with("timed out") || r.contains("failed (exit ")))
                .count();
            prop_assert_eq!(named, failed);
        }
        // A protected file put back from base is always on the receipt, whatever the verdict.
        let restored = protected.iter().filter(|p| p.action == "restored_from_base").count();
        let says_restored = reasons.iter().any(|r| r.contains("protected file"));
        prop_assert_eq!(says_restored, restored > 0);
        if restored > 0 {
            let restored_reason = format!("the worker changed {restored} protected file(s)");
            prop_assert!(reasons[0].starts_with(&restored_reason), "{:?}", reasons);
        }
    }

    #[test]
    fn claim_status_follows_the_verdict(claim in proptest::option::of(text()), v in verdict()) {
        let s = receipt::claim_status(claim.as_deref(), v);
        let has_claim = claim.as_deref().is_some_and(|c| !c.trim().is_empty());
        let expected = match (has_claim, v) {
            (false, _) => ClaimStatus::NoClaim,
            (true, Verdict::Verified) => ClaimStatus::Supported,
            (true, Verdict::Failed) => ClaimStatus::Contradicted,
            (true, Verdict::Unverified) => ClaimStatus::NotChecked,
        };
        prop_assert_eq!(s, expected);
    }
}

// ---- protect ----

fn segment() -> impl Strategy<Value = String> {
    prop_oneof![
        4 => "[a-z_]{1,6}",
        1 => Just("tests".to_string()),
        1 => Just(".countercheck".to_string()),
        1 => Just("x.py".to_string()),
    ]
}

fn rel_path() -> impl Strategy<Value = String> {
    proptest::collection::vec(segment(), 1..5).prop_map(|s| s.join("/"))
}

fn patterns() -> impl Strategy<Value = Vec<String>> {
    proptest::collection::vec(
        prop_oneof![
            Just("tests/**".to_string()),
            Just("**/x.py".to_string()),
            Just("**".to_string()),
            Just("*".to_string()),
            Just("src/*.py".to_string()),
            Just(ALWAYS_PROTECTED.to_string()),
            rel_path(),
        ],
        0..4,
    )
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(1024))]

    #[test]
    fn a_leading_dot_slash_does_not_change_matching(pats in patterns(), p in rel_path()) {
        let set = Protected::new(&pats).map_err(TestCaseError::fail)?;
        prop_assert_eq!(set.matches(&p), set.matches(&format!("./{p}")), "{}", p);
        prop_assert_eq!(set.matches(&p), set.matches(&format!("././{p}")), "{}", p);
    }

    #[test]
    fn dot_dot_escapes_never_match(pats in patterns(), p in rel_path(), ups in 1usize..3) {
        let set = Protected::new(&pats).map_err(TestCaseError::fail)?;
        let escape = format!("{}{p}", "../".repeat(ups));
        prop_assert!(!set.matches(&escape), "{}", escape);
        let inner = format!("a/{}{p}", "../".repeat(ups + 1));
        prop_assert!(!set.matches(&inner), "{}", inner);
    }

    #[test]
    fn dot_dot_inside_the_repo_resolves(pats in patterns(), p in rel_path()) {
        let set = Protected::new(&pats).map_err(TestCaseError::fail)?;
        prop_assert_eq!(set.matches(&p), set.matches(&format!("zz/../{p}")), "{}", p);
    }

    #[test]
    fn always_protected_covers_the_check_file(pats in patterns(), p in rel_path()) {
        let file = config::ChecksFile { protected: pats, ..Default::default() };
        let set = Protected::new(&file.protected_patterns()).map_err(TestCaseError::fail)?;
        prop_assert!(set.matches(config::CHECKS_PATH));
        let inside = format!(".countercheck/{p}");
        prop_assert!(set.matches(&inside));
    }

    #[test]
    fn junk_is_by_segment(prefix in rel_path(), name in "[a-z_]{1,8}") {
        use countercheck_core::protect::is_junk;
        let junk = [
            format!("{prefix}/__pycache__/{name}"),
            format!("{prefix}/.DS_Store"),
            format!("{prefix}/{name}.pyc"),
        ];
        for j in &junk {
            prop_assert!(is_junk(j), "{}", j);
        }
        // Lookalikes are not junk.
        let fine = [
            format!("{prefix}/{name}__pycache__"),
            format!("{prefix}/{name}.DS_Store"),
            format!("{prefix}/{name}.py"),
        ];
        for f in &fine {
            prop_assert!(!is_junk(f), "{}", f);
        }
    }

    #[test]
    fn protected_change_actions(p in rel_path(), c in change()) {
        let pc = ProtectedChange::decide(&p, c);
        prop_assert_eq!(&pc.path, &p);
        prop_assert_eq!(pc.change, c);
        let want = if c == Change::Added { "kept_new_file" } else { "restored_from_base" };
        prop_assert_eq!(pc.action, want);
    }
}

// ---- config ----

/// Pieces that make up plausible (and broken) check files.
fn toml_piece() -> impl Strategy<Value = String> {
    prop_oneof![
        Just("[[check]]\n".to_string()),
        "name = \"[a-z ]{0,4}\"\n",
        "run = \"[a-z ]{0,4}\"\n",
        (0u64..8000).prop_map(|t| format!("timeout_secs = {t}\n")),
        Just("timeout_secs = -1\n".to_string()),
        Just("timeout_secs = 99999999999999999999\n".to_string()),
        "protected = \\[\"[a-z*/\\[\\]{},.]{0,6}\"\\]\n",
        "link = \\[\"[a-z./*\\\\]{0,6}\"\\]\n",
        Just("extra = 1\n".to_string()),
        Just("[check]\n".to_string()),
        Just("\"".to_string()),
        any::<String>().prop_map(|s| s.chars().take(8).collect()),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(2048))]

    #[test]
    fn parse_never_panics_on_any_text(s in any::<String>()) {
        let _ = config::parse(&s);
    }

    #[test]
    fn parse_never_panics_and_accepts_only_valid_files(
        pieces in proptest::collection::vec(toml_piece(), 0..12)
    ) {
        let text = pieces.concat();
        if let Ok(f) = config::parse(&text) {
            prop_assert!(!f.checks.is_empty());
            let mut names = std::collections::BTreeSet::new();
            for c in &f.checks {
                prop_assert!(!c.name.trim().is_empty());
                prop_assert!(!c.run.trim().is_empty());
                prop_assert!(c.timeout_secs >= 1 && c.timeout_secs <= MAX_TIMEOUT_SECS);
                prop_assert!(names.insert(c.name.clone()), "duplicate {}", c.name);
            }
            for l in &f.link {
                prop_assert!(!l.is_empty() && !l.starts_with('/') && !l.contains(".."), "{}", l);
                prop_assert!(!l.contains(['*', '?', '[', '\\']), "{}", l);
                prop_assert!(!l.starts_with(".git") && !l.starts_with(".countercheck"), "{}", l);
            }
            let pats = f.protected_patterns();
            prop_assert_eq!(pats.iter().filter(|p| *p == ALWAYS_PROTECTED).count(), 1);
            prop_assert!(Protected::new(&pats).is_ok());
        }
    }

    #[test]
    fn protected_patterns_keep_the_file_patterns(pats in proptest::collection::vec("[a-z*/.]{0,8}", 0..5)) {
        let file = config::ChecksFile { protected: pats.clone(), ..Default::default() };
        let out = file.protected_patterns();
        prop_assert_eq!(&out[..pats.len()], &pats[..]);
        prop_assert!(out.iter().any(|p| p == ALWAYS_PROTECTED));
        prop_assert!(out.len() == pats.len() || out.len() == pats.len() + 1);
    }
}

#[test]
fn parse_refuses_oversized_files_before_reading_them() {
    let ok = "[[check]]\nname = \"a\"\nrun = \"x\"\n";
    let padded = format!("{ok}{}", "#".repeat(MAX_CHECKS_BYTES - ok.len()));
    assert_eq!(padded.len(), MAX_CHECKS_BYTES);
    assert!(config::parse(&padded).is_ok());
    let over = format!("{padded}#");
    assert!(config::parse(&over).unwrap_err().contains("larger than"));
}

// ---- claim ----

const SUCCESS: &[&str] = &[
    "all tests pass",
    "all tests now pass",
    "tests now pass",
    "all checks pass",
    "everything passes",
    "suite is green",
    "reports ok",
    "tests passing",
];

fn any_text() -> impl Strategy<Value = String> {
    prop_oneof![
        any::<String>(),
        // Text built from blocker and negation words, to reach the deeper branches.
        proptest::collection::vec(
            prop_oneof![
                Just("can't pass "),
                Just("not done "),
                Just("impossible "),
                Just("not "),
                Just("n't "),
                Just(" of "),
                Just("/"),
                Just("’"),
                Just("`"),
                Just("all tests pass "),
                Just("İ"),
                Just("ß"),
                Just("😀"),
                Just("x "),
            ],
            0..20
        )
        .prop_map(|w| w.concat()),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(2048))]

    #[test]
    fn reports_blocker_never_panics(s in any_text()) {
        let _ = reports_blocker(&s);
    }

    /// A success claim with no negator, count or fraction in the 40 characters before it always
    /// wins, whatever else the text says.
    #[test]
    fn an_unnegated_success_claim_is_never_a_blocker(
        before in any_text(),
        after in any_text(),
        phrase in proptest::sample::select(SUCCESS),
        upper in any::<bool>(),
    ) {
        let phrase = if upper { phrase.to_uppercase() } else { phrase.to_string() };
        let text = format!("{before}. {}{phrase}{after}", "z".repeat(48));
        prop_assert!(!reports_blocker(&text), "{:?}", text);
    }

    /// Without a blocker phrase there is nothing to report.
    #[test]
    fn plain_text_is_never_a_blocker(s in "[a-z ]{0,60}") {
        let words = ["impossible", "contradict", "conflicting", "unreachable", "not ", "n't", "still ", "remain", "blocker", "mutually"];
        prop_assume!(!words.iter().any(|w| s.contains(w)));
        prop_assert!(!reports_blocker(&s));
    }

    #[test]
    fn blocker_phrases_count_in_any_case_and_with_curly_quotes(
        before in "[a-z ]{0,20}",
        after in "[a-z ]{0,20}",
        upper in any::<bool>(),
    ) {
        let words = ["pass", "green", "ok", "everything"];
        prop_assume!(!words.iter().any(|w| before.contains(w) || after.contains(w)));
        for p in ["this can’t pass", "it is impossible", "the tests contradict", "not `done`"] {
            let p = if upper { p.to_uppercase() } else { p.to_string() };
            let text = format!("{before} {p} {after}");
            prop_assert!(reports_blocker(&text), "{:?}", text);
        }
    }
}

// ---- matrix ----

proptest! {
    #![proptest_config(ProptestConfig::with_cases(2048))]

    /// A matrix value goes into `sh -c`: anything that passes validation is shell-inert.
    #[test]
    fn valid_matrix_values_have_no_shell_metacharacters(v in prop_oneof![any::<String>(), "[A-Za-z0-9_.;$`'\" |&-]{0,12}"]) {
        use countercheck_core::matrix::valid_value;
        if valid_value(&v) {
            let meta = [' ', '\t', '\n', ';', '&', '|', '$', '`', '\'', '"', '\\', '(', ')', '<', '>', '*', '?', '[', ']', '{', '}', '~', '#', '!', '=', '/', '%'];
            prop_assert!(!v.contains(meta), "{:?}", v);
            prop_assert!(!v.is_empty() && v.len() <= 128);
        }
    }
}

proptest! {
    // Big inputs: fewer cases.
    #![proptest_config(ProptestConfig::with_cases(64))]

    /// Expansion never produces more than 1000 runs, and every run name is unique.
    #[test]
    fn matrix_plans_are_bounded_and_uniquely_named(
        values in proptest::collection::vec("[A-Za-z0-9_.-]{1,6}", 0..1200),
    ) {
        let list = values.iter().map(|v| format!("\"{v}\"")).collect::<Vec<_>>().join(",");
        let text = format!("[[check]]\nname = \"m\"\nrun = \"x {{k}}\"\nmatrix = {{ k = [{list}] }}\n");
        if let Ok(f) = config::parse(&text) {
            let plan = config::plan(&f, &Default::default()).map_err(TestCaseError::fail)?;
            prop_assert_eq!(plan.len(), values.len());
            prop_assert!(plan.len() <= 1000);
            let names: std::collections::BTreeSet<_> = plan.iter().map(|p| p.name.clone()).collect();
            prop_assert_eq!(names.len(), plan.len());
        }
    }
}

// ---- forbid ----

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    /// A literal pattern counts exactly the lines that contain it; at most 20 are shown, each cut.
    #[test]
    fn forbid_counts_matching_lines(
        lines in proptest::collection::vec(prop_oneof!["[a-c ]{0,8}", any::<String>().prop_map(|s| s.chars().take(300).collect())], 0..60),
        needle in "[a-c]{1,3}",
    ) {
        use countercheck_core::forbid::{ForbidSpec, Rule, Scan, MAX_HITS_SHOWN, MAX_LINE_CHARS};
        let spec = ForbidSpec {
            name: "n".into(),
            pattern: regex::escape(&needle),
            paths: vec!["**".into()],
            message: None,
            case_insensitive: false,
        };
        let rule = Rule::new(&spec).map_err(TestCaseError::fail)?;
        let text = lines.join("\n");
        prop_assume!(!text.contains('\0'));
        let mut scan = Scan::default();
        scan.add_file(&rule, "f", text.as_bytes());
        let want = text.lines().filter(|l| l.contains(&needle)).count();
        prop_assert_eq!(scan.total, want);
        prop_assert!(scan.shown.len() <= MAX_HITS_SHOWN);
        prop_assert_eq!(scan.shown.len(), want.min(MAX_HITS_SHOWN));
        for s in &scan.shown {
            prop_assert!(s.chars().count() <= "f:99: ".len() + MAX_LINE_CHARS + 3, "{}", s);
        }
        let (passed, _) = scan.report(&spec);
        prop_assert_eq!(passed, want == 0);
    }
}

// ---- golden ----

fn csv_text() -> impl Strategy<Value = String> {
    let cell = prop_oneof![
        "[a-z]{0,3}",
        "-?[0-9]{1,3}(\\.[0-9]{1,3})?",
        Just("\"a,b\"".to_string())
    ];
    proptest::collection::vec(proptest::collection::vec(cell, 1..4), 1..8).prop_map(|rows| {
        rows.iter()
            .map(|r| r.join(","))
            .collect::<Vec<_>>()
            .join("\n")
            + "\n"
    })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// Any CSV equals itself, with or without sorting and tolerance.
    #[test]
    fn csv_compare_is_reflexive(a in csv_text(), sort in any::<bool>(), tol in prop_oneof![Just(0.0), 0.0f64..1.0]) {
        let c = countercheck_core::golden::compare_csv(a.as_bytes(), a.as_bytes(), tol, sort);
        prop_assert!(c.equal, "{}", c.report);
        prop_assert!(c.report.is_empty());
    }

    /// Pass or fail doesn't depend on which side is the reference.
    #[test]
    fn csv_compare_is_symmetric(a in csv_text(), b in csv_text(), sort in any::<bool>(), tol in prop_oneof![Just(0.0), 0.0f64..1.0]) {
        use countercheck_core::golden::compare_csv;
        let ab = compare_csv(a.as_bytes(), b.as_bytes(), tol, sort);
        let ba = compare_csv(b.as_bytes(), a.as_bytes(), tol, sort);
        prop_assert_eq!(ab.equal, ba.equal);
        if !ab.equal {
            prop_assert!(!ab.report.is_empty());
        }
    }

    /// Text compare: equal iff the line-ending-normalised texts are equal; never panics.
    #[test]
    fn text_compare_matches_normalised_equality(a in any::<String>(), b in any::<String>()) {
        use countercheck_core::golden::compare_text;
        let norm = |s: &str| s.replace("\r\n", "\n").replace('\r', "\n");
        prop_assert!(compare_text(a.as_bytes(), a.as_bytes()).equal);
        prop_assert_eq!(compare_text(a.as_bytes(), b.as_bytes()).equal, norm(&a) == norm(&b));
    }
}

// ---- budget ----

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    /// The last METRIC line wins, and the verdict is exactly "within [min, max]".
    #[test]
    fn budget_takes_the_last_value_and_honours_the_limits(
        values in proptest::collection::vec(-1000.0f64..1000.0, 1..6),
        noise in "[a-z =]{0,20}",
        min in proptest::option::of(-500.0f64..0.0),
        max in proptest::option::of(0.0f64..500.0),
    ) {
        use countercheck_core::budget::{judge, last_metric, BudgetSpec};
        let text: String = values.iter().map(|v| format!("METRIC t={v}\n{noise}\n")).collect();
        let last = values.last().copied();
        prop_assert_eq!(last_metric(&text, "t"), last);
        prop_assume!(min.is_some() || max.is_some());
        let spec = BudgetSpec { metric: "t".into(), min, max };
        prop_assert!(spec.validate("c").is_ok());
        let v = last.unwrap_or(0.0);
        let want = min.is_none_or(|lo| v >= lo) && max.is_none_or(|hi| v <= hi);
        prop_assert_eq!(judge(&spec, last).0, want);
    }
}
