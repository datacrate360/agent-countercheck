---
name: add-check-type
description: Use when adding a new check type or option to .countercheck/checks.toml (like matrix, forbid, golden, budget, coverage). Covers the design bar, the files to touch, the security rules and the tests.
---

# Add a check type

Check types are Countercheck's core vocabulary, so they are the hardest contribution. Read `docs/architecture.md`
and `docs/checks.md` first, and open an issue before coding.

## The design bar (answer these in the issue)

1. **Can a plain `[[check]] run = "…"` do it?** If yes, write a domain example instead (`add-domain-example` skill).
   New types are for what a command can't do safely: reading files before commands run, comparing against references
   the worker can't edit, enforcing lists the worker can't shrink.
2. **Is it domain-blind?** "Compare a CSV within a tolerance" is general; "check Snowflake syntax" is not.
3. **Where does every input come from?** Anything that decides pass/fail (lists, references, limits) must come from
   the base revision or a `protected` path, never only from the worker's tree.
4. **How can a worker game it?** Name the cheats and how the type stops each one.

## Files to touch (follow `budget`, the smallest existing type)

| Step | File |
|---|---|
| Spec struct + `validate` + comparison logic (pure, no I/O) | new `crates/countercheck-core/src/<type>.rs`, `pub mod` in `lib.rs` |
| Parse it: a field on `CheckSpec` (or a new `[[table]]` on `ChecksFile`), validated in `parse`, copied into `PlannedCheck` in **both** places in `plan_inner` | `crates/countercheck-core/src/config.rs` |
| Run it: commands → `runner.rs`; scans of the worker's files before commands → `scan.rs`; locked reference files → read from base in `verify.rs` | `apps/countercheck/src/` |
| Results appear as `CheckResult`s so `decide` and the seal cover them; never add a separate verdict path | `receipt.rs` |
| Document with an example, limits and failure output | `docs/checks.md` (table at the top + a section) |
| Changelog line under "Unreleased" | `CHANGELOG.md` |

## Security rules

- Validate everything at parse time with a plain error: paths relative, no `..`, under `.countercheck/` when it's a
  reference; sizes and counts capped; regexes with a size limit.
- Never follow symlinks out of the clean copy; skip binary and huge files and say so in the output.
- Failure output must say what is wrong in a few lines (cap long lists at ~20 items) so the agent can fix it.
- Old check files must parse exactly as before; the receipt stays backward-compatible (new fields optional with
  `#[serde(default)]`).

## Tests (see the `write-tests` skill)

- Core unit tests: parsing (good and every rejected input) and the comparison.
- End-to-end in `apps/countercheck/tests/verify.rs`: pass, fail with a clear message, each cheat from the design bar
  fails, works with `matrix` if it makes sense.
- If it adds a receipt field: add it to the property test's seal-edit list.
- Extend an example in `examples/` to use the type.

Done when the gate passes and `docs/checks.md` shows a copy-pasteable example.
