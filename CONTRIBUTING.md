# Contributing to Countercheck

Thanks for helping. Countercheck checks whether AI work really happened, so it has to be more trustworthy than the
agents it checks. That shapes the rules below.

## Quick start

```sh
git clone https://github.com/datacrate360/countercheck && cd countercheck
cargo build --release
cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test
```

Rust 1.90+. macOS is the main platform today (the check sandbox uses macOS Seatbelt); Linux support is planned.

## How changes flow

- Open pull requests against the **`DEV`** branch. `TEST` and `PROD` are release branches maintained by the owner.
- One topic per pull request. Describe what changed and how you tested it.
- CI must pass (format, clippy with warnings as errors, all tests).

## Rules (read [AGENTS.md](AGENTS.md) for the full list)

1. **Never crash.** No `unwrap`/`expect`/`panic!` outside tests; no `unsafe`. Return a plain-English error.
2. **The worker never grades itself.** Checks come from the base revision; protected files are restored from base.
   Changes that weaken this need an issue and a discussion first.
3. **Honest verdicts.** `verified` only when locked checks ran and passed; anything unproven is `unverified` with a
   reason.
4. **Tests are the gate.** Every behaviour change gets an end-to-end test in `apps/countercheck/tests/verify.rs`.
5. **No telemetry, no network calls** from Countercheck itself.

## Good first contributions

- `countercheck init` detecting the project's test command (pytest, npm, cargo, go, dbt).
- Hooks for more agents (Claude Code, Codex, OpenCode).
- A Linux sandbox (bubblewrap or namespaces).
- New tasks for the evaluation set in `eval/tasks.py` (each must pass `python3 eval/validate.py`).

## Reporting security problems

Please don't open a public issue; see [SECURITY.md](SECURITY.md).

By contributing you agree that your work is released under the [MIT License](LICENSE).
