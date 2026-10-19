# Changelog

## Unreleased

- **`countercheck report`**: one self-contained HTML page of every saved run (verdict, the agent's claim, each check
  and its output, protected changes, seal status, a targets × checks grid for matrix checks). No scripts and no
  outside resources (enforced by a Content-Security-Policy); every value is escaped, because receipts carry text from
  the agent's files. `--open` opens it in the browser.

## v0.2.1 (2026-10-19)

**Upgrade note:** add the new `session-start` hook next to your existing stop hook (snippets in the README, "Agent
hooks"). Without it, hooks behave as in v0.2.0.

- **Security: `countercheck hook session-start`** (Claude Code / Cortex Code `SessionStart`, Hermes
  `on_session_start`). The stop hooks locked the checks to `HEAD`, so an agent that **committed** an edit to a
  protected test or to `checks.toml` moved the "locked" rules with it and was let through. The new hook records the
  commit each session started from (in `.git/countercheck/sessions/`), and the stop hooks lock the checks to it.
  Add it to your hook settings (README "Agent hooks"). The benchmark was not affected: it pinned the base with
  `COUNTERCHECK_BASE`.

- **`network = true`** on a check lets it use the internet under the macOS sandbox (writes stay confined; the receipt
  marks it). A check that fails while running without the internet gets a hint; the verdict stays FAILED, so a
  worker can't soften a failure by making a blocked network call on purpose. This removes the benchmark's only
  false alarms (checks that call a real website). The hint only appears when the output looks like a connection
  error.
- **Contributors:** `docs/architecture.md` (the rules and one run step by step), step-by-step guides in
  `.claude/skills/`, a new example `examples/ml-eval-threshold/`, DCO sign-off for pull requests.

## v0.2.0 (2026-10-19)

**Upgrade if you use Countercheck on Linux.** In v0.1.0, when a check ran past its time limit, Countercheck
stopped it with `kill -KILL -<process group>`. The `kill` in procps-ng 4.0.4 (Ubuntu 24.04, Debian 13 and others)
can misread that and signal every process the user owns. v0.2.0 uses `kill -s KILL -- -<group>` and never signals
group ids of 1 or less; a test checks that a timed-out check's whole process group really stops. macOS was not
affected.

### New

- **Check types** ([docs/checks.md](docs/checks.md)): `matrix` (one check over many targets, a result per target),
  `[[forbid]]` (patterns that must not appear, reported as `file:line`), `golden` (output must match a locked
  reference: exact text, or CSV with a numeric tolerance and optional row sorting), `budget` (a printed
  `METRIC name=value` within limits) and `[[coverage]]` (every item in a locked list is found). Target lists,
  reference files and inventories must be protected, so a worker can't change what it is judged by.
- **Example:** [examples/data-migration/](examples/data-migration/), a made-up two-tenant migration that uses
  every check type and fails for four explained reasons.
- **Claude Code hook:** `countercheck hook claude-stop` blocks Claude Code's stop while the checks fail and passes the
  reasons back ([docs/hooks/claude-code.md](docs/hooks/claude-code.md)). Cortex Code reads the same hook format.
  A loop guard (`COUNTERCHECK_MAX_NUDGES`, default 3) means it never blocks forever.
- **Honest blockers:** both hooks let an agent stop when its final message honestly says the work can't be
  finished (and it changed no protected file). The FAILED receipt is kept. Claims of success and any protected-file
  change are still pushed back.
- **Sandbox (macOS):** localhost is allowed, so web tests with a local server and a browser work; the internet
  stays blocked.

### Fixed

- A receipt that recorded a cost (`--cost-usd`) could read back as BROKEN in `countercheck show` (a float lost one
  bit in JSON). Numbers now round-trip exactly.
- Protected-file matching resolves `./`, `..` and empty path segments first, so `./tests/a` is protected and paths
  outside the repository never match.
- The PyPI source package includes the LICENSE file.

### Tests

69 → 136 tests on macOS (134 on Linux), including property tests and tests that catch every surviving mutant
found by mutation testing in the receipt, claim and config code.

## v0.1.0 (2026-10-18)

First release: `countercheck verify / init / show`, locked checks from the base revision in a clean copy,
protected files restored, sealed receipts, macOS sandbox, Hermes Agent hook; install with pip, uv, Homebrew or
the install script.
