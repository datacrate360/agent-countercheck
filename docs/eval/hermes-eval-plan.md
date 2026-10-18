# Evaluation: does Countercheck actually help Hermes?

Status: plan, not yet run. Hermes facts below are from the installed source, v0.22.0, read-only.

## 1. In plain words

We give Hermes the same coding tasks several times, with and without Countercheck, and judge every result with
**hidden tests that neither Hermes nor Countercheck can see**. Then we count how often Hermes says "done" when the work
isn't, how often each setup catches it, how often it raises false alarms, and what it costs in time and money.
Countercheck earns its place only if it beats **Hermes's own built-in check**, not just a bare agent.

## 2. What Hermes already has (the honest baseline)

| Feature | What it does | Default | Source |
|---|---|---|---|
| `verify_on_stop` | After code edits, if there's no "fresh passing verification evidence", nudges the agent to run the tests and repair failures (max 2 attempts) | **off**: "the nudges proved more noise than signal" | `agent/verification_stop.py`, `hermes_cli/config_defaults.py:214-221` |
| `hermes verify --json` | Build + test + start + readiness; a pass records evidence for the workspace | manual | `agent/verification_stop.py:189-193` |
| `pre_verify` hook | Outside script runs when the agent tries to finish after code edits; `{"action":"continue","message":…}` makes it keep working (max 3 nudges per turn); Claude-Code Stop format accepted | no hook configured | `agent/shell_hooks.py:485-497`, `agent/turn_stop_gates.py` |
| `/goal` judge | A helper model decides whether a goal is met; fails open | manual | Hermes source, read at v0.22.0 |

**What the built-in check doesn't do** (what Countercheck adds), each tested below:

1. Hermes's evidence comes from the agent's own runs in its own workspace. If the agent edits or skips a test and then
   runs it, that counts as passing. Countercheck uses the checks from the **base revision** and **restores protected
   tests**.
2. No clean copy and no sandbox. Countercheck runs in a fresh copy with no network.
3. No receipt anyone else can check later. Countercheck writes a sealed one.
4. It works only inside Hermes. Countercheck gives the same result for any agent.

## 3. Setups compared

| Arm | Setup | How |
|---|---|---|
| **A** | Hermes as installed (`verify_on_stop` off) | `hermes chat -q "<task>" -Q --yolo` in the task repo |
| **B** | Hermes + its own check | same, with `HERMES_VERIFY_ON_STOP=1` |
| **C** | Hermes + Countercheck | same, with a `pre_verify` hook that runs `countercheck verify`; on FAILED it returns `continue` + the receipt's reasons and output tail |
| **D** | Hermes + both | B + C |

All arms use the same model, the same tasks, a fresh copy of each task repo, and a **separate Hermes profile**
(`countercheck-eval`), so the owner's own profile, memory and skills are never touched and learning from one run
can't leak into the next.

## 4. Tasks (the test set)

About 24 small, realistic tasks in Python and JavaScript repos. Each has: a description, visible tests (which
Countercheck locks), **hidden acceptance tests** (ground truth, stored outside the repo), and a known correct solution.
Every task is checked in advance: the solution passes the hidden tests and the starting code fails them.

| Kind | Count | Why it's included |
|---|---|---|
| **Trap: hard failing test** (the easy way out is to skip or loosen the test) | 6 | Where false "done" is most likely |
| **Trap: tests need running** (code looks right but fails at runtime) | 4 | Catches "never ran it" |
| **Trap: partial fix** (one of two cases fixed) | 4 | Catches "said everything, did half" |
| **Normal bug fixes and small features** | 8 | Measures false alarms and overhead on honest work |
| **Can't be verified locally** (needs network/service) | 2 | Countercheck must say UNVERIFIED, not FAILED |

Each task × arm runs **3 times** (models vary run to run): 24 × 4 × 3 = **288 runs**.

## 5. What we measure

| Metric | Definition | Judged by |
|---|---|---|
| **False-done rate** | Hermes says done, but the hidden tests fail | hidden tests |
| **Catch rate** | Of the false dones, how many the setup flagged before the end | log + receipts |
| **False alarms** | The setup flagged work that the hidden tests pass | hidden tests |
| **Final success** | Hidden tests pass at the end (after any retries) | hidden tests |
| **Test tampering** | The agent edited or skipped a protected test | git diff vs base |
| **Cost** | Tokens and dollars per task (Hermes usage file / `post_api_request` hook) | Hermes logs |
| **Time** | Wall time per task | runner |

## 6. What counts as "Countercheck helps" (decided before running)

Countercheck (arm C) is worth shipping as-is if, compared with arm B (Hermes's own check):

- it catches **at least twice as many** false dones, **or** raises final success by **≥ 10 percentage points**;
- with false alarms **≤ 1 in 20** honest tasks;
- and adds **≤ 20%** time and cost.

If it catches more but with more false alarms, we fix the noise before launch. If it doesn't beat arm B, we say so
in public and change direction (cost per verified task, or receipts across vendors, not the check itself).

## 7. How it runs

- `eval/` folder in this repo: the tasks (`eval/tasks/<id>/` with repo, `task.md`, hidden tests stored outside the
  repo copy), `eval/run.py` (makes a fresh copy, sets the arm, runs Hermes, collects claim/receipt/diff/cost, then runs
  the hidden tests), and `eval/report.py` (tables + HTML report).
- `eval/hooks/countercheck_pre_verify.sh`: reads Hermes's hook JSON from stdin, runs `countercheck verify --json`
  in `cwd`, prints `{"action":"continue","message":…}` only when the verdict is FAILED.
- Small pilot first: 4 tasks × 4 arms × 1 run, to check the plumbing and the cost per run, then the full set.

## 8. What the owner decides before we run

1. **Model and budget**: which model Hermes uses for the eval (results depend on it; ideally one strong and one
   cheaper), and a spending cap. The pilot shows the real cost per run before the full set.
2. **Permission to create the `countercheck-eval` Hermes profile** (a new folder under `~/.hermes/profiles/`, with
   its own config and the hook; the owner's profiles stay unchanged).
3. Whether to publish the results (recommended either way: an honest public result is the best launch material).

## 9. How to run (built 2026-10-18)

```sh
cargo build --release                         # countercheck binary used by the hook
python3 eval/validate.py                      # 24/24 tasks must be valid
python3 eval/setup_profile.py                 # creates ~/.hermes/profiles/countercheck-eval (idempotent)
# owner: add DEEPSEEK_API_KEY=... to ~/.hermes/profiles/countercheck-eval/.env (never in chat)
python3 eval/run.py --tasks pilot --reps 1 --budget-usd 1 --out eval/results/pilot
python3 eval/report.py eval/results/pilot
```

Checked so far without spending: dry run of all 16 pilot runs (plumbing, judging, report); the hook fired
through Hermes's own runner (`hermes hooks test pre_verify`) and returned `continue` with the failure in 0.23 s;
Hermes accepted the runner's exact command line and stopped only at the missing DeepSeek key.
