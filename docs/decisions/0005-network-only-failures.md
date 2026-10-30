# ADR-0005: A check that fails only because the sandbox blocked the internet is UNVERIFIED

Status: **accepted** (2026-10-30, by the maintainer). Backlog CC-19.

## The problem

Checks run with the internet blocked (localhost still works). A check that needs the internet and doesn't say so
(`network = true`) fails, and today that makes the run **FAILED**: "the work is wrong". The receipt adds a hint
(`NO_INTERNET_HINT`, `apps/countercheck/src/runner.rs`), but a hint never changes a verdict.

In the October benchmark that rule produced about 24 FAILEDs per model on correct work, all on the two tasks that
need the internet ([results](../eval/results-2026-10.md), "False alarms": 6.9% overall, 0 on the other 27 tasks). A
FAILED on correct work teaches people to ignore FAILED. Countercheck doesn't know whether that work is right: it
couldn't run the check. By rule 3 of AGENTS.md ("anything Countercheck can't prove is unverified"), that is
UNVERIFIED.

## Why not just read the error text

`looks_like_network_error` reads the check's output, and the output comes from code the agent wrote. If "network
error in the output" softened a verdict, an agent could turn a real FAILED into UNVERIFIED by printing such a line, or
by adding a network call to the code so the check can't run. The hook sends an agent back only on FAILED, so either
trick would end the push-back. Asking the operating system for proof that a connection was refused doesn't work on
every OS (macOS logs sandbox denials only to its unified log; Linux and Windows keep no record).

## Decision

A check's failure counts as **"needs the internet"** only when all of these hold:

1. the sandbox blocks the internet (`Sandbox::blocks_internet`) and the check doesn't have `network = true`;
2. its output looks like a network error (`looks_like_network_error`, as today);
3. **the same check, run on the base revision in its own clean copy and sandbox, also fails with a network error.**
   The base is the locked revision the agent can't change, so this shows the check itself needs the internet. If the
   base run passes, or fails any other way, the agent's change brought the network in: the failure stands.

Then:

- If every failed check "needs the internet", the run is **UNVERIFIED**, with the reason "check X needs the internet,
  which the sandbox blocks; set `network = true` on it or run it in CI". Any other failure keeps the run FAILED.
- The receipt records it per check (`needs_network: true` on the check result, sealed like every field).
- The hook lets the agent stop with the ⚠ UNVERIFIED line (as for every UNVERIFIED); CI's exit code is 3, which still
  fails a job that requires VERIFIED.
- The base run happens only in this case: one extra run of the affected checks, never of the others.

This amends golden rule 3 in one narrow, written way: the network hint still never changes a verdict on its own; a
**second run on the base** can.

## Consequences

- Benchmark internet tasks (checked with `eval/validate.py` after the change): u1_fetch_title's correct solution is
  UNVERIFIED instead of FAILED. u2_status's correct solution stays FAILED: it catches the network error itself, so
  the check's output never mentions the network, and Countercheck doesn't guess from silence. The starting code of
  all three internet tasks (u1, u2, i4_status, whose check calls a name that never resolves) is UNVERIFIED: the check
  can't run, so it can't show the bug either. No task moves to VERIFIED.
- A check that needs the internet costs one extra run when it fails. Projects avoid both by setting `network = true`.
- Not covered: a check that needs a local service that isn't running (a database). That's a different failure and
  stays FAILED.

## Tests (each must fail on the code before the change)

- A check that fetches a URL, unchanged by the agent: UNVERIFIED, `needs_network` on the check, the reason text.
- The agent adds a network call to code that didn't have one, so the check fails with a network error: FAILED (the
  base run doesn't need the internet).
- The agent's code prints a fake network error and exits 1: FAILED.
- One check needs the internet and another fails for real: FAILED.
- `network = true`: no base run; the check runs with the internet as today.
- The seal property test covers `needs_network`.
