# Changelog

## Unreleased

- **Changed (CC-19, [ADR-0005](docs/decisions/0005-network-only-failures.md)): a check that fails only because the
  sandbox blocked the internet makes the run UNVERIFIED, not FAILED.** Countercheck confirms it by running that check
  once more on the base revision; it counts only if the base fails for lack of internet too, so an agent can't use it
  to soften a real failure. The receipt marks the check `needs_network`. Fix for such checks: `network = true`.
- **New (CC-03): strict mode.** `sudo countercheck hook install --agent claude-code --scope managed` puts the hooks in
  Claude Code's managed settings, as Countercheck's own drop-in file (`managed-settings.d/50-countercheck.json`) with
  `countercheck` named by its full path. Per Claude Code's docs, hooks there can't be switched off from user, project
  or local settings, which closes the `~/.claude/settings.json` gap. A refused write says how to get admin rights.
- **New (CC-05): `countercheck hook install / uninstall / status`.** `hook install --agent claude-code` (or
  `cortex-code`; `--scope user` for every project) adds the `SessionStart` and `Stop` hooks to the agent's settings:
  it shows the change and asks first (`--yes`, `--dry-run`), keeps the previous file as `*.countercheck-backup`,
  leaves every other setting exactly as written, adds nothing when the hooks are there (also as `hook stop`), refuses
  a symlinked or broken file, quotes a `--bin` path with spaces, warns when `countercheck` isn't on PATH and when
  Cortex Code would run the check twice. `hook uninstall` takes out only Countercheck's entries. `hook status` lists
  every agent, project and user, installed or not, and whether the project file is committed (an uncommitted hook
  is one an agent could remove unnoticed). New agents plug in as one file in `apps/countercheck/src/agents/`.
- **Docs (CC-04):** [docs/hooks/README.md](docs/hooks/README.md) lists which agents Countercheck can stop at "done"
  (gated), which it can only report on, and how sure each row is.

## v0.18.0 (2026-10-30)

Linked folders (a `.venv`, `node_modules`) can no longer carry the agent's own files into the checks.

**Upgrade:** recommended for everyone with `link` in `checks.toml`, required for Python projects installed into a
linked venv in editable mode (`pip install -e .`, `uv sync` with a `src/` layout): their runs said VERIFIED for a
protected file the agent changed. Such a venv now makes runs UNVERIFIED until it holds only dependencies; see
`docs/checks.md` ("Lock the test runner's settings").

- **Security: a linked folder that points back into the project.** An editable install writes the project's own path
  into the venv, so a check imported the agent's real files instead of the clean copy: a protected file Countercheck
  put back was not what ran, and the receipt said it was. Countercheck now reads each linked folder 4 levels deep
  for files and symlinks that name a project path outside the linked folders (an editable `.pth` or finder, a
  workspace `node_modules/<pkg>` symlink; on Windows and macOS in any letter case) and lists them as
  `linked_pointers`. The run is UNVERIFIED, or FAILED when
  the agent also changed a protected or frozen file (the hook sends it back).
- **New (CC-22): a linked folder that changes during a session.** `SessionStart` records a fingerprint of each
  linked folder; the stop hook compares, and a package installed or edited since makes the run UNVERIFIED
  (`linked_changed`). Checks run with `PYTHONPYCACHEPREFIX` in their temp folder, so a planted bytecode cache can't
  stand in for a source file.
- **New (CC-23): `require_clean = true`** in `checks.toml` (or `verify --require-clean`): a run that includes
  uncommitted changes is UNVERIFIED, so VERIFIED also means the commit passes. Older versions refuse the key.
- **New (CC-24): `countercheck hook stop`**, the same Stop hook as `claude-stop` under a name that fits Cortex Code.
- The UNVERIFIED line the hook shows now gives the reason that kept the run from VERIFIED, not the "all checks
  passed" summary.

## v0.17.0 (2026-10-29)

Fixes from the first real Cortex Code setup, found by a user and his agent.

**Upgrade:** recommended for anyone using the Claude Code or Cortex Code hook, and required if a `link` folder in
your `checks.toml` is a symlink (v0.16.0 failed those runs).

- **A stop the hook lets through now says so.** On VERIFIED, UNVERIFIED, an honest blocker or the push-back limit,
  the Claude Code / Cortex Code hook prints one line for the user (`systemMessage`), e.g. `✓ Countercheck: VERIFIED,
  2 check(s) passed in a clean copy (receipt …)`. Before, a pass, a hook that never ran and a FAILED after the
  push-back limit all looked the same; after the limit the user now reads "the work did NOT pass".
- **Fix: a `link` folder that is a symlink** (a shared venv, ignored as `.venv/`) was taken out as a link leading
  outside the project in v0.16.0, so the checks failed; it is linked in again.
- **Agent label:** a Stop with no transcript path takes the app `SessionStart` recorded for the session
  (`cortex-code`) before falling back to `claude-code`.
