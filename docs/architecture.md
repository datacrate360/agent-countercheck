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
| **Never soften a failure on the worker's word** | a worker must not be able to turn FAILED into something milder | hints may explain a failure (e.g. "ran without internet"), but they never change the verdict; the one exception is proven on the locked base, not read from the worker's output: a check that also fails offline on the base revision is UNVERIFIED ([ADR-0005](decisions/0005-network-only-failures.md)) |
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
| **3. Put protected files back** | the clean copy's own bytes are compared with the base (not git's index, filters or `diff`, which the worker can configure; a file that differs only by CRLF line endings counts as unchanged but still runs with its base bytes if protected); every changed or deleted file matching `protected` is restored from the base revision; a new one is kept (an agent may add tests) unless it matches `frozen`, then it is removed from the copy; a symlink the worker added or changed that leads outside the copy is removed too (`snapshot::link_escapes`); each is recorded | `verify.rs`, `crates/countercheck-core/src/protect.rs`, `ProtectedChange` in `receipt.rs` |
| **4a. Scan** | `[[forbid]]` and `[[coverage]]` read the copy **before** any command runs, so a command can't hide what was there | `apps/countercheck/src/scan.rs`, core `forbid.rs`, `coverage.rs` |
| **4b. Run** | each check runs with `sh -c` (or its `shell`) in the copy, with a timeout, in the macOS sandbox (no internet unless `network = true`, writes only in the copy). Golden output files are removed first; after the run, golden and budget compare the output | `apps/countercheck/src/runner.rs`, `golden.rs`; core `golden.rs`, `budget.rs` |
| **4b on Windows** | the same steps, natively: `sh` is Git for Windows' `sh.exe` (missing: the check fails with a plain message), `shell = "pwsh"`/`"cmd"` also work; the sandbox is a write-restricted token (`countercheck sandbox-exec`, writes only in the check folder, temp dir and `writes`; internet not blocked, and the receipt says so) for `pwsh`/`cmd` checks; `sh` checks run without it (MSYS can't start under that token), each marked `sandboxed: false` on the receipt, and the run's label says `partial`; each check runs in a Job Object, so a timeout kills every process it started (and leftovers stop when the check ends), with `taskkill /T /F` as a fallback; `link` folders are directory junctions; repo symlinks that Windows refuses become text files with the target (noted in the check output); every path on the receipt, in scans and in the tree hash uses `/`, so the same work gives the same tree hash on every OS | `runner.rs` (`shell_program`, `kill_tree`, `windows_command_line`), `windows_sandbox.rs`, `crates/countercheck-winsandbox` (the only `unsafe`), `snapshot.rs` (`rel_slash`, `canonical`, `link_or_text`) |
| **4c. Hook settings** | the agent's hook files in the real working tree (`hook_settings::FILES`: `.claude/` and `.cortex/` `settings*.json`, the files `hook install` writes for Cursor, Codex, Copilot, Goose, Kiro and Devin CLI, and Codex's `.codex/config.toml`, read as TOML) are compared with the base, and Amp's plugin byte for byte (`PLUGIN_FILES`): hooks switched off (`disableAllHooks`, Codex's `[features] hooks = false`), a Countercheck hook removed or changed, a `COUNTERCHECK_*` variable set there, or Goose's `disabledPlugins` naming countercheck makes the run FAILED (`hook_settings` on the receipt), since the agent may have stopped its own referee | core `hook_settings.rs`, `verify.rs` (`read_settings`) |
| **4d. Linked folders** | each `link` folder (from the worker's tree) is read 4 levels deep for files or symlinks that name a project path outside the linked folders (an editable install's `.pth`, a workspace symlink): such a pointer makes the run UNVERIFIED, or FAILED with a protected change, since the checks may have read the worker's real files; in a hooked session the folder's fingerprint is compared with the one `SessionStart` recorded, and a change makes it UNVERIFIED; checks run with `PYTHONPYCACHEPREFIX` in their temp folder; `require_clean` turns a VERIFIED of uncommitted work into UNVERIFIED | `apps/countercheck/src/linkscan.rs`, core `linked.rs`, `hook.rs` (`linked_fingerprints`) |
| **5. Decide, seal, save** | `decide` turns results and protected changes into a verdict and reasons; `seal` hashes the receipt; it is saved to `.countercheck/receipts/<id>.json` | `crates/countercheck-core/src/receipt.rs` (`decide`, `seal`, `seal_is_valid`) |

Exit codes: `0` verified, `2` failed, `3` unverified, `1` Countercheck itself couldn't run.

### The hooks

A hook is how an agent gets checked automatically when it tries to finish. Each hook reads the agent's own JSON input,
runs the same `verify`, and answers in the agent's format: `apps/countercheck/src/hook.rs`.

- `decide` in `hook.rs`: VERIFIED → the agent may finish (the Claude Code / Cortex Code hook shows the user one `systemMessage` line with the verdict and receipt id). FAILED and the agent claims success → **continue**,
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
| Support another agent (OpenCode, Devin, …) | one file in `agents/` (its hook file, its stop hook), `main.rs`, `hook_settings::FILES`, `docs/hooks/` | [add-agent-hook](../.claude/skills/add-agent-hook/SKILL.md) |
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
