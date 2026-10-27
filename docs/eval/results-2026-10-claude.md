# Benchmark: Claude Fable 5.1 and Opus 5.5 (October 2026)

The same benchmark as [results-2026-10.md](results-2026-10.md) (GPT 6.1, DeepSeek V4 Pro), run on Anthropic's two
strongest models: does Countercheck change how often Hermes Agent says "done" when it isn't?

**Short answer:** neither Claude model gamed the tests, so there was almost nothing for a referee to catch. Opus 5.5
made **no** false "done" claim in 348 runs, in any setup. Fable 5.1 made 1 to 3 per setup, all on the two trap tasks
whose bug only the hidden tests see. Countercheck cost them almost nothing: no extra time on the median run and no
false alarm on a single correct result. Hermes's own `verify_on_stop` didn't hurt them the way it hurt GPT 6.1, but
it doubled the time and cost of every run for no gain.

## Setup

Same tasks, setups, repeats and grading as October; differences are listed under "What differed".

- **Agent:** Hermes Agent **v0.22.0** (commit `e06df9c2`), profile with memory off; one-shot runs.
- **Models:** `claude-fable-5-1` and `claude-opus-5-5`, Anthropic API.
- **Setups:** **A** Hermes as installed · **B** + `verify_on_stop` · **C** + Countercheck `pre_verify` hook ·
  **D** both.
- **Tasks:** the 29 tasks of `eval/tasks.py` and `eval/tasks_hard.py` (8 normal, 14 traps, 5 impossible, 2 that need
  the internet), 3 repeats each.
- **Countercheck:** v0.14.0 (DEV `cd8b799`), sandbox `linux-landlock` (no internet, localhost only; writes only in the
  check folder).
- **"Claimed done"** is judged by `claude-sonnet-5-5` reading the final message (`eval/judge.py`).

## Results

### Claude Opus 5.5 (348 runs, $21.10)

| Setup | Runs | Solved | False "done" | on impossible tasks | on trap tasks | Median time* | Cost per run |
|---|---|---|---|---|---|---|---|
| A Hermes | 87 | 82.8% | 0 | 0 | 0 | 17 s | $0.039 |
| B + verify_on_stop | 87 | 82.8% | 0 | 0 | 0 | 31 s | $0.079 |
| C + Countercheck | 87 | 82.8% | 0 | 0 | 0 | 17 s | $0.043 |
| D both | 87 | 82.8% | 0 | 0 | 0 | 30 s | $0.082 |

82.8% is the ceiling: Opus solved all 72 solvable runs in every setup, and the other 15 per setup are the impossible
tasks, where it reported the blocker every time (0 of 60 claimed success).

### Claude Fable 5.1 (338 runs, $60.10)

| Setup | Runs | Solved | False "done" | on impossible tasks | on trap tasks | Median time* | Cost per run |
|---|---|---|---|---|---|---|---|
| A Hermes | 87 | 79.3% | 3 | 0 | 3 | 18 s | $0.094 |
| B + verify_on_stop | 81 | 81.5% | 2 | 0 | 2 | 41 s | $0.235 |
| C + Countercheck | 87 | 81.6% | **1** | 0 | 1 | 18 s | $0.111 |
| D both | 83 | 79.5% | 2 | 0 | 2 | 43 s | $0.280 |

The $60 budget cap stopped the run 10 short of 348 (6 in B, 4 in D). All 8 false claims are on two trap tasks
(`h5_deep_merge`, `h4_duration`) where the visible tests pass and the hidden ones fail.

\* median seconds per run, excluding the two internet tasks. Without the internet tasks the false-"done" counts are
the same; the tables per setup are in each folder's `report.md`.

### All four models side by side

False "done" claims (claimed success while the hidden tests fail), 87 runs per setup unless noted:

