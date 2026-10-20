# Changelog

## Unreleased

- **Renamed to Agent Countercheck**, to make clear it's for AI agents. The command is still `countercheck`, and so are
  the `.countercheck/` folder, the hooks and the `COUNTERCHECK_*` settings: existing projects need no change.
  - GitHub: `datacrate360/agent-countercheck` (the old address redirects).
  - PyPI: `pip install agent-countercheck` / `uvx --from agent-countercheck countercheck`. `pip install countercheck`
    keeps working: from this release it installs `agent-countercheck`.
  - Homebrew: `brew install datacrate360/tap/agent-countercheck`; the tap renames the old formula, so
    `brew upgrade` moves existing installs over.
- **New look for `countercheck ui` and `countercheck report`**, in DataCrate360's teal and coral: a header, cards for the
  totals, verdict pills, and runs that open and close. Still no web fonts, images or outside requests.
- **Fixed: the `countercheck ui` page showed "disconnected" and stopped updating after a new receipt landed.** The
  server handled one connection at a time and answered a browser's spare, still-silent connection with `400 Bad
  Request` after 2 s; the browser then read that stale reply for its next update. Silent connections are now closed
  without a reply, and each connection has its own thread (at most 32), so one can't hold up another.

## v0.3.1 (2026-10-20)

**Hermes desktop app / gateway users:** upgrade; with v0.3.0 the hook ran no checks there. Then approve the hook once
in a terminal ([docs/hooks/hermes.md](docs/hooks/hermes.md)).

- **Recipes:** [`recipes/`](recipes) with a first recipe, Oracle → Snowflake (check files, helper scripts, a tested
  5-minute start). README FAQ and diagrams.

- **Fixed: the Hermes hook did nothing when Hermes ran outside the project.** Hermes sends its own working directory
  as `cwd`, and the desktop app and gateway run from the home folder, so `hermes-pre-verify` found no git repository
  and silently let every turn finish. It now checks the git repositories holding the turn's `changed_paths` that
  have a `.countercheck/checks.toml` (each one; a failure names its project), and stays silent when the changed files
  are in no such project. A project counts if its check file is in the working tree or the base revision, so
  deleting `checks.toml` doesn't skip the checks. Without `changed_paths`, `cwd` is used as before.
- **Docs: [docs/hooks/hermes.md](docs/hooks/hermes.md)**: setup, which project gets checked, and the one-time hook
  approval Hermes needs (`hermes hooks list` shows "not allowlisted" until you answer y in a terminal; until then the
  hook never runs).

## v0.3.0 (2026-10-19)

- **Native Windows support** (Windows 10/11, x86_64): `countercheck.exe` runs without WSL. Install with
  `pip install countercheck`, `uvx countercheck` or `irm .../install.ps1 | iex` (checks the SHA-256 against
  `SHA256SUMS.txt` before installing). What works: `verify`, `show`, `init`, `report`, `ui`, the three hooks
  (`claude-stop`, `hermes-pre-verify`, `session-start`) with Windows paths, every check type. Checks run with Git for
  Windows' `sh` by default (a missing `sh` fails the check with a plain message); each check runs in a Job Object,
  so a timeout kills the whole process tree and background leftovers stop when the check ends; `link` folders become directory junctions; symlinks Windows refuses are written as text files with the
  target, like git does. Paths and the tree hash use `/`, so the same work gives the same receipt paths and tree
  hash on Windows and Unix. CI tests every change on Windows. What doesn't, yet: **no sandbox on Windows** (checks
  run as a normal process; the receipt says `none (no sandbox on Windows yet)`, `--sandbox seatbelt` is a plain
  error); no ARM64 build (the x86_64 one runs under emulation); no winget or scoop.
- **`shell` option on a check**: `"sh"` (default), `"pwsh"` (PowerShell 7, or Windows PowerShell) or `"cmd"`
  (Windows). Validated when the check file is read.
- **Fixed: Windows line endings.** A `$` at the end of a `coverage` (or `forbid`) pattern now also matches before
  `\r\n`; before, `coverage` reported items in CRLF files as missing.
- **`countercheck ui`**: a live page of the receipts that updates while an agent works. 127.0.0.1 only, a random
  token in the address, `Host` checked (so other web pages and DNS rebinding can't read it), GET only, read-only, a
  strict Content-Security-Policy with a per-page nonce. Port 7357, or any free port when taken (`--port` to choose);
  a second `countercheck ui` reuses the running one. No web framework: the Rust standard library only.
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