- **Report:** the run replay counted new files on protected paths (kept) as "put back"; a receipt for uncommitted
  work says it covers the files on disk, not only the commit.
- **Docs:** Cortex Code behaviour seen in a real setup (hooks from `.cortex/settings.json`, receipts labelled
  `cortex-code`, instruction files it loads), Claude Code and Cortex Code in one project, the hook runs at every turn
  end, and the limit of `link` folders (a linked venv the agent can change before the run; use CI with a fresh one).

## v0.16.0 (2026-10-29)

Closes two ways a new file could fake a pass, and shows sessions that were never checked.

**Upgrade:** recommended for everyone. Then add `frozen` to your `checks.toml` (see the README starter or
`docs/checks.md`, "Lock the test runner's settings"). Older versions refuse a `checks.toml` that has `frozen`, so
upgrade every machine and CI job that runs the checks first.

- **Security: new `frozen = [...]` paths.** `protected` puts back files the agent changed or deleted, but keeps
  new ones so agents can add tests; a new `tests/unit/conftest.py` (pytest loads it) or a `pytest.py` in the
  folder `python -m pytest` runs from could turn a real bug's FAILED into VERIFIED, even with the recommended
  pytest setup. Files on `frozen` paths must stay exactly as committed: changed ones are put back, new ones are left
  out of the clean copy (`removed_new_file` on the receipt, a reason, and the hook sends the agent back). The README
  starter, `docs/checks.md` and `examples/python-pytest` now freeze `**/conftest.py` and modules named like pytest;
  the example runs both tricks. Older versions refuse a check file with `frozen` (exit 1): upgrade first.
- **Security: links out of the project.** A symlink the agent added or changed that leads outside the project
  (a `tests/unit` folder pointing at a `conftest.py` elsewhere on disk) let the checks read files Countercheck never
  saw, whatever `protected` or `frozen` said. It is now taken out of the clean copy (`removed_outside_link` on the
  receipt, a reason, and the hook sends the agent back). Links that stay inside the project are unchanged.
- **Docs:** `docs/architecture.md` said new protected files are removed; they are kept (now stated, with `frozen`).
- **New (CC-02): "Not checked" sessions.** `countercheck report` and `ui` list every agent session that started in
  the project (recorded by the `SessionStart` hook) more than 30 minutes ago and has no receipt and no run in
  progress: its hook was switched off or removed, it timed out, or it was never given a task. Display only, never a
  verdict; session records now also say which agent started (`agent`).

## v0.15.0 (2026-10-29)

An agent can no longer switch its own referee off without the receipt saying so.

**Upgrade:** recommended for everyone using the Claude Code or Cortex Code hook. If you changed your own
`.claude/settings.json` hooks and haven't committed that yet, commit it, or the next run says FAILED.

