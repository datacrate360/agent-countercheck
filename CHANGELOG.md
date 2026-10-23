# Changelog

## Unreleased

- **PyPI page:** the README's images and links work there (the logo, diagrams and demo were broken). The release
  pins them to the tag's absolute URLs while building the Python packages (`packaging/pypi_readme.py`); CI checks
  every link resolves.

## v0.9.0 (2026-10-22)

The full sandbox on Windows (opt-in, experimental) and a logo.

- **New (Windows, experimental): the full check sandbox, an opt-in admin setup** ([ADR-0004](docs/decisions/0004-windows-runner-account.md)).
  `countercheck windows-setup` (once, as administrator; `--remove` undoes it) creates a low-rights `countercheck-runner`
  account and a firewall rule that blocks its internet (loopback stays open). Checks then run as that account inside
  the existing write-restricted token and Job Object: every shell sandboxed, Git's `sh` included; internet blocked,
  localhost kept. Receipt label `windows-runner (...)`. Without the setup nothing changes. Experimental until tried on
  real Windows machines beyond CI; follow-ups: a design without a stored password, and full sandboxing for
  `network = true` checks (today they run as you).
- **A logo**: the agent (a ghost) hands in its receipt, and Countercheck's coral seal is the verdict. In the README,
  the HTML report and live UI; the full set (icon, favicons, lockups, social preview) is in `docs/assets/brand/`. The
  name and logo are not covered by the MIT licence (see `docs/assets/brand/README.md`).

## v0.8.0 (2026-10-22)

A check sandbox on Windows (partial, no admin needed), so every OS now has one.

