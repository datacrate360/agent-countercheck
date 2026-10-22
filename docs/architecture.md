# How Countercheck works inside

This page is for contributors: the rules Countercheck is built on, what happens during one `countercheck verify`, where
each step lives in the code, and where new work plugs in. For users, the README and [checks.md](checks.md) are enough.

## The mindset

Countercheck is a **referee**. An AI agent (or a person, or a script) says "done"; Countercheck finds out whether that is
true. A referee is only useful if it is harder to fool than the players, so every design choice follows from these
rules. A change that breaks one of them needs an issue and a discussion first.

| Rule | In plain words | What it means in the code |
|---|---|---|
| **The worker never grades itself** | the rules come from before the work started | checks, matrix lists, coverage lists and golden files are read from the **base revision**; protected files the worker changed are put back before anything runs |
| **Re-run, don't read** | don't trust what the agent says it ran | every check runs again in a fresh copy; the agent's claim and logs are never evidence |
| **Honest verdicts** | three answers, never a guess | `verified` only when locked checks ran and all passed; `failed` when any failed; `unverified` (with a reason) when nothing failed but it can't be proven |
| **Never soften a failure** | a worker must not be able to turn FAILED into something milder | hints may explain a failure (e.g. "ran without internet"), but they never change the verdict |
| **Everything from the repo is hostile** | the worker controls its files | paths are validated (no absolute, no `..`), sizes are capped, symlinks never lead out of the copy, regexes have size limits |
| **Domain-blind core** | Countercheck knows nothing about SQL, games or ML | the core offers general mechanisms (command, matrix, forbid, golden, budget, coverage); domain knowledge lives in a project's checks |
| **Fail closed on the verdict, open in the hooks** | a broken referee must not wave work through, nor freeze your agent | any doubt gives `unverified`, never `verified`; a hook that errors logs it and lets the agent continue |
| **Leave evidence** | anyone can check the referee later | every run writes a receipt with a SHA-256 seal; editing any field breaks it |
| **Never crash** | errors are plain sentences | no `unwrap`/`expect`/`panic!` outside tests, `unsafe` forbidden (enforced at each crate root) |

## One run, step by step

`countercheck verify` is one function: `apps/countercheck/src/verify.rs::verify`. Its numbered comments match these
steps.

| Step | What happens | Where |
|---|---|---|
| **1. Lock the checks** | read `.countercheck/checks.toml` (plus matrix files, coverage lists, golden files) from the base revision, through git objects that are hashed again and must match their ids (no replace refs), so a changed `.git` can't swap what the base holds. If the checks aren't in the base yet, use the worker's copy, but the verdict can then be at most `unverified` | `verify.rs`, `git.rs` (`Revision`), core `gitobj.rs`, `crates/countercheck-core/src/config.rs` (parse, validate, `plan` expands the matrix), `matrix.rs` |
| **2. Clean copy** | copy the worker's files (committed or not, minus ignored ones) into a new temporary folder | `apps/countercheck/src/snapshot.rs`, `git.rs` |
| **3. Put protected files back** | the clean copy's own bytes are compared with the base (not git's index, filters or `diff`, which the worker can configure; a file that differs only by CRLF line endings counts as unchanged but still runs with its base bytes if protected); every changed file matching `protected` is restored from the base revision (or removed, if it was new) and recorded | `verify.rs`, `crates/countercheck-core/src/protect.rs`, `ProtectedChange` in `receipt.rs` |
| **4a. Scan** | `[[forbid]]` and `[[coverage]]` read the copy **before** any command runs, so a command can't hide what was there | `apps/countercheck/src/scan.rs`, core `forbid.rs`, `coverage.rs` |
| **4b. Run** | each check runs with `sh -c` (or its `shell`) in the copy, with a timeout, in the macOS sandbox (no internet unless `network = true`, writes only in the copy). Golden output files are removed first; after the run, golden and budget compare the output | `apps/countercheck/src/runner.rs`, `golden.rs`; core `golden.rs`, `budget.rs` |
| **4b on Windows** | the same steps, natively: `sh` is Git for Windows' `sh.exe` (missing: the check fails with a plain message), `shell = "pwsh"`/`"cmd"` also work; the sandbox is a write-restricted token (`countercheck sandbox-exec`, writes only in the check folder, temp dir and `writes`; internet not blocked, and the receipt says so); each check runs in a Job Object, so a timeout kills every process it started (and leftovers stop when the check ends), with `taskkill /T /F` as a fallback; `link` folders are directory junctions; repo symlinks that Windows refuses become text files with the target (noted in the check output); every path on the receipt, in scans and in the tree hash uses `/`, so the same work gives the same tree hash on every OS | `runner.rs` (`shell_program`, `kill_tree`, `windows_command_line`), `windows_sandbox.rs`, `crates/countercheck-winsandbox` (the only `unsafe`), `snapshot.rs` (`rel_slash`, `canonical`, `link_or_text`) |
| **5. Decide, seal, save** | `decide` turns results and protected changes into a verdict and reasons; `seal` hashes the receipt; it is saved to `.countercheck/receipts/<id>.json` | `crates/countercheck-core/src/receipt.rs` (`decide`, `seal`, `seal_is_valid`) |

