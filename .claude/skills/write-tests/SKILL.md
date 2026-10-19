---
name: write-tests
description: Use when changing Countercheck's Rust code or adding tests to it. Says which test layer to use (unit, end-to-end, property, claim fixture, mutation), how to write each, and the gate every change must pass.
---

# Write tests for Countercheck

Countercheck is a referee: a bug can wave a false "done" through. Every behaviour change needs a test that **fails
before the change and passes after it**. Read `docs/architecture.md` ("How it is tested") first.

## Pick the layer

| Change | Test in | Pattern |
|---|---|---|
| Pure logic in `crates/countercheck-core` (parsing, verdict, receipt, a check type's comparison) | `#[cfg(test)] mod tests` in that module | plain values in, assert out; include the bad inputs that must be rejected |
| Anything a user sees from the binary (verdict, exit code, receipt, output, hooks) | `apps/countercheck/tests/verify.rs` | `project(checks_toml)` makes a throwaway git repo with a base commit; change files; `countercheck(dir, &[args])` returns `(exit_code, receipt_json)` |
| A rule that must hold for every input (seal breaks on any edit, `decide` never verifies a failure, paths never escape) | `crates/countercheck-core/tests/properties.rs` (proptest) | generate inputs, assert the rule; a new receipt field must be added to the seal-edit list (`EDITS`) |
| Blocker / success detection in agent answers | `crates/countercheck-core/tests/fixtures/claims/{success,blocker}/` | add the real agent answer as a file; `tests/claims.rs` picks it up |
| macOS sandbox behaviour | `verify.rs` with `#[cfg(target_os = "macos")]` and `if nested() { return; }` | macOS can't nest sandboxes; the guard skips inside an outer sandbox |

## Rules

- **Test the attack, not just the happy path.** For anything security-relevant, write the cheat an agent would try
  (edit a protected file, plant a fake output, a path with `..`, a symlink out) and assert it fails.
- **Assert the verdict and the reason.** Check `json["verdict"]` and that the receipt explains why.
- No network in tests except deliberate sandbox tests; no sleeps longer than needed; temp dirs only (`tempfile`).
- `unwrap`/`expect` are fine in tests, never in non-test code.

## Mutation testing (for core logic)

```sh
cargo install cargo-mutants
cargo mutants -p countercheck-core -f crates/countercheck-core/src/<module>.rs
```

Every "missed" mutant is a change to the logic no test noticed: add a test that kills it, or note in the PR why it's
equivalent (the change can't alter behaviour).

## The gate (CI runs the same)

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo build --release && python3 eval/validate.py
```

Done when all four pass and the new test was seen failing without your change (revert it once, run the test, restore).

## Pitfalls

- A new field on `CheckResult` or `Receipt` must be added to every struct literal (tests, `scan.rs`) and to the
  property test's edit list, or the seal coverage silently shrinks.
- End-to-end tests run the debug binary built by `cargo test`; don't rely on `target/release`.
- Linux runs checks without a sandbox: sandbox tests must be macOS-only.
