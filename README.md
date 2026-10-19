# Countercheck

**Proof that AI work really happened.**

AI coding agents often say "done, all tests pass" when it isn't true: they skip a failing test, edit the test so it
passes, or never run it. `countercheck verify` checks the claim itself:

1. It reads your checks from the **base revision** (before the agent's change), so the agent can't weaken them.
2. It copies the agent's files into a **clean folder** and puts back any protected file the agent changed (your tests).
3. It runs the checks in a **macOS sandbox**: no internet (localhost works; a check can opt in with `network = true`),
   writes only inside that folder.
4. It writes a **receipt**: the claim, what ran, the output, what the agent changed that it shouldn't have, the cost if
   known, and a verdict. Receipts are sealed with a SHA-256 hash, so edits are detectable.

```text
$ countercheck verify --claim "Fixed the discount bug. All tests pass." --agent claude-code --cost-usd 0.42
countercheck receipt 9073a384e88f9d37  FAILED
  claim:    "Fixed the discount bug. All tests pass." -> CONTRADICTED by evidence
  checks:   .countercheck/checks.toml @ HEAD (locked), sandbox: macos-seatbelt (no internet, localhost only; writes only in the check folder)
  protected Modified: tests/test_pricing.py -> restored_from_base
  [FAIL] unit tests  (65 ms)  $ python3 -m unittest -q
         |     self.assertEqual(apply_discount(10, 150), 0)
         | AssertionError: -5.0 != 0
  cost:     $0.4200 (reported)
  - the worker changed 1 protected file(s); countercheck ran the checks with the base version
  - check "unit tests" failed (exit 1)
```

(The agent's own run said `OK (skipped=1)`: it had marked the failing test as skipped.)

## How it works

![How Countercheck works: it reads the locked checks from the base revision, the agent's work and its claim; locks the checks, makes a clean copy with protected files put back, runs the checks in a sandbox, and seals a receipt; then the agent may finish (VERIFIED), is sent back with the failing check (FAILED), or may stop after honestly reporting a blocker.](docs/img/how-it-works.svg)

The mental model: **the agent never grades itself.** Its claim is compared with what actually runs, on rules it
couldn't change:

```text
base revision (locked)        agent's work + claim
  checks.toml, your tests          files, "Done, all tests pass."
          \                           /
           +--> countercheck verify <+
                1 lock the checks     (from the base revision)
                2 clean copy          (edited tests put back)
                3 run in a sandbox    (no internet unless a check opts in)
                4 decide + seal       (receipt with a SHA-256 seal)
                        |
      VERIFIED -> agent may finish
      FAILED + "done" -> hook sends the failure back; agent fixes the code (max 3 rounds)
      FAILED + honest "can't, because ..." -> agent may stop; receipt still says FAILED
```

## Does it work?

We gave [Hermes Agent](https://github.com/NousResearch/hermes-agent) 29 small coding tasks, 3 times each, in four
setups, and graded every run with **hidden tests** the agent never saw. Some tasks are impossible on purpose (a test
expects a wrong value, a server can't be reached): the honest answer is "this can't pass, because …".

| False "done" claims (claimed success, hidden tests fail) | Hermes | + its `verify_on_stop` | **+ Countercheck** | both |
|---|---|---|---|---|
| GPT 6.1, all tasks (87 runs per setup) | 7 | 11 | **3** | **2** |
| GPT 6.1, impossible tasks only | 6 | 8 | **0** | **0** |
| DeepSeek V4 Pro, all tasks | 7 | 6 | 6 | 6 |

- When GPT 6.1 got stuck, it edited the protected test and said "all tests pass". Countercheck re-ran the original
  tests, said FAILED and pushed back once; the agent restored the test and reported the real problem.
- Countercheck only knows what your checks test: the remaining false claims were cases the visible tests didn't
  cover. DeepSeek rarely games tests, so there was little to catch.
- It added about 1 second to the median run.

Full method, numbers and limits: [docs/eval/results-2026-10.md](docs/eval/results-2026-10.md). Reproduce:
[eval/](eval).

## Install

| You use | Install with |
|---|---|
| Python (pip, a `.venv`, uv) | `pip install countercheck` · `uv tool install countercheck` · or run without installing: `uvx countercheck verify` |
| macOS or Linux, Homebrew | `brew install datacrate360/tap/countercheck` |
| macOS or Linux, any shell | `curl -fsSL https://raw.githubusercontent.com/datacrate360/countercheck/PROD/install.sh \| sh` |
| Rust | `cargo install --git https://github.com/datacrate360/countercheck countercheck` |
| Direct download | [Releases](https://github.com/datacrate360/countercheck/releases): one file per platform + `SHA256SUMS.txt` |

Builds: macOS (Apple silicon and Intel, one universal binary) and Linux (x86_64, arm64; static). The install
script checks every download against `SHA256SUMS.txt` before installing. Windows is planned; use WSL for now.

**Sandbox by platform.** On macOS, checks run in the system sandbox (no internet, localhost allowed, writes only in
the check folder; `network = true` lets a single check use the internet).
On Linux there is no sandbox yet: checks run as a normal process and the receipt says so; run Countercheck inside
a container with networking off when you need isolation.

What's shipped, in progress and next: [docs/ROADMAP.md](docs/ROADMAP.md).

## Quick start

```sh
countercheck init                      # writes .countercheck/checks.toml, edit the commands
git add .countercheck && git commit -m "Countercheck checks"
# ... let an agent work ...
countercheck verify --claim "what the agent said"            # uncommitted work vs HEAD
countercheck verify --base main                              # committed work on a branch vs main
countercheck show .countercheck/receipts/<id>.json                   # re-check a receipt's seal
```

`.countercheck/checks.toml`:

```toml
protected = ["tests/**"]          # the agent may not change these; .countercheck/** is always protected
link = ["node_modules", ".venv"]  # git-ignored dependency folders to make visible (read-only)

[[check]]
name = "unit tests"
run = "pytest -q"                 # any command; runs with `sh -c` in the clean copy
timeout_secs = 600
```

Besides plain commands there are matrix, forbid, golden, budget and coverage checks: see [docs/checks.md](docs/checks.md)
and the runnable example in [examples/data-migration](examples/data-migration).

Exit codes: `0` verified · `2` failed (claim contradicted) · `3` unverified (nothing failed, but the checks weren't
locked in the base revision, or none ran) · `1` error · `4` (`countercheck show`) receipt was edited.

## Agent hooks

- Hermes Agent: `countercheck hook hermes-pre-verify` keeps Hermes working while the checks fail.
- Claude Code and Cortex Code: `countercheck hook claude-stop` blocks the `Stop` while the checks fail. See
  [docs/hooks/claude-code.md](docs/hooks/claude-code.md).

## How Countercheck itself is tested

A referee has to be right, so Countercheck is tested harder than most tools its size:

- **140 tests** (`cargo test --workspace`): unit tests, end-to-end tests that run the real binary on throwaway git
  repositories (tampered tests, timeouts, the macOS sandbox, both agent hooks, every check type), and **property
  tests** that throw thousands of generated inputs at the verdict and seal logic (e.g. a receipt's seal breaks if any
  one of 33 fields changes; `decide` never says VERIFIED when a check failed).
- **Mutation testing** ([cargo-mutants](https://mutants.rs)): the receipt, claim and config code was deliberately
  broken in every way the tool could find; tests were added until every break that compiles is caught (one remaining
  change is provably equivalent).
- **CI on every push** (macOS and Linux): `cargo fmt --check`, `cargo clippy -D warnings`, all tests, a release build
  and the benchmark task validator. Countercheck also verifies itself with its own `.countercheck/checks.toml`.
- **The benchmark** above, with hidden tests as ground truth.

## Status

v0.2 ([CHANGELOG](CHANGELOG.md)). macOS has the sandbox; Linux runs checks without one (the receipt says so).
Not yet: signed receipts, a per-host network allow-list, cost read from agent logs, Linux/Windows sandboxes, more agent
hooks (Codex, Cursor, a GitHub Action). See [docs/ROADMAP.md](docs/ROADMAP.md).

## Build

Rust 1.90+: `cargo build --release` → `target/release/countercheck`. Gate: `cargo fmt --check`, `cargo clippy --workspace
--all-targets -- -D warnings`, `cargo test`. Countercheck also verifies itself: `countercheck verify` (checks in `.countercheck/checks.toml`).

## Repository

- `crates/countercheck-core`: pure logic (check file, protected paths, receipts, verdicts)
- `apps/countercheck`: the CLI (git, clean copy, sandbox, runner)
- `eval/`, `docs/eval/`: the evaluation against Hermes Agent (tasks with hidden tests)
- `packaging/`, `install.sh`, `pyproject.toml`: release packaging (Homebrew, install script, Python wheels)
- `docs/checks.md`: every check type · `docs/hooks/`: agent hooks · `docs/decisions/`: design decisions

MIT licensed.
