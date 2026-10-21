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
| Check types: **matrix**, **forbid**, **golden**, **budget**, **coverage** (`docs/checks.md`) + made-up data-migration example (`examples/data-migration/`) | v0.2.0 |
| Fix: on Linux, a timed-out check could make `kill` signal every process (`kill -KILL -<pgid>` misread by procps-ng 4.0.4); now `kill -s KILL -- -<pgid>` + a test | v0.2.0 |
| Mutation + property tests for the verdict logic (32 → 69 tests); fixes: receipts with a cost read back as BROKEN, `./` and `..` paths in protected matching | v0.2.0 |

## In progress

- **Benchmark results** published: `docs/eval/results-2026-10.md` (GPT 6.1: false "done" 7 → 3, impossible-task
  cheats 6 → 0; DeepSeek: no difference). Next: sandbox the agent during evals, Claude models.

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
8. **More hooks**: Codex, Cursor, a GitHub Action (verify every PR and post the receipt).
9. **Windows sandbox**: run checks in an AppContainer (or a restricted token + Job Object) with no internet and
   writes only in the check folder, like the macOS one. Windows runs checks without a sandbox today.
10. **winget and scoop** packages for Windows (today: `install.ps1`, `pip`/`uvx`, or the zip from Releases).

## Later

- **More recipes** (data migration, web app, games, video/media, ML models), community-maintained; started in
  `recipes/`.
- **Data migration helpers**: generate reconciliation checks from table metadata (row counts, nulls, sums,
  distinct counts, table hashes); capture golden outputs from the old system once.
- **Blind spot: hard-coded answers.** Locked tests stop test edits, not code that copies the expected value.
  Ideas: flag diffs that add literals copied from protected tests; hidden checks the agent never sees.
- **Linux sandbox** (bubblewrap / Landlock).
- **Windows on ARM64** build (`aarch64-pc-windows-msvc`; the x86_64 build runs under emulation today).
- **Receipt index**: a rebuildable local index (e.g. SQLite) over the JSON receipts for "which targets failed this
  week?" questions. JSON receipts stay the source of truth.
- **Signed receipts** that third parties can verify.
- Mutation testing for `hook.rs`.
- Kill process groups with a system call (e.g. `rustix`) instead of the `kill` program, so it doesn't depend on which `kill` a system ships.

## Not planned

- Countercheck writing the tests itself. A check is only worth something if it comes from outside the agent
  being checked; AI may *suggest* inputs, but the expected answers come from a person or a reference system.
