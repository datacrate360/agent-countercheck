# ADR-0002: Checks from outside the repository (`--checks-from`)

Status: accepted (2026-10-21, decisions 1-3 approved by the maintainer), for [issue #20](https://github.com/datacrate360/agent-countercheck/issues/20).

## The problem

Countercheck only trusts checks committed in the target repository at the base revision (`verify.rs`, reading
`.countercheck/checks.toml` from `--base`). That rules out verifying work on a repository you don't control: a
contribution to an upstream project whose rules forbid adding `.countercheck/`, or one team's shared check suite used
across many repositories. The workaround today is six manual steps (a local-only base branch with the check file, a
cherry-pick, `--base` on that branch), and the receipt then names a base commit that upstream never had.

## The trap in the obvious design

"Read the checks from a file outside the repository" sounds safe, but **outside the worker's tree is not outside the
worker's reach**. A coding agent has a shell: it can edit `~/countercheck-suites/project.toml` as easily as
`tests/test_x.py`. Recording the file's hash on the receipt makes the edit *detectable afterwards*, not *impossible*:
the receipt would say VERIFIED for checks the agent weakened, with a hash nobody compares against anything.

Golden rule 2 (the worker never grades itself) therefore needs the external checks to be **pinned by something the
agent can't change during its run**: a hash or commit given on the command line by the person or CI job that starts
`verify`, not found on disk by `verify`.

## Decision

Two forms, both read before the worker's files are examined:

| Form | Example | Pinned by | Verdict can be VERIFIED |
|---|---|---|---|
| **A git commit of another repository** | `--checks-from ../suites@3f9c2e1d…(40 hex):hermes-agent/checks.toml` | the full commit id (a branch or tag name is refused: they move) | yes |
| **A file + its SHA-256** | `--checks-from ~/suites/hermes.toml --checks-sha256 ab12…` | the hash on the command line | yes |
| A file without `--checks-sha256` | `--checks-from ~/suites/hermes.toml` | nothing | **no: UNVERIFIED** ("external checks not pinned"), like uncommitted checks today |

Rules:

1. **Read like the base.** The git form reads through the same hash-checked object reader as the base revision
   (`git::Revision`, objects re-hashed, `--no-replace-objects`), from the suite repository at that commit. The file form
   reads the file once, hashes it, and compares with `--checks-sha256`; a mismatch is an error (exit 1), never a verdict.
2. **Reference files come from the same place.** Matrix files, coverage item lists and golden expected files named in
   the external check file are read from the suite: in the git form, by the same relative path in the suite commit; in
   the file form, **not supported in v1** (an external check file that names other files is refused with a plain
   error), because each extra file would need its own pin.
3. **The target repository still supplies the work and the base.** `--base` is the real upstream base (e.g.
   `origin/main`); protected files are restored from it; diffs, `uncovered` and `watched` are against it. Protected
   patterns, `watched` and every other rule come from the external check file.
4. **External replaces, never merges.** If the target repository also has `.countercheck/checks.toml`, it is ignored and
   the receipt says so. Merging two rule sources would let a worker-editable file add or soften rules.
5. **Refuse paths inside the target repository**: a `--checks-from` file (after symlinks) inside the worker's tree is
   an error, even with a pin, because it hides the external-ness the flag is for.
6. **The receipt names it.** `checks_source` becomes e.g.
   `external: ../suites@3f9c2e1d:hermes-agent/checks.toml` or `external: ~/suites/hermes.toml sha256:ab12… (pinned)`,
   and a new optional receipt field `checks_external = { kind, source, sha256 }` (older receipts still verify).
7. **CLI only in v1.** Hooks keep reading the repository's own checks. An agent can edit its harness settings, so a
   hook-configured external source needs its own design (likely an environment variable set by the person, plus the
   pin).

## How a worker could try to game it, and what stops it

| Cheat | Stopped by |
|---|---|
| Edit the external file during the run | the pin: `--checks-sha256` mismatch is an error; the git form reads a fixed commit |
| Commit a weaker suite to the suite repository | the commit id on the command line doesn't move |
| Move a branch or tag the command line names | branch/tag names are refused; only a 40-hex commit id |
| Rewrite objects in the suite repository's `.git` | the same re-hashing reader as the base (v0.5.1) |
| Add a stronger-looking `.countercheck/checks.toml` to the target repo to soften the suite | external replaces, never merges; the receipt says the repo's own file was ignored |
| Point `--checks-from` at a file inside the work tree | refused after symlink resolution |
| Run without the pin | UNVERIFIED, with the reason |

## What it costs

- A new input path through `verify` (two sources of truth for "where the rules come from"), so every check type's
  reference-file read must take a "suite" reader instead of the base. That is the riskiest part of the change and gets
  the most tests.
- No hook support in v1.

## Tests it needs (acceptance)

- Git form on a repo with no `.countercheck/`: VERIFIED for an honest fix, FAILED for a defect, and an edited
  protected test is restored from `--base`.
- Git form with a branch name, a short id, or a missing path: plain errors.
- File form with the right pin: can be VERIFIED; wrong pin: error; no pin: UNVERIFIED with the reason; file inside the
  repo: error; a file naming a golden/matrix/coverage file: error.
- A rewritten object in the suite repository is refused.
- The repo's own `checks.toml` is ignored and the receipt says so.
- Receipt: `checks_external` in the property test's seal-edit list; old receipts verify.
- Without `--checks-from`, behaviour and receipts are byte-for-byte unchanged (existing tests).