Exit codes: `0` verified, `2` failed, `3` unverified, `1` Countercheck itself couldn't run.

### The hooks

A hook is how an agent gets checked automatically when it tries to finish. Each hook reads the agent's own JSON input,
runs the same `verify`, and answers in the agent's format: `apps/countercheck/src/hook.rs`.

- `decide` in `hook.rs`: VERIFIED → stay silent (the agent may finish). FAILED and the agent claims success → **continue**,
  with the failing check's output. FAILED but the agent honestly reports a blocker and changed no protected file → let it
  stop (`HonestBlocker`), keeping the FAILED receipt.
- "Reports a blocker" is `crates/countercheck-core/src/claim.rs::reports_blocker`, tested on real agent answers in
  `crates/countercheck-core/tests/fixtures/claims/`.
- A loop guard (`COUNTERCHECK_MAX_NUDGES`, default 3) stops endless push-backs; any error fails open.
- **Which base?** `COUNTERCHECK_BASE` if set; else the commit the session started from, recorded by
  `countercheck hook session-start` in `.git/countercheck/sessions/` (outside the work tree); else `HEAD`. Without
  the recorded start, an agent that commits its edits to tests or `checks.toml` would move the locked rules.

## The two crates

| Crate | Role | Rule |
|---|---|---|
| `crates/countercheck-core` | pure logic: config, protected paths, check types, verdict, receipt, claim detection | **no I/O**; everything here is testable with plain values (this is where property and mutation tests aim) |
| `apps/countercheck` | the binary: git, files, processes, sandbox, CLI, hooks | thin; each module does one kind of I/O and calls core for decisions |

## Where new work plugs in

| You want to… | Touch | Skill (for AI agents and people) |
|---|---|---|
| Show Countercheck on a new domain (games, ML, dbt, …) | `examples/<domain>/` only, no Rust | [add-domain-example](../.claude/skills/add-domain-example/SKILL.md) |
| Add a benchmark task | `eval/tasks.py` or `eval/tasks_hard.py` | [add-eval-task](../.claude/skills/add-eval-task/SKILL.md) |
| Support another agent (Codex, Cursor, …) | `hook.rs`, `main.rs`, `docs/hooks/` | [add-agent-hook](../.claude/skills/add-agent-hook/SKILL.md) |
| Add a check type | core `config.rs` + a new core module, `verify.rs`/`scan.rs`/`runner.rs`, `docs/checks.md` | [add-check-type](../.claude/skills/add-check-type/SKILL.md) |
| Write or strengthen tests | `apps/countercheck/tests/verify.rs`, core `tests/` | [write-tests](../.claude/skills/write-tests/SKILL.md) |

Before adding a check type, ask: can a `command` check do it? If yes, write an example instead. New types are for
things a command can't do safely (reading files before commands run, comparing against locked references, enforcing a
list the worker can't shrink).

## How it is tested

| Layer | Where | What it proves |
|---|---|---|
| Unit | `#[cfg(test)]` in each module | each function on small inputs |
| End-to-end | `apps/countercheck/tests/verify.rs` | the real binary on throwaway git repos: tampering, timeouts, sandbox, hooks, every check type |
| Property | `crates/countercheck-core/tests/properties.rs` | rules hold for thousands of generated inputs (e.g. any edit breaks the seal) |
| Claim fixtures | `crates/countercheck-core/tests/claims.rs` | blocker detection on real agent answers |
| Mutation | `cargo mutants` (manual) | the tests notice when the logic is broken |
| Benchmark | `eval/` | real agents on tasks with hidden tests ([results](eval/results-2026-10.md)) |
| Dogfood | `.countercheck/checks.toml` | Countercheck verifies its own repo |
