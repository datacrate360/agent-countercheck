# AGENTS.md: guide for AI agents and contributors

omni-agent checks whether an AI worker's "done" is really done and writes a receipt. It is a **referee**, so it must
be more trustworthy than the agents it checks.

## Map

- `crates/omni-core`: pure logic, no I/O: `config` (`.omni/checks.toml`), `protect` (protected paths, junk files),
  `receipt` (receipt, verdict rules, seal).
- `apps/omni`: the `omni` binary: `git.rs` (git CLI wrappers), `snapshot.rs` (clean copy, restore, tree hash),
  `runner.rs` (run a check, seatbelt sandbox, timeouts), `verify.rs` (the flow), `main.rs` (CLI).
- `apps/omni/tests/verify.rs`: end-to-end tests with real git repos and the real binary.
- `research/`, `reports/`, `docs/decisions/`: research and decisions (read `docs/decisions/0001-stack.md`).

## Golden rules

1. **Never crash.** Non-test code has no `unwrap`/`expect`/`panic!`/`todo!`/`unreachable!` (denied at each crate
   root); `unsafe` is forbidden. Return a plain-English error.
2. **The worker never grades itself.** Checks come from the base revision. Protected files are restored from base.
   Never add an option that lets the worker's tree change what is checked without the receipt saying so.
3. **Honest verdicts.** Only `verified` when locked checks ran and passed. Anything omni can't prove is `unverified`,
   with a reason. Keep reasons short and plain; noise makes people stop trusting a checker.
4. **Everything from the repo is hostile.** Validate paths (no absolute, no `..`), cap sizes, never follow symlinks
   out of the snapshot.
5. **Tests are the gate.** Every behaviour change gets an end-to-end test in `apps/omni/tests/verify.rs`.

## Before you finish

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test
```

Work happens on branch `DEV`; promotion DEV → TEST → PROD.
