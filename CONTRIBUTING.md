# Contributing to Countercheck

Thanks for helping. Countercheck checks whether AI work is really done, so it has to be more trustworthy than the
agents it checks. Start with [docs/architecture.md](docs/architecture.md): the rules behind every decision, one run
step by step, and where your change plugs in.

## Ways to contribute (easiest first)

| Contribution | Rust needed? | Guide |
|---|---|---|
| **Try it on your project and tell us what broke or was confusing** | no | open a "Bug" issue |
| **A domain example**: show how your field (games, ML, dbt, web, video, a migration) writes checks | no | [add-domain-example](.claude/skills/add-domain-example/SKILL.md) |
| **A benchmark task**: a small task with hidden tests that tempts agents to fake "done" | no (Python) | [add-eval-task](.claude/skills/add-eval-task/SKILL.md) |
| **Docs**: clearer wording, a missing case | no | |
| **A hook for another agent** (Codex, Cursor, OpenCode, Aider, …) | yes | [add-agent-hook](.claude/skills/add-agent-hook/SKILL.md) |
| **Tests**: kill a surviving mutant, add a property, add a real agent answer to the claim fixtures | yes | [write-tests](.claude/skills/write-tests/SKILL.md) |
| **Smarter `countercheck init`**: detect more test commands (go, dbt, gradle, …) | yes | [write-tests](.claude/skills/write-tests/SKILL.md) |
| **A check type** (discuss in an issue first) | yes | [add-check-type](.claude/skills/add-check-type/SKILL.md) |
| **Bigger items**: Linux sandbox, signed receipts, GitHub Action | yes | [docs/ROADMAP.md](docs/ROADMAP.md), open an issue first |

The guides are skills in `.claude/skills/`: Claude Code loads them automatically, other agents can read them (plain
Markdown), and they work as checklists for people too.

**Making Countercheck fit your domain** almost never needs a change to Countercheck: the core is domain-blind, and a
domain is covered by a good `checks.toml` plus an example. If your field needs something no check type can express,
open a "New domain" issue: that's exactly the feedback we need.

## Set up

```sh
git clone -b DEV https://github.com/datacrate360/agent-countercheck && cd agent-countercheck
cargo build --release
```

Rust 1.90+. macOS runs checks in its sandbox; Linux and Windows run them without one (the receipt says so).

**Windows:** install [Rust](https://rustup.rs) (MSVC toolchain) and [Git for Windows](https://git-scm.com/download/win).
The end-to-end tests and most checks use Git for Windows' `sh`; Countercheck finds it on `PATH` or next to `git.exe`.
The repository's `.gitattributes` keeps LF line endings, so shell scripts run as they do on Unix. Run the gate below
from PowerShell or Git Bash; the last line (`eval/validate.py`) needs the macOS sandbox, so on Windows and Linux CI
runs the example instead: `sh examples/ml-eval-threshold/run-example.sh "$PWD/target/debug/countercheck.exe"`.

## The gate (CI runs the same on macOS, Linux and Windows)

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo build --release && python3 eval/validate.py
```

Every behaviour change needs a test that fails without the change and passes with it.

## Rules (the full list is in [AGENTS.md](AGENTS.md))

1. **The worker never grades itself.** Checks come from the base revision; protected files are restored from base.
   Changes that weaken this need an issue and a discussion first.
2. **Honest verdicts.** `verified` only when locked checks ran and passed; anything unproven is `unverified` with a
   reason; a hint never softens a failure.
3. **Never crash.** No `unwrap`/`expect`/`panic!` outside tests; no `unsafe`.
4. **No telemetry.** Countercheck itself never calls home; only checks that opt in (`network = true`) use the internet.
5. **Domain-blind core.** Domain knowledge goes in examples and check files, not in the Rust code.

## Pull requests

- Open them against **`DEV`**. GitHub shows `PROD` (the latest release) by default and suggests it as the base:
  switch the base to `DEV`. `TEST` and `PROD` are release branches maintained by the owner.
- One topic per pull request; fill in the template (what, why, how tested).
- Add a line under "Unreleased" in [CHANGELOG.md](CHANGELOG.md) for anything users notice.
- AI-assisted contributions are welcome. You are responsible for them: you've read the code, run the gate, and can
  explain every line.

## Sign your commits (DCO)

We use the [Developer Certificate of Origin](https://developercertificate.org/) instead of a contributor licence
agreement: one line in each commit saying you wrote the change (or have the right to submit it) under the project's
licence.

```sh
git commit -s -m "your message"     # adds: Signed-off-by: Your Name <you@example.com>
```

Forgot? `git commit --amend -s` (last commit) or `git rebase --signoff DEV` (all of them), then force-push your branch.
CI checks every commit in a pull request.

Your contribution is released under the [MIT License](LICENSE), like the rest of the project.

## Conduct and security

Be kind and constructive: [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md). Security problems go through a private report, not
a public issue: [SECURITY.md](SECURITY.md).
