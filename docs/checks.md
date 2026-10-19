# Check types

Checks live in `.countercheck/checks.toml`. Countercheck reads that file from the **base revision** (before the
worker's change), runs every check in a clean copy of the worker's files, and puts back any protected file the worker
changed. Everything under `.countercheck/` is always protected.

A check result is `pass` or `FAIL`. Any failed check makes the verdict `FAILED`. The verdict is `VERIFIED` only when
the checks came from the base revision and all of them passed.

| Type | Use it when | Table |
|---|---|---|
| [Command](#command) | a command can tell right from wrong (tests, linters, builds) | `[[check]]` |
| [Matrix](#matrix) | the same command must pass for many targets (tenants, schemas, levels, files) | `[[check]]` + `matrix` |
| [Forbid](#forbid) | some text must not appear in the worker's files (leftover syntax, debug code, secrets) | `[[forbid]]` |
| [Golden](#golden) | a command's output file must match a reference you keep (exports, reports, renders) | `[[check]]` + `golden` |

## Command

Runs a command with `sh -c` in the clean copy. Exit code 0 is a pass.

```toml
protected = ["tests/**"]

[[check]]
name = "unit tests"
run = "pytest -q"
timeout_secs = 600          # optional, default 600, max 7200
```

## Matrix

Runs one check once per value. Each run is its own result on the receipt, named `<name> [<key>=<value>]`, so you see
exactly which targets failed.

```toml
[[check]]
name = "reconcile"
run = "python3 scripts/reconcile.py --schema {schema}"
matrix = { schema = ["CA1", "NYJUV1", "MN3"] }
```

The receipt then has `reconcile [schema=CA1]`, `reconcile [schema=NYJUV1]` and `reconcile [schema=MN3]`.

- `{schema}` in `run` is replaced by the value. The value is also in the environment as `COUNTERCHECK_MATRIX_SCHEMA`
  (the key in upper case).
- One key per matrix for now.
- Values may only use letters, digits, `_`, `.` and `-` (1 to 128 characters), because they go into a shell command.
  Anything else is refused when the check file is read.
- At most 1000 runs per check file, counting every value.

A long target list can live in a file, one value per line (`#` starts a comment):

```toml
[[check]]
name = "reconcile"
run = "python3 scripts/reconcile.py --schema {schema}"
matrix_file = { schema = ".countercheck/targets/schemas.txt" }
```

The file must be under `.countercheck/`. Like the check file, it is read from the base revision, so a worker can't
make the list shorter.

## Forbid

Fails when a pattern appears in the worker's files. Countercheck scans the clean copy itself, before any check
command runs, so no command is needed.

```toml
[[forbid]]
name = "no Oracle-only syntax left"
pattern = '\(\+\)|\bDECODE\s*\(|\bROWNUM\b'     # a regex
paths = ["snowflake/**/*.sql"]                     # globs, same matcher as `protected`
message = "Convert these to Snowflake syntax."    # optional, shown to the agent on failure
case_insensitive = true                            # optional, default false
```

- On the receipt it is a check named `forbid: <name>`.
- The pattern is matched line by line ([regex syntax](https://docs.rs/regex/latest/regex/#syntax); it always runs
  in linear time). Use single quotes in TOML so backslashes stay as written.
- A failure lists up to 20 hits as `path:line: text` (lines cut to 200 characters) and the total count, then the
  `message`.
- Binary files and files over 5 MiB are skipped and listed as skipped.
- When no file matches `paths`, the check passes and says "nothing to scan".
- An invalid regex is refused when the check file is read.

## Golden

After the command passes, Countercheck compares the file it wrote with a reference file you committed.

```toml
[[check]]
name = "monthly totals"
run = "python3 scripts/export.py > out/totals.csv"
golden = { output = "out/totals.csv", expected = ".countercheck/golden/totals.csv", mode = "csv", tolerance = 1e-6, sort_rows = true }
```

- `expected` must be under `.countercheck/` or match a `protected` pattern; anything else is refused, because the
  worker could edit it. With locked checks it is read from the base revision.
- Any `output` file already in the worker's tree is removed before the command runs, so only what the command
  writes counts. A missing output fails with a plain message.
- `mode = "text"` (the default): exact match after normalising line endings. A failure shows the first differing
  line number with both lines.
- `mode = "csv"`: the header must be the same; rows are compared cell by cell. Cells that are numbers on both sides
  match within `tolerance` (absolute, default 0, so `20` and `20.0` match). `sort_rows = true` sorts both sides
  first, for outputs whose row order means nothing. A failure shows the row count difference, then up to 20
  differences as `row N, column C: expected X, got Y`.
- Works with `matrix`: `{key}` may appear in `output` and `expected`, e.g.
  `golden = { output = "out/{schema}.csv", expected = ".countercheck/golden/{schema}.csv", mode = "csv" }`.
- The receipt's reason reads `failed (exit 0; its output did not pass)`: the command ran fine, its output didn't.
