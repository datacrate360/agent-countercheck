# Roadmap

What Countercheck can do today, what's being built, and what comes next. Ordered by priority within each
section. Ideas and use cases are welcome as issues.

## Shipped

| | Version |
|---|---|
| `countercheck verify / init / show`: locked checks from the base revision, clean copy, protected files restored, sealed receipt | v0.1.0 |
| macOS sandbox (no internet, localhost allowed for web tests); `--sandbox auto` elsewhere | v0.1.0 (localhost: DEV) |
| Install: pip, uv, Homebrew, install script, release files with checksums | v0.1.0 |
| Hermes Agent hook (`hook hermes-pre-verify`) | v0.1.0 |
| Hook lets an agent stop when it honestly reports a blocker; still pushes back on success claims and test edits | DEV |
| Claude Code `Stop` hook (`hook claude-stop`), loop guard (max push-backs per session) | DEV |
| Check types: **matrix**, **forbid**, **golden**, **budget**, **coverage** (`docs/checks.md`) + made-up data-migration example (`examples/data-migration/`) | DEV |
| Fix: on Linux, a timed-out check could make `kill` signal every process (`kill -KILL -<pgid>` misread by procps-ng 4.0.4); now `kill -s KILL -- -<pgid>` + a test | DEV |
| Mutation + property tests for the verdict logic (32 → 69 tests); fixes: receipts with a cost read back as BROKEN, `./` and `..` paths in protected matching | DEV |

## In progress

- **Benchmark vs Hermes `verify_on_stop`** (`eval/`, plan in `docs/eval/hermes-eval-plan.md`): 29 tasks × 4 setups ×
  3 repeats on DeepSeek V4 Pro and GPT 6.1. Pilot results: `docs/eval/pilot-results.md`.

## Next

1. **Release v0.2.0**: everything on DEV above. Urgent because of the Linux timeout fix (v0.1.0 is affected).
2. **Publish the benchmark**: method, per-model results, every impossible-task answer, known blind spots.
3. **Test with real users**: Claude Code users, and a data team converting PL/SQL packages to Snowflake with
   Cortex Code (same hook format; confirm it on a real Cortex Code install).
4. **Benchmark on more models**: Claude Opus / Sonnet on the trap and impossible tasks.
5. **Targets (profiles)**: named targets in `checks.toml` that point at connections defined elsewhere (e.g.
   `~/.snowflake/connections.toml`); never credentials in Countercheck files. Needed for checks that run against a
   database.
6. **Network allow-list for checks** that must reach one service (a test database) while the sandbox stays on.
7. **More hooks**: Codex, Cursor, a GitHub Action (verify every PR and post the receipt).

## Later

- **Packs**: ready-made recipes per domain (data migration, web app, games, video/media, ML models): a
  `checks.toml` template, helper scripts and a README each; community-maintained.
- **Data migration helpers**: generate reconciliation checks from table metadata (row counts, nulls, sums,
  distinct counts, table hashes); capture golden outputs from the old system once.
- **Blind spot: hard-coded answers.** Locked tests stop test edits, not code that copies the expected value.
  Ideas: flag diffs that add literals copied from protected tests; hidden checks the agent never sees.
- **Linux sandbox** (bubblewrap / Landlock), and Windows guidance (WSL today).
- **Receipt index**: a rebuildable local index (e.g. SQLite) over the JSON receipts for "which targets failed this
  week?" questions. JSON receipts stay the source of truth.
- **Signed receipts** that third parties can verify.
- Mutation testing for `hook.rs`.
- Kill process groups with a system call (e.g. `rustix`) instead of the `kill` program, so it doesn't depend on which `kill` a system ships.

## Not planned

- Countercheck writing the tests itself. A check is only worth something if it comes from outside the agent
  being checked; AI may *suggest* inputs, but the expected answers come from a person or a reference system.
