# Countercheck

**Proof that AI work really happened.**

AI coding agents often say "done, all tests pass" when it isn't true: they skip a failing test, edit the test so it
passes, or never run it. `countercheck verify` checks the claim itself:

1. It reads your checks from the **base revision** (before the agent's change), so the agent can't weaken them.
2. It copies the agent's files into a **clean folder** and puts back any protected file the agent changed (your tests).
3. It runs the checks in a **macOS sandbox**: no network, writes only inside that folder.
4. It writes a **receipt**: the claim, what ran, the output, what the agent changed that it shouldn't have, the cost if
   known, and a verdict. Receipts are sealed with a SHA-256 hash, so edits are detectable.

```text
$ countercheck verify --claim "Fixed the discount bug. All tests pass." --agent claude-code --cost-usd 0.42
countercheck receipt 9073a384e88f9d37  FAILED
  claim:    "Fixed the discount bug. All tests pass." -> CONTRADICTED by evidence
  checks:   .countercheck/checks.toml @ HEAD (locked), sandbox: macos-seatbelt (no network, writes only in the check folder)
  protected Modified: tests/test_pricing.py -> restored_from_base
  [FAIL] unit tests  (65 ms)  $ python3 -m unittest -q
         |     self.assertEqual(apply_discount(10, 150), 0)
         | AssertionError: -5.0 != 0
  cost:     $0.4200 (reported)
  - the worker changed 1 protected file(s); countercheck ran the checks with the base version
  - check "unit tests" failed (exit 1)
```

(The agent's own run said `OK (skipped=1)`: it had marked the failing test as skipped.)

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

**Sandbox by platform.** On macOS, checks run in the system sandbox (no network, writes only in the check folder).
On Linux there is no sandbox yet: checks run as a normal process and the receipt says so; run Countercheck inside
a container with networking off when you need isolation.

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

Exit codes: `0` verified · `2` failed (claim contradicted) · `3` unverified (nothing failed, but the checks weren't
locked in the base revision, or none ran) · `1` error · `4` (`countercheck show`) receipt was edited.

## Status

Early (v0.1, macOS first). Not yet: signed receipts (Ed25519), cost read from agent logs, read-back checks against real
systems (databases, CRMs, web pages), a receipts UI, and Linux/Windows sandboxes. See `docs/decisions/` and `research/`.

## Build

Rust 1.90+: `cargo build --release` → `target/release/Countercheck`. Gate: `cargo fmt --check`, `cargo clippy --workspace
--all-targets -- -D warnings`, `cargo test`. Countercheck also verifies itself: `countercheck verify` (checks in `.countercheck/checks.toml`).

## Repository

- `crates/countercheck-core`: pure logic (check file, protected paths, receipts, verdicts)
- `apps/countercheck`: the CLI (git, clean copy, sandbox, runner)
- `research/`: the research behind the project · `reports/`: rendered reports · `docs/decisions/`: decisions

MIT licensed.
