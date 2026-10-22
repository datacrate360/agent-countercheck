# AGENTS.md: guide for AI agents and contributors

Countercheck checks whether an AI worker's "done" is really done and writes a receipt. It is a **referee**, so it must
be more trustworthy than the agents it checks. Read [docs/architecture.md](docs/architecture.md) before changing
code: the rules, one run step by step, and where each part lives.

## Map

- `crates/countercheck-core`: pure logic, no I/O: `config` (`.countercheck/checks.toml`, matrix expansion), `protect`
  (protected paths, junk files), `receipt` (receipt, verdict rules, seal), `claim` (does an answer report a blocker),
  check types `matrix`, `forbid`, `golden`, `budget`, `coverage`.
- `crates/countercheck-winsandbox`: the Windows sandbox's Win32 calls (restricted token, file permissions) behind a
  safe API; the **only crate allowed `unsafe`**, every block with a `SAFETY:` note (ADR-0003).
- `apps/countercheck`: the `countercheck` binary: `git.rs` (git CLI wrappers), `snapshot.rs` (clean copy, restore,
  tree hash), `runner.rs` (run a check, macOS sandbox, timeouts, budget), `linux_sandbox.rs` / `windows_sandbox.rs` (the
  `sandbox-exec` helper), `scan.rs` (forbid + coverage scans),
  `golden.rs`, `verify.rs` (the flow), `hook.rs` (agent hooks), `main.rs` (CLI).
- `apps/countercheck/tests/verify.rs`: end-to-end tests with real git repos and the real binary.
  `crates/countercheck-core/tests/`: property tests and claim fixtures.
- `examples/`: one made-up project per domain. `eval/` + `docs/eval/`: the benchmark (tasks with hidden tests).
- `docs/`: `checks.md` (every check type), `hooks/`, `decisions/`, `ROADMAP.md`. Market and ecosystem research is
  kept outside this repo; keep this repo about the tool.

## Skills (task guides)

Follow the matching guide in `.claude/skills/` (Claude Code loads them automatically; other agents: read the file):

| Task | Skill |
|---|---|
| Show Countercheck on a new domain | `.claude/skills/add-domain-example/SKILL.md` |
| Add a benchmark task | `.claude/skills/add-eval-task/SKILL.md` |
| Hook another agent | `.claude/skills/add-agent-hook/SKILL.md` |
| Add a check type | `.claude/skills/add-check-type/SKILL.md` |
| Write tests for any Rust change | `.claude/skills/write-tests/SKILL.md` |

## Golden rules

1. **Never crash.** Non-test code has no `unwrap`/`expect`/`panic!`/`todo!`/`unreachable!` (denied at each crate
   root); `unsafe` is forbidden (`#![forbid(unsafe_code)]`). **One exception, not a precedent:**
   `crates/countercheck-winsandbox`, the Win32 calls of the Windows sandbox that no safe crate offers (ADR-0003),
   under these rules: `unsafe` only in its Windows-only `sys` module (`cfg(windows)`; forbidden on every other OS
   and in its other modules); each block as small as possible, one unsafe operation, with a `// SAFETY:` comment
   saying why it is sound (`clippy::undocumented_unsafe_blocks` and `clippy::multiple_unsafe_ops_per_block` are
   errors there); only safe functions leave the crate. New FFI goes there under the same rules, after the maintainer
   agrees; never in another crate. Return a plain-English error.
2. **The worker never grades itself.** Checks come from the base revision. Protected files are restored from base.
   Never add an option that lets the worker's tree change what is checked without the receipt saying so.
3. **Honest verdicts.** Only `verified` when locked checks ran and passed. Anything Countercheck can't prove is
   `unverified`, with a reason. A hint may explain a failure but never changes the verdict. Keep reasons short and
   plain; noise makes people stop trusting a checker.
4. **Everything from the repo is hostile.** Validate paths (no absolute, no `..`), cap sizes, never follow symlinks
   out of the snapshot.
5. **Domain-blind core.** No SQL, game or ML knowledge in Rust; domains live in examples and check files.
6. **Tests are the gate.** Every behaviour change gets a test that fails without it, usually end-to-end in
   `apps/countercheck/tests/verify.rs`.

## Before you finish

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo build --release && python3 eval/validate.py
```

Commits are signed off (`git commit -s`, see CONTRIBUTING.md). Work happens on branch `DEV`; promotion DEV → TEST →
PROD.
