//! Reading an agent's final message: does it report that the work is NOT done?
//!
//! The hook uses this to decide whether to push back on a FAILED check. An agent that already
//! says "this can't pass, because ..." is telling the truth; pushing it to "keep going" wastes
//! time and can push it into gaming the check. An agent that claims success while the locked
//! checks fail is the case Countercheck exists for.
//!
//! Deterministic phrase matching (hooks must be fast and offline). It errs towards "not a
//! blocker": when in doubt the hook pushes back, which is the safe default. Tested against real
//! agent answers in `tests/fixtures/claims/`.

/// Phrases that say the work isn't (fully) done.
const BLOCKER: &[&str] = &[
    "can't pass",
    "cannot pass",
    "can't both",
    "cannot both",
    "can't be satisfied",
    "cannot be satisfied",
    "can not be satisfied",
    "cannot be legitimately",
    "can't be legitimately",
    "not possible",
    "impossible",
    "contradict",
    "conflicting",
    "mutually exclusive",
    "still fail",
    "still does not",
    "still doesn't",
    "does not pass",
    "doesn't pass",
    "do not pass",
    "don't pass",
    "not passing",
    "not go green",
    "not green",
    "remaining failure",
    "remains failing",
    "one failure remains",
    "failure remains",
    "not completable",
    "not done",
    "isn't done",
    "not complete",
    "not claiming",
    "not going to claim",
    "won't claim",
    "unreachable",
    "a blocker",
    "as a blocker",
    "cannot be met",
    "can't be met",
];

/// Phrases that claim success. Each counts only when not negated just before it
/// ("I'm not going to claim the tests pass" is not a success claim).
const SUCCESS: &[&str] = &[
    "all tests pass",
    "all tests now pass",
    "all the tests pass",
    "tests now pass",
    "tests pass now",
    "both tests now pass",
    "all checks pass",
    "everything passes",
    "suite passes",
    "suite is green",
    "reports ok",
    "tests passing",
];

/// Words that, shortly before a success phrase, negate it.
const NEGATORS: &[&str] = &[
    "not ", "n't ", "never ", "no ", "cannot ", "without ", "claim ", "would ", "if ",
];

/// True when `text` reports that the work is not done and makes no un-negated success claim.
pub fn reports_blocker(text: &str) -> bool {
    let t = text.to_lowercase().replace('’', "'").replace('`', "");
    let blocker = BLOCKER.iter().any(|p| t.contains(p));
    blocker && !claims_success(&t)
}

fn claims_success(t: &str) -> bool {
    SUCCESS.iter().any(|phrase| {
        t.match_indices(phrase).any(|(at, _)| {
            let from = t[..at].char_indices().rev().nth(40).map_or(0, |(i, _)| i);
            let before = &t[from..at];
            // A count like "7 of 8 tests pass" or "2/3" is a partial result, not a success claim.
            let partial = before.contains(" of ") || before.contains('/');
            !partial && !NEGATORS.iter().any(|n| before.contains(n))
        })
    })
}
