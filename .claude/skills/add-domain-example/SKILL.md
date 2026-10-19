---
name: add-domain-example
description: Use when showing Countercheck on a new kind of project (games, ML, dbt, web, video, a migration) under examples/. Builds a small runnable example with locked checks, a planted bug the checks catch, scripted cheats that still fail, and a script that proves it. No Rust changes.
---

# Add a domain example

An example shows people in one field how to write checks for their work. It is the easiest and most valuable
contribution: Countercheck's core knows nothing about any domain, so examples are where domain knowledge lives.

**Example or recipe?** An example teaches the check types on made-up data. A ready starting point for a real
job (check files + helper scripts + a tested 5-minute start) is a **recipe**: follow `recipes/README.md` "Add a
recipe" instead.

**Templates:** copy `examples/ml-eval-threshold/` for the script (`run-example.sh`: bug → cheats → fix, exits 1 on
any surprise), and read `examples/data-migration/` for more check types (matrix, coverage).

## Rules

- **Made-up, tiny data only.** No real customer data, no secrets, no downloads. Everything runs offline in seconds.
- **No new dependencies** beyond `sh`, `git` and the `python3` standard library (or the domain's own tool, if the
  README says how to install it and the script skips cleanly when it's missing).
- **Plant at least one realistic bug** an agent could leave behind while claiming "done": one that doesn't crash
  (wrong data split, off-by-one, silently dropped rows).
- Use the existing check types (`docs/checks.md`). If none can express what the domain needs, open a "New domain"
  issue instead of writing Rust.

## Where files go

| File | Location | Why |
|---|---|---|
| the work (code, SQL, configs) | anywhere in the example | the agent changes these |
| truth the agent must not edit (datasets, tests, source data) | outside `.countercheck/`, listed in `protected` | restored from the base before checks run |
| locked references (golden files, coverage lists, matrix files) | under `.countercheck/` | always read from the base revision |
| generated output (`reports/`, `target/`) | in the example's `.gitignore` | never committed, never left in `examples/` |

## Procedure

1. Name the folder `examples/<field>-<task>/` (e.g. `game-asset-pipeline`, `dbt-model-refactor`): someone in that
   field should recognise it from the name alone.
2. Write the work and the truth files (table above). Keep it under ~15 files.
3. Write `.countercheck/checks.toml`: `protected` first, then the checks, with a comment above each saying which
   mistake it catches.
4. Make golden files by running the **fixed** code once and saving its output under `.countercheck/golden/`. Check
   it by hand: it defines "right".
5. Plant the bug. Keep the fix as `fix.patch` (`git diff > fix.patch` from the fixed state); the script deletes it
   from the copy and applies it with `git apply "$HERE/fix.patch"`.
6. Write `run-example.sh` (copy the ML one):
   - copy the example to `mktemp -d`, delete `run-example.sh` and `fix.patch` from the copy, `git init`, commit
     everything as the **base**;
   - make every later change **without committing**: `countercheck verify` compares the uncommitted work tree with
     the base (`--base HEAD`), which is how an agent's work looks before review;
   - judge each verdict by **exit code**: `0` VERIFIED, `2` FAILED, `3` UNVERIFIED, `1` Countercheck couldn't run;
   - steps: planted bug → FAILED; at least one **scripted cheat** → FAILED (undo it with `git checkout -- <paths>`);
     fix → VERIFIED; `exit 1` on any other result;
   - pass `--sandbox none`, so it also runs on Linux and in CI (the macOS sandbox is tested elsewhere).
7. Write `README.md`: what the made-up project is (say the data is made up), a file table, each check and the mistake
   it catches, each scripted cheat and why it fails, how to run it.
8. Add a row to `examples/README.md`, and a step to the `linux` job in `.github/workflows/ci.yml`.

## Verification

```sh
cargo build                                                          # or any recent countercheck binary
sh examples/<name>/run-example.sh "$PWD/target/debug/countercheck"   # ends "all verdicts as expected", exit 0
```

An examples-only change doesn't need `cargo test`; CI runs everything anyway.

## Pitfalls

- **A budget alone can be faked**: the last `METRIC name=value` line counts, so an agent can just print a good
  number. Pair a budget with a `forbid` on hard-coded metric lines, or a `golden` on the real output.
- **Don't commit inside the script.** A commit becomes the new `HEAD`, and with the default `--base HEAD` the
  "locked" files would come from it. (Agent hooks avoid this with `countercheck hook session-start`.)
- `countercheck verify` needs the checks at the root of a git repo: never run it inside `examples/` directly.
- The console shows only the last lines of a failing check; the receipt JSON (`output_tail`) has more.
- Matrix values must match `^[A-Za-z0-9_.-]{1,128}$`. Checks have no internet on macOS unless `network = true`.
