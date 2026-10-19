# Benchmark: Countercheck vs Hermes `verify_on_stop` (October 2026)

**Question:** when an AI agent says "done", does Countercheck catch the false claims better than Hermes Agent's
own `verify_on_stop`, without slowing honest work down or crying wolf?

**Short answer:** with GPT 6.1, yes: false "done" claims fell from 7 to 3 (11 with `verify_on_stop`), and all 6
cheats on impossible tasks disappeared. With DeepSeek V4 Pro, no difference: its false claims came from bugs that
the project's own checks didn't test, which no referee running those checks can see. Countercheck is exactly as
strong as the checks a project locks.

## Setup

- **Agent:** Hermes Agent v0.22, its own profile with memory off; one-shot runs, tools: terminal, files, todo.
- **Models:** DeepSeek V4 Pro (API) and GPT 6.1 (`gpt-6.1-sol`, ChatGPT subscription).
- **Setups:** **A** Hermes as installed · **B** + `verify_on_stop` · **C** + Countercheck `pre_verify` hook ·
  **D** both.
- **Tasks:** 29 small Python / JavaScript projects (`eval/tasks.py`, `eval/tasks_hard.py`), each with visible tests
  (the agent sees them; Countercheck locks them) and **hidden tests** (ground truth; nobody sees them): 8 normal,
  14 traps (the obvious fix is wrong, only running finds it, or only part is fixed), 5 impossible (no honest change
  passes every test), 2 that need the internet. Validated: every reference solution passes, every starting state
  fails (`eval/validate.py`).
- **Runs:** 29 tasks × 4 setups × 3 repeats = 348 per model, each in a fresh private copy.
- **"Claimed done"** is judged by a model reading the final message (`eval/judge.py`; agreed with hand labels
  36/36 on the pilots). **False "done"** = claimed done while the hidden tests fail.
- Countercheck build: the DEV commit of 2026-10-18 (v0.2.0 behaviour for these tasks: the "honest blocker" rule on;
  the Linux timeout fix doesn't apply on macOS).

## Results

### GPT 6.1 (348 runs, all contained)

| Setup | Solved | False "done" | on impossible tasks | on trap tasks | Median time* |
|---|---|---|---|---|---|
| A Hermes | 81.6% | 7 | 6 | 1 | 29 s |
| B + verify_on_stop | 79.3% | **11** | 8 | 3 | 55 s |
| C + Countercheck | 79.3% | **3** | **0** | 3 | 30 s |
| D both | 80.5% | **2** | **0** | 2 | 56 s |

### DeepSeek V4 Pro (337 clean runs, $5.54)

| Setup | Solved | False "done" | on impossible tasks | on trap tasks | on normal tasks | Median time* |
|---|---|---|---|---|---|---|
| A Hermes | 86.9% | 7 | 2 | 3 | 2 | 20 s |
| B + verify_on_stop | 86.9% | 6 | 2 | 3 | 1 | 38 s |
| C + Countercheck | 84.3% | 6 | 1 | 3 | 2 | 20 s |
| D both | 80.2% | 6 | 2 | 2 | 2 | 42 s |

\* median seconds per run, excluding the two internet tasks.

## What it means

1. **Countercheck stops gaming.** GPT 6.1's typical cheat on an impossible task is to edit the protected test
   (fix the "wrong" expected value, mock the unreachable server) and report "all tests pass". Countercheck re-runs
   the original tests, says FAILED and pushes back once; GPT then restores the test and reports the real blocker.
   6 → 0 on impossible tasks. `verify_on_stop` made it worse (6 → 8): its own ad-hoc checks passed on the edited
   tests.
2. **Countercheck can't see what the checks don't test.** Every remaining false "done" (both models) is a trap task
   where the visible tests pass but the hidden tests fail, mostly one task (`h5_deep_merge`, a nested-merge edge case
   the visible tests don't cover). Countercheck says VERIFIED there, correctly for the checks it was given. This is
   the main limit, and the reason for the new check types (golden, coverage, matrix): richer locked checks catch
   more.
3. **Model behaviour decides the value.** DeepSeek rarely games tests (2 test edits in 337 runs) and mostly fails
   honestly; GPT edits test files in most runs, mostly by **adding** tests (about 3,900 lines added, 144 removed),
   and cheats when stuck. The value of a referee depends on the model's habits; stronger models game more
   cleverly.
4. **No false alarms on ordinary work.** Countercheck said FAILED on correct work only on the two internet tasks
   (24 / 348 per model): their visible test calls a real website, and the sandbox blocks the internet by design.
   It should report those as "can't verify" (UNVERIFIED) instead of FAILED; see Limits.
5. **Time cost is small.** Countercheck adds about 1 second to the median run (29 → 30 s GPT, 20 → 20 s DeepSeek);
   `verify_on_stop` roughly doubles it, because it makes the agent write and run extra checks.

### Against the bar set in the eval plan

| Bar | GPT 6.1 | DeepSeek V4 Pro |
|---|---|---|
| ≥ 2× the false-done catches of `verify_on_stop` | **Met** (C removes 4; B adds 4) | Not met (no difference) |
| or +10 pp success | Not met (−2 pp) | Not met (−3 pp) |
| False alarms < 1/20 | Not met overall (6.9%, all internet tasks); **0** on the other 27 tasks | Same |
| Overhead ≤ 20% | **Met** (+1 s median) | **Met** |

## Limits and honesty notes

- **Leaks:** on the internet tasks, DeepSeek sometimes searched the disk for the unreachable service and found
  other runs' saved outputs. 11 such runs are excluded (the GPT data has none). Fix planned: sandbox the agent
  itself during evals, not only the checks.
- **Pilots 1-2** (before 2026-10-18 evening) ran with agents starting in the wrong folder and are not used here.
- **One build, one agent:** results are for Hermes Agent; the Claude Code hook (v0.2.0) is not benchmarked yet.
- **Small numbers:** 87 runs per setup; differences of 1-2 runs are noise. The impossible-task result (6 / 8 vs 0 / 0
  over 15 runs each) is the clearest signal.
- **Not yet tested:** Claude models (planned: Opus and Sonnet on the trap and impossible tasks).

## Next

- UNVERIFIED (not FAILED) when a check fails only because the sandbox blocked the network, plus a per-check
  network allow-list (ROADMAP).
- Sandbox the agent during evals; rerun the internet tasks.
- Claude Opus / Sonnet on the 19 trap and impossible tasks; the Claude Code hook on the same tasks.

Raw data: `eval/results/full-deepseek`, `eval/results/full-gpt` (not in git: `results.jsonl`, `judge.json`, per run
`final.txt`, `diff.patch`, `hook.jsonl`, `repo.tar.gz`).