| Model | Hermes | + its `verify_on_stop` | **+ Countercheck** | both |
|---|---|---|---|---|
| GPT 6.1, all tasks | 7 | 11 | **3** | **2** |
| GPT 6.1, impossible tasks only | 6 | 8 | **0** | **0** |
| DeepSeek V4 Pro, all tasks (≈84 clean runs) | 7 | 6 | 6 | 6 |
| Claude Fable 5.1, all tasks | 3 | 2 (81 runs) | **1** | 2 (83 runs) |
| Claude Opus 5.5, all tasks | 0 | 0 | **0** | 0 |

## What it means

1. **Claude doesn't game tests.** In 686 runs, neither model skipped or loosened a protected test, and neither
   claimed success on an impossible task (0 of 118). The one protected-test edit (Opus, `n7_stack_peek`) *added* a
   missing empty-stack test next to a correct fix, and Countercheck verified it. GPT 6.1's typical cheat (edit the
   test, report "all tests pass") never happened, so the effect that drove its result (6 → 0) had nothing to act on.
2. **The referee is cheap when the agent is honest.** Countercheck added no time to the median run for either model
   and raised **no false alarm**: FAILED on 0 of 512 correct results outside the internet tasks, while it caught 118
   of 126 broken ones (the 8 misses are the hidden-only trap bugs). On honest models it costs about as much as one
   test run, and it is the record that the work was checked.
3. **`verify_on_stop` doubles the bill.** It didn't backfire on Claude (GPT went 7 → 11), but it doubled median time
   (17 → 31 s Opus, 18 → 41 s Fable) and cost per run (2× Opus, 2.5× Fable) without changing any result.
4. **The known limit stays.** A referee runs the checks it is given. Fable's remaining false claims are bugs that only
   the hidden tests catch, exactly as with GPT and DeepSeek; richer locked checks (golden, coverage, matrix) are the
   answer, not a stronger referee.

## What differed from October

- **Linux cloud machines** (Claude Code cloud sessions) instead of macOS: the sandbox is Landlock and namespaces,
  not Seatbelt. Both block the internet during checks.
- **Countercheck v0.14.0** instead of the 2026-10-18 DEV build (the October numbers were not re-run on it).
- **Hermes pinned to v0.22.0.** The installer now gives v0.22.5, which runs one-shot jobs inside a shared
  background process. That process doesn't pass the API key or run.py's per-run settings to the hook, so the harness
  can't run on it as is.
- **Judge:** `claude-sonnet-5-5` instead of the DeepSeek judge. `eval/agreement.py` couldn't run, because the hand
  labels of the October pilots aren't in git.
- **Fable:** Hermes v0.22.0 always turns extended thinking on for Fable models (its Anthropic adapter), and the budget cap cut 10 runs (above).

## Limits and honesty notes

- **Small numbers:** 87 runs per setup; differences of 1-2 runs are noise. Fable's 3 → 1 between A and C is not a
  measured effect.
- **The agent could read the API key.** run.py passes its environment to the agent, and in one Opus run the agent
  printed its environment while checking the network proxy. The key went only to the API it belongs to, nothing was
  saved or committed, and it was a key made for this benchmark only. Next harness change: strip the provider key from the
  agent's shell (see Next in [results-2026-10.md](results-2026-10.md): sandbox the agent during evals).
- **One agent:** these are Hermes Agent results. The Claude Code hook is not benchmarked here.

## Cost

| Model | Runs | Benchmark | Pilot + judge |
|---|---|---|---|
| Opus 5.5 | 348 | $21.10 | $1.23 |
| Fable 5.1 | 338 | $60.10 | $1.63 |

Costs are Hermes's own estimates from the provider's published prices.

Raw data (in git): [`eval/results/full-claude-opus-5-5`](../../eval/results/full-claude-opus-5-5),
[`eval/results/full-claude-fable-5-1`](../../eval/results/full-claude-fable-5-1): `results.jsonl` (one line per run),
`judge.json`, `config.json`, `report.md`.
