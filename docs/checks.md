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