- **Security (CC-01): an agent can't quietly switch the referee off.** Claude Code reads its hooks from
  `.claude/settings.json` (and the usually git-ignored `.claude/settings.local.json`), which the agent can edit
  mid-session; one line (`"disableAllHooks": true`) stopped the Stop hook, and nothing recorded it. Every `verify`
  and hook run now compares those files (and Cortex Code's `.cortex/` ones) with the base: turning hooks off,
  removing or changing a Countercheck hook (command, timeout), or setting a `COUNTERCHECK_*` variable that changes the
  check makes the run **FAILED** with a plain reason, and the receipt lists it under `hook_settings`. `report` and
  `ui` warn at the top while hooks are switched off. Adding other hooks or settings is fine; commit your own changes.
  The hooks compare the settings with the commit the session started from, never with `COUNTERCHECK_BASE`, which a
  settings file's `env` block could otherwise point past the agent's own committed change.
- **Benchmark on Claude:** Fable 5.1 and Opus 5.5 on the 29-task benchmark, all four setups
  ([docs/eval/results-2026-10-claude.md](docs/eval/results-2026-10-claude.md)): neither gamed the tests; Opus made no
  false "done" claim in 348 runs; Countercheck added no time and no false alarm. Raw data in `eval/results/`.
- **eval:** `validate.py` uses `--sandbox auto` (runs on Linux); `run.py --price-in/--price-out` for models Hermes
  has no price for.

## v0.14.0 (2026-10-24)

Every run says which agent made it, and Countercheck cleans up after itself.

- **Agents name themselves:** without `--agent`, `verify` takes the agent from the shell it runs in: Claude Code
  (`CLAUDE_CODE_CHILD_SESSION` / `CLAUDECODE`), Cursor (`CURSOR_AGENT`), Codex (`CODEX_SANDBOX*`), Hermes
  (`HERMES_SESSION_ID`, which also gives the session). A label like `--agent`, never a proof; `--agent` still wins.
  Runs that say nothing are shown as *unnamed agent* ("run without `--agent` or a role"), not "unknown agent".
- **New: `countercheck prune --older-than 90d`** deletes receipts older than an age, by each receipt's own date
  (`--dry-run` lists them first). Receipts are still never deleted otherwise; files that aren't receipts are never
  touched.
- **Cleanup:** each `verify` clears what killed runs left in `running/` (stale or broken progress files, half writes)
  once they've been quiet a minute; the Claude Code hook's push-back counters of sessions unused for 30 days are
  removed, like session records already were.
- **Fix (report):** an agent's run counts wrap in their column instead of running into its latest claim; a run with
  no claim says "no claim given" (and its replay starts "Agent asked for a check"); runs in progress show the time
  of day.

## v0.13.0 (2026-10-24)

Watch agents' claims being checked as it happens, and keep a busy project readable.

- **New: runs in progress.** While `verify` works it keeps `.countercheck/receipts/running/<id>.json` (stage, check
  *n of m*, the check's name, the agent and its claim), replaced whole at each step and removed when the receipt is
  written. The report and live UI show it as a moving line into Countercheck and a *Checking now* list with the
  current step. Display only: no verdict reads it, the page says it isn't a receipt, and since the agent could write
  such a file too, every field is treated as hostile (size cap, labels cleaned, a run past its `stale_after` time is
  skipped, so a killed run doesn't linger). The receipts folder is now git-ignored before the clean copy is made.
- **New: many agents.** With more than 12 agents the flow map groups them by tool (`claude-code · 7 agents`, a line
  per outcome, wider for more agents), with a *By tool / Each agent* switch (CSS; the live UI remembers it).

## v0.12.0 (2026-10-23)

See how each agent's "done" went: a flow map and a step-by-step replay of every run.

- **New: flow map** at the top of the report and live UI: each agent's latest claim runs through Countercheck (the
  checks locked at the base commit) to *delivered*, *sent back* or *needs a person*. Point at an agent to light its
  path and the checks it broke; point at a check to see which agents broke it; click an agent to filter the runs.
  Lines draw in once; a light runs along each delivered one. All CSS, so the offline report (no script) has it too.
  In the live UI, only new lines draw after a reload, and the grid lights up around the pointer.
- **New: step replay.** Opening a run plays what happened, from the receipt: the claim, checks locked (or not), clean
  copy, protected files put back, each check with a bar sized by its time, and the sealed receipt (now a paper card
  with a wax seal).
- **Look:** an agent keeps its colour when others join (taken from its name, not its place in the list). On phones,
  a long check command scrolls inside its table instead of widening the page.
- **Fix:** the report's check-time total can't overflow on a receipt with absurd durations (found by the property
  tests; receipts come from the worker's side).

## v0.11.0 (2026-10-23)

A new report and live UI, and many agents on one project told apart.

- **New: many agents, one project.** `verify --role "Security review"` (or `COUNTERCHECK_ROLE` in the agent's
  environment, which the hooks read) labels what each agent is for; the hooks also save the agent's session id. Both
  go on the sealed receipt (left out when unset, so older receipts read the same). A role never changes what is
  checked or the verdict.
- **New: report and live UI redesign.** The project's name and folder (home shown as `~`) at the top; a verdict bar;
  an **Agents** panel with one row per agent (its role, else the tool): its runs as coloured dots, its pass count and
  its latest claim. Filter the runs by verdict or by agent (pure CSS, still no script in `report`; the live UI keeps
  your filters across reloads). Light and dark, following the system.
- **Look:** the logo's wordmark and colours follow DataCrate360's current style (Inter, one colour, teal `#0F4C4C`;
  coral only on the seal), in the README, the HTML report and the live UI. The README is rewritten: shorter, and
  current for v0.10 (GitHub Action on the Marketplace, the Windows sandbox, the platform table).
- **Fix:** the Claude Code hook now records the model. Claude Code's Stop input never includes it (only
  SessionStart's does), so receipts had none; the hook reads it from the last assistant message in the transcript
  (the new model after a `/model` switch), else from what the SessionStart hook recorded.

## v0.10.1 (2026-10-23)

- **PyPI page:** the licence badge links to the licence page on GitHub (it opened the raw file). The README link check
  now also fails when a page would be linked as a raw file.

## v0.10.0 (2026-10-23)

A GitHub Action, and a working PyPI page.

- **Installers:** when GitHub's API rate-limits the "latest version" lookup (common on shared CI runners), `install.sh`
  and `install.ps1` read it from the `releases/latest` page instead of failing. The Action uses the installers from its
  own pinned checkout.
- **GitHub Action**: `uses: datacrate360/agent-countercheck@<tag>` verifies every pull request with the checks
  from its base commit; a job summary (verdict, checks, protected changes, failing output), outputs, and the receipt
  plus an HTML report as an artifact. `fail-on`: unverified (default), failed or never. Linux, macOS and Windows
  runners. Guide: `docs/guides/github-action.md`.
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