- **New: a sandbox on Windows, for `pwsh` and `cmd` checks** ([ADR-0003](docs/decisions/0003-windows-sandbox.md)).
  Checks run through `countercheck sandbox-exec` with a **write-restricted token**, no admin needed: writes only in
  the check folder, its temp dir and `writes = [...]` locations; reads work as before (Python, node, PowerShell). The
  existing Job Object still stops everything a check started. **Git for Windows' `sh` can't start in it** (MSYS
  secures its shared memory and signal pipes to the user account only), so in a run with `sh` checks the `pwsh`/`cmd`
  checks still run in the sandbox and only the `sh` checks run without it, each marked on the receipt
  (`"sandboxed": false` and `"unsandboxed_reason"`, new optional fields; `NOT sandboxed` in the text receipt) and the
  run's label says `windows-restricted-token, partial: sh checks ran unsandboxed (...)`; set `shell = "cmd"` or
  `"pwsh"` on a check to sandbox it. **The internet is not blocked** on Windows (the only unprivileged way, an AppContainer, can't run most toolchains; a
  firewall rule needs admin); the receipt's `sandbox` says `windows-restricted-token (...; internet
  NOT blocked ...)`. `--sandbox auto` picks it when a probe write is really refused; `--sandbox windows` requires it
  (and refuses a run with `sh` checks).
  If it can't be set up at run time, the check fails rather than running unconfined.
- **New: `writes` on Windows**: `C:/path` (forward slashes) and `{home}` (`USERPROFILE`); the run's permission
  entry on the location is removed after the check.
- Write hints recognise Windows' refusals (sh, Python, PowerShell, cmd).
- The workspace's only `unsafe` code is in the new `crates/countercheck-winsandbox` (Win32 FFI, reviewed per block).

## v0.7.0 (2026-10-22)

A sandbox on Linux, checks kept outside the repository, and detached processes stopped on every OS.

- **New: a sandbox on Linux.** Checks run through `countercheck sandbox-exec`, with no root needed:
  **Landlock** confines writes to the check folder, its temp dir and `writes = [...]` (and, on kernels 6.7+, stops
  signals and abstract sockets reaching outside processes); where the system allows unprivileged user namespaces, a
  private **network namespace** with only loopback up blocks the internet (localhost works; `network = true` opts a
  check out) and a **PID namespace** stops everything a check started when it ends. Where namespaces are blocked
  (Docker's default profile, some hardened servers) writes are still confined, the internet is not, and the receipt's
  `sandbox` says exactly that; a **subreaper** then finds and kills detached processes. `--sandbox auto` picks it
  on Linux; `--sandbox landlock` requires it. The write and no-internet hints work under it too.
- **New: `--checks-from`, checks kept outside the repository** ([#20](https://github.com/datacrate360/agent-countercheck/issues/20),
  [ADR-0002](docs/decisions/0002-external-checks.md)). Verify work on a repository you don't control:
  `--checks-from <suite repo>@<full commit id>:<path>` (read through the hash-checked object reader; branch and tag
  names refused) or `--checks-from <file> --checks-sha256 <hex>`. The pin, not the location, locks the checks: an
  unpinned file gives UNVERIFIED, a changed pinned file an error, a file inside the repository is refused. External
  checks replace the repository's own (never merged), and the receipt records the source (`checks_external`,
  optional field). Command line only.
- **New: `writes = [...]` on a `[[check]]`** ([#18](https://github.com/datacrate360/agent-countercheck/issues/18)).
  Lets one check write to a fixed location under the macOS sandbox (e.g. `/var/tmp/hermes-pytest-{uid}`), so a test
  runner with a hard-coded scratch path no longer forces `--sandbox none`. Absolute paths with `{uid}`, `{user}`,
  `{home}`; after symlinks are followed, `/`, the home folder (or above) and the repository (inside, equal or above)
  are refused before anything runs. Read from the locked check file; shown on the receipt per check as `writes
  allowed: ...` (new optional field, older receipts still verify). The write-denied hint now suggests it.
- **Fix: a check's detached processes (`setsid`, double fork) are stopped on Linux** when it ends or times out.
- **Fix: on macOS, processes a check detached with `setsid` (double fork, new session) are stopped when it ends.**
  macOS has no subreaper or PID namespace, so after each check a cheap `ps` lists young orphans of this user, and a
  targeted `lsof` confirms which ones have their working folder in the check folder or still hold the check's output;
  those are killed. A process that also leaves the folder and closes its output can still escape (documented).
- **`watched` line counts are a real line diff**, the same numbers as `git diff --stat` (a moved line now counts as
  one removed and one added; before, lines were matched by text and a move counted as nothing).
- `writes` asks for the user id only when a path uses `{uid}`.

## v0.6.0 (2026-10-21)

- **New: `watched` paths** ([#19](https://github.com/datacrate360/agent-countercheck/issues/19)). Files the agent may
  change, but every change is listed on the receipt (`A`/`M`/`D` with line counts) and in `show`, `report` and `ui`:
  the middle ground between `protected` (put back) and unprotected (silent). `watched_mode = "unverified"` turns a
  VERIFIED into UNVERIFIED when an existing watched file was modified or deleted; adding files never does. Read from
  the base like every rule; protected wins when a path is both. New optional receipt field `watched`, so older
  receipts still verify.
- **New: `scope = "added"` on `[[forbid]]`** ([#17](https://github.com/datacrate360/agent-countercheck/issues/17)).
  Only lines new since the base count, compared by text with the same file at the base revision: an existing
  legitimate `pytest.skip(` no longer forces the rule to be weakened, while a new skip, a copied old one, or a new
  file still fails. Moving a line doesn't count. Default (`"all"`) is unchanged.
- **Fix: a write the macOS sandbox refused got the "no internet" hint**
  ([#16](https://github.com/datacrate360/agent-countercheck/issues/16)). "Operation not permitted" counted as a
  network error, but the sandbox prints the same words for a refused file write, so a test runner writing to a fixed
  path like `/var/tmp/...` was told to set `network = true`, which can't help. When the output names the refused
  path (Python, sh, mkdir, Node), the hint now says the check tried to write there and how to fix it; a bare
  "Operation not permitted" with no path, which a refused connection to an IP address prints too, gets a hint naming
  both causes. Hints only: verdicts are unchanged.
- **Fix: the Stop hook reads Cortex Code's final message, and the receipt names Cortex Code.** Cortex Code's Stop
  input has no `last_assistant_message`, and its transcript isn't Claude Code's JSON Lines, so every Cortex Code
  receipt had no claim, and an agent honestly reporting "this can't pass, because …" was still pushed back until the
  loop guard let it go. The hook now also reads Cortex Code's session files (`<id>.json` with a `history` array, or
  `<id>.history.jsonl`), and records the agent as `cortex-code` when the transcript is under `.snowflake/cortex/`.
  Docs now say that Snowflake's CLI page lists Stop as unable to block. Still not tried in a real Cortex Code session.
- **Docs: lock the test runner's settings, not just the tests.** With only `tests/**` protected, an agent could skip
  or deselect a failing test from outside `tests/` (a `conftest.py` in any folder, `addopts` in `pyproject.toml` or
  `pytest.ini`) and get VERIFIED. The README setup and `docs/checks.md` now protect the test-only config files and pin
  pytest's settings in the locked command; notes for jest, go and cargo; new
  [examples/python-pytest](examples/python-pytest/) proves each trick now fails (run in CI). `pyproject.toml` is
  deliberately not protected: it holds dependencies. Suggested in a reply to the launch post.

## v0.5.3 (2026-10-21)

- **Fix: on macOS and Linux, processes a check leaves running in the background are stopped when the check ends.**
  A check that started a server or `sleep 300 &` and exited could pass while that process kept running (and kept
  its port open) after `verify` finished, beyond the check's timeout. Each check already ran in its own process
  group, but the group was only killed on timeout; now it is killed after every check, as Windows already did with
  its Job Object. Reported by Perch Scan. A process that deliberately leaves the group (`setsid`) can still escape
  on Unix; the macOS sandbox and a container remain the way to contain a hostile check.

## v0.5.2 (2026-10-21)

- **Fix: protected files stored in Git LFS work again** ([#11](https://github.com/datacrate360/agent-countercheck/issues/11)).
  Since v0.5.1 the base is read from git's own objects, which for an LFS file is the small pointer, so an untouched
  LFS test fixture counted as changed and the checks ran against the pointer. Now the real content comes from the
  local LFS store, checked against the pointer's SHA-256 and size: an untouched file isn't a change, an edited one is
  put back with its real content, and a changed LFS store is refused like a changed git object. If the content isn't
  on the computer, `verify` says so (`git lfs pull`) instead of failing the check. Golden expected files stored in LFS
  are read the same way.

## v0.5.1 (2026-10-20)

**Upgrade:** security fix. With v0.5.0 and earlier, an agent that changed settings in the project's own `.git` could
get VERIFIED for broken code with weakened tests. Found in an audit (`docs/audit/2026-10-findings.md`).


- **Fix: what changed is decided from the files themselves, and the base from checked git objects.** Countercheck
  used git's index and `git diff` to find changed files, and trusted git to show the base revision. Settings in the
  worker's own `.git` could make an edited protected file look unchanged, or make the base show other content, and
  turn a FAILED into VERIFIED. Now the clean copy's bytes are compared with the base, and every git object read for
  the base is hashed again and must match its id (replace refs are ignored); a mismatch stops the run with an error.
  A protected file that differs only by CRLF line endings is not reported as changed and still runs with its base
  bytes. Protected files stored through a git filter (e.g. Git LFS) are compared as stored in git.
- **Fix: nothing is written or deleted through a symlinked folder in the clean copy.** If the worker replaced a
  folder with a symlink out of the project, putting back its protected files wrote them through the link, and a
  golden check's old output file was deleted through it. Now such a link is replaced by a real folder in the copy
  before protected files are restored, a golden output behind it is left alone (and refused after the run), and a
  `link` folder behind it is not linked.
- **Docs fix: the README's "at least N tests" example could pass failing tests.** Its exit code came from
  `grep -q '^OK'` on the output, so code that printed `OK` and a few `... ok` lines passed while its tests failed. The
  example now keeps unittest's own exit code and counts tests from its `Ran N tests` line.

## v0.5.0 (2026-10-20)

- **Receipts can say which changed files no check covers** ([#6](https://github.com/datacrate360/agent-countercheck/issues/6)).
  Add `covers = ["backend/**"]` to a `[[check]]`; `[[forbid]]` and `[[coverage]]` cover their `paths`. The receipt
  then lists every changed file none of them covers, and `show`, `report` and `ui` show it. Top-level
  `uncovered = "unverified"` turns such a VERIFIED into UNVERIFIED. Off until a check declares `covers`, so existing
  projects see no change; read from the base revision like every other rule. docs/checks.md "Which files the checks
  cover".

## v0.4.0 (2026-10-20)

**Upgrading:** nothing to change in your projects. Homebrew users: `brew trust --formula
datacrate360/tap/agent-countercheck`, then `brew upgrade` (Homebrew 7 asks before loading a renamed third-party formula).


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
