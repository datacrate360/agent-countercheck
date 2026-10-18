# AGENTS.md: guide for AI agents and contributors

Countercheck checks whether an AI worker's "done" is really done and writes a receipt. It is a **referee**, so it must
be more trustworthy than the agents it checks.

## Map

- `crates/countercheck-core`: pure logic, no I/O: `config` (`.countercheck/checks.toml`), `protect` (protected paths, junk files),
  `receipt` (receipt, verdict rules, seal).
- `apps/countercheck`: the `countercheck` binary: `git.rs` (git CLI wrappers), `snapshot.rs` (clean copy, restore, tree hash),
  `runner.rs` (run a check, seatbelt sandbox, timeouts), `verify.rs` (the flow), `main.rs` (CLI).
- `apps/countercheck/tests/verify.rs`: end-to-end tests with real git repos and the real binary.
- `eval/` + `docs/eval/`: the evaluation against Hermes Agent (tasks with hidden tests, runner, report).
- `docs/decisions/`: design decisions (read `docs/decisions/0001-stack.md`). Market and ecosystem research is kept
  outside this repo; keep this repo about the tool.

## Golden rules

1. **Never crash.** Non-test code has no `unwrap`/`expect`/`panic!`/`todo!`/`unreachable!` (denied at each crate
   root); `unsafe` is forbidden. Return a plain-English error.
2. **The worker never grades itself.** Checks come from the base revision. Protected files are restored from base.
   Never add an option that lets the worker's tree change what is checked without the receipt saying so.
3. **Honest verdicts.** Only `verified` when locked checks ran and passed. Anything Countercheck can't prove is `unverified`,
   with a reason. Keep reasons short and plain; noise makes people stop trusting a checker.
4. **Everything from the repo is hostile.** Validate paths (no absolute, no `..`), cap sizes, never follow symlinks
   out of the snapshot.
5. **Tests are the gate.** Every behaviour change gets an end-to-end test in `apps/countercheck/tests/verify.rs`.

## Before you finish

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test
```

Work happens on branch `DEV`; promotion DEV → TEST → PROD.
