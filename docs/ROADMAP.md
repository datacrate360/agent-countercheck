# Roadmap

What Countercheck can do today, what's being built, and what comes next. Ordered by priority within each
section. Ideas and use cases are welcome as issues.

## Shipped

| | Version |
|---|---|
| `countercheck verify / init / show`: locked checks from the base revision, clean copy, protected files restored, sealed receipt | v0.1.0 |
| macOS sandbox (no internet, localhost allowed for web tests); `--sandbox auto` elsewhere | v0.1.0 (localhost: v0.2.0) |
| Install: pip, uv, Homebrew, install script, release files with checksums | v0.1.0 |
| Hermes Agent hook (`hook hermes-pre-verify`) | v0.1.0 |
| Hook lets an agent stop when it honestly reports a blocker; still pushes back on success claims and test edits | v0.2.0 |
| Claude Code `Stop` hook (`hook claude-stop`), loop guard (max push-backs per session) | v0.2.0 |
| `network = true` per check (internet under the macOS sandbox, marked on the receipt); a failed check that ran without internet gets a hint, verdict unchanged | v0.2.1 |
| `countercheck hook session-start`: hooks lock the checks to the commit the session started from | v0.2.1 |
| Native Windows 10/11 (x86_64): `countercheck.exe`, `install.ps1`, pip/uvx wheel, Windows CI; `shell = "pwsh" / "cmd"` per check; Job Object kills a timed-out check's whole tree (no Windows sandbox yet) | v0.3.0 |
| `countercheck report` (one self-contained HTML page of all runs) and `countercheck ui` (live local page, 127.0.0.1 + token) | v0.3.0 |
| Fix: `coverage`/`forbid` patterns ending in `$` missed lines in files with Windows (CRLF) line endings | v0.3.0 |
| Recipes (`recipes/`): ready check files + helpers + a tested 5-minute start per job; first: Oracle → Snowflake | v0.3.1 |
| Hermes hook finds the project from the turn's changed files (works in the desktop app and gateway) | v0.3.1 |
| Renamed to Agent Countercheck (`agent-countercheck` on GitHub, PyPI, Homebrew; command still `countercheck`) | v0.4.0 |
| `ui` / `report` in DataCrate360's teal and coral; fix: live page no longer shows "disconnected" after a new receipt | v0.4.0 |
| Receipts list changed files no check covers (`covers`, `uncovered = "unverified"`, #6) | v0.5.0 |
| Security: changes decided from file bytes, base read from hash-checked git objects; no writes through symlinked folders (2026-10 audit) | v0.5.1 |
| Protected files in Git LFS: real content from the local LFS store, hash-checked (#11) | v0.5.2 |
| macOS/Linux: background processes a check leaves running are stopped when it ends (process group killed after every check) | v0.5.3 |
| `watched` paths: every change to files the agent may edit is listed on the receipt, optionally UNVERIFIED (#19); `[[forbid]] scope = "added"`: only lines new since the base (#17); sandbox hints tell a refused write from a refused connection (#16); Cortex Code Stop hook reads its session files | v0.6.0 |
| Linux sandbox: Landlock confines writes; user + network + PID namespaces block the internet (localhost allowed) and stop everything a check started, `setsid` included; a subreaper does it where namespaces are blocked. No root needed | v0.7.0 |
| `--checks-from`: checks kept outside the repository, pinned by a suite commit or SHA-256 (#20, ADR-0002); `writes = [...]` per check (#18); macOS stops processes a check detached with `setsid`; `watched` line counts like `git diff --stat` | v0.7.0 |
| Windows sandbox (partial, no admin): a write-restricted token confines writes for `pwsh`/`cmd` checks; `sh` checks run unsandboxed and are marked per check; internet not blocked (ADR-0003) | v0.8.0 |
| Windows full sandbox, experimental (ADR-0004): `countercheck windows-setup` (admin once) runs checks as a low-rights account with no internet; every shell sandboxed, Git's `sh` included. A logo and brand set (`docs/assets/brand/`) | v0.9.0 |
| GitHub Action (`uses: datacrate360/agent-countercheck@v0.10.0`): every pull request verified with its base's checks; job summary, outputs, receipt artifact. README images and links work on PyPI | v0.10.0 |
| Report and live UI redesign (project folder, Agents panel, filters by verdict and agent); `--role` / `COUNTERCHECK_ROLE` and session id on receipts; Claude Code model recorded | v0.11.0 |
| Flow map (agents → locked checks → delivered / sent back / needs a person, hover highlights, CSS only) and step-by-step run replay with a receipt card | v0.12.0 |
| Runs in progress shown live (display only, never a verdict); flow map groups more than 12 agents by tool with a By tool / Each agent switch | v0.13.0 |
| Agent named from its shell when `--agent` is missing (Claude Code, Cursor, Codex, Hermes); `countercheck prune`; dead runs' progress files and old hook counters cleaned | v0.14.0 |
| Security (CC-01): an agent switching its own hook off in `.claude/` / `.cortex/` settings (`disableAllHooks`, a removed or changed Countercheck hook, a `COUNTERCHECK_*` variable) fails the run; `report` / `ui` warn while hooks are off | v0.15.0 |
| Security: `frozen = [...]` paths (a new file there is left out of the clean copy: a new `conftest.py`, a `pytest.py` shadowing the runner); links the agent adds that lead outside the project are taken out; **Not checked** sessions (started, no receipt) in `report` / `ui` | v0.16.0 |
| From the first real Cortex Code user: the hook shows the verdict to the user on every stop it lets through (VERIFIED, and FAILED after the push-back limit); symlinked `link` folders linked again; agent label from the session start; Cortex Code docs from a real setup | v0.17.0 |
| Linked folders can't carry the agent's files into the checks (editable installs and workspace links that point back into the project; a venv changed during the session); `require_clean`; `countercheck hook stop` | v0.18.0 |
| `countercheck hook install / uninstall / status` for Claude Code, Cortex Code, Cursor, Codex, GitHub Copilot, Goose and Kiro, each guarded by the tamper check; strict mode (`--scope managed`); a check that only needs the blocked internet is UNVERIFIED, confirmed on the base (ADR-0005); which agents can be stopped at "done" (`docs/hooks/`) | v0.19.0 |
| Check types: **matrix**, **forbid**, **golden**, **budget**, **coverage** (`docs/checks.md`) + made-up data-migration example (`examples/data-migration/`) | v0.2.0 |
| Fix: on Linux, a timed-out check could make `kill` signal every process (`kill -KILL -<pgid>` misread by procps-ng 4.0.4); now `kill -s KILL -- -<pgid>` + a test | v0.2.0 |
| Mutation + property tests for the verdict logic (32 → 69 tests); fixes: receipts with a cost read back as BROKEN, `./` and `..` paths in protected matching | v0.2.0 |

## In progress

- **Benchmark results** published: `docs/eval/results-2026-10.md` (GPT 6.1: false "done" 7 → 3, impossible-task
  cheats 6 → 0; DeepSeek: no difference). Claude Fable 5.1 and Opus 5.5 (Oct 2026): no gaming, no false alarms, no added time. Next: sandbox the agent during evals (and keep the provider key out of its shell), adapt the harness to Hermes v0.22.1+.

## Next

1. **Per-host network allow-list** (today `network = true` opens the whole internet for that check; the macOS sandbox
   can only filter by IP and port, not host name).
2. **Test with real users**: Claude Code users, and a data team converting PL/SQL packages to Snowflake with
   Cortex Code (same hook format; confirm it on a real Cortex Code install).
3. **Recipe #2 and #3 from different fields** (a dbt project, a web app), so we learn which helpers are shared.
4. **Coverage lists built from source files**: `coverage` takes its items from a pattern over locked files (e.g.
   every `CREATE PROCEDURE` name in `oracle/`, every route in the old app) instead of a hand-written list, with a
   rename rule for names that change (`ORDER_API.CREATE_ORDER` → `ORDER_API_CREATE_ORDER`). Generalises the Oracle
   recipe's inventory script to any migration. Build once 2–3 recipes need it, not before.
5. **Benchmark on more models**: Claude Opus / Sonnet on the trap and impossible tasks.
6. **Targets (profiles)**: named targets in `checks.toml` that point at connections defined elsewhere (e.g.
   `~/.snowflake/connections.toml`); never credentials in Countercheck files. Needed for checks that run against a
   database.
7. **Network allow-list for checks** that must reach one service (a test database) while the sandbox stays on.
8. **More hooks**: Codex, Cursor. The GitHub Action could also post the verdict as a PR comment (today: job summary
   and artifact), and be listed on the GitHub Marketplace (needs its own repository).
9. **Windows full sandbox, after the experimental release** (ADR-0004, built: `countercheck windows-setup`): try it on
   real Windows 11 machines and drop "experimental"; then the two follow-ups: no stored password for the runner
   account, and `network = true` checks fully sandboxed (a second account without the firewall rule).
10. **winget and scoop** packages for Windows (today: `install.ps1`, `pip`/`uvx`, or the zip from Releases).

## Later

### Enterprise (Windows-first)

Most enterprises run Windows, where developers usually have no admin rights but IT can deploy software centrally.

- **Docs: "Countercheck in an enterprise"**: CI as the gate (`verify --base origin/main` on controlled runners), WSL2
  on Windows laptops (the full Linux sandbox today), and what each receipt `sandbox` label means.
- **Authenticode-signed `countercheck.exe`** (a code-signing certificate or Azure Trusted Signing): unsigned binaries
  get SmartScreen warnings and are blocked by AppLocker/WDAC allow-lists in many companies.
- **Packaging IT can deploy**: winget and an MSI (Intune, Group Policy).
- **Managed full sandbox**: the Windows admin setup (Next, item 9) packaged for IT to deploy once, so developers
  never need admin; must handle policies that restrict extra local accounts.
- **Sandbox policy**: an org or check-file setting like `require_sandbox = "full"`, so a run without the full sandbox
  is UNVERIFIED instead of VERIFIED (what compliance teams ask for).
- **Signed receipts** that say who produced a receipt, for audit (see below).
- Before building the managed sandbox: talk to 2-3 enterprise IT/security people as design partners ("would you
  deploy this, what would block it?").

### Other

- **More recipes** (data migration, web app, games, video/media, ML models), community-maintained; started in
  `recipes/`.
- **Data migration helpers**: generate reconciliation checks from table metadata (row counts, nulls, sums,
  distinct counts, table hashes); capture golden outputs from the old system once.
- **Blind spot: hard-coded answers.** Locked tests stop test edits, not code that copies the expected value.
  Ideas: flag diffs that add literals copied from protected tests; hidden checks the agent never sees.
- **Windows on ARM64** build (`aarch64-pc-windows-msvc`; the x86_64 build runs under emulation today).
- **Receipt index**: a rebuildable local index (e.g. SQLite) over the JSON receipts for "which targets failed this
  week?" questions. JSON receipts stay the source of truth.
- **Signed receipts** that third parties can verify.
- Mutation testing for `hook.rs`.
- Kill process groups with a system call (e.g. `rustix`) instead of the `kill` program, so it doesn't depend on which `kill` a system ships.

## Not planned

- Countercheck writing the tests itself. A check is only worth something if it comes from outside the agent
  being checked; AI may *suggest* inputs, but the expected answers come from a person or a reference system.
