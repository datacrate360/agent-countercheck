# Check types

Checks live in `.countercheck/checks.toml`. Countercheck reads that file from the **base revision** (before the
worker's change), runs every check in a clean copy of the worker's files, and puts back any protected file the worker
changed. Everything under `.countercheck/` is always protected.

A check result is `pass` or `FAIL`. Any failed check makes the verdict `FAILED`. The verdict is `VERIFIED` only when
the checks came from the base revision and all of them passed.

**Pick the right base.** The base is the commit the work started from. `countercheck verify` defaults to `HEAD`, which
is right for uncommitted work. If the work was committed, pass where it started (`--base origin/main` in CI). Agent
hooks find it themselves with `countercheck hook session-start` (README "Agent hooks").

| Type | Use it when | Table |
|---|---|---|
| [Command](#command) | a command can tell right from wrong (tests, linters, builds) | `[[check]]` |
| [Matrix](#matrix) | the same command must pass for many targets (tenants, schemas, levels, files) | `[[check]]` + `matrix` |
| [Forbid](#forbid) | some text must not appear in the worker's files (leftover syntax, debug code, secrets) | `[[forbid]]` |
| [Golden](#golden) | a command's output file must match a reference you keep (exports, reports, renders) | `[[check]]` + `golden` |
| [Budget](#budget) | a number the command prints must stay within a limit (time, size, frame rate, error rate) | `[[check]]` + `budget` |
| [Coverage](#coverage) | every item in a list you keep must show up in the worker's files (tables, procedures, levels, pages) | `[[coverage]]` |

[examples/data-migration](../examples/data-migration) uses all of them on a small made-up migration.

## Command

Runs a command with `sh -c` in the clean copy. Exit code 0 is a pass.

```toml
protected = ["tests/**"]

[[check]]
name = "unit tests"
run = "pytest -q"
timeout_secs = 600          # optional, default 600, max 7200
shell = "sh"                # optional: "sh" (default), "pwsh" or "cmd"
```

### Which shell

| `shell` | Runs | Where |
|---|---|---|
| `"sh"` (default) | `sh -c <run>` | everywhere; on Windows, Git for Windows' `sh.exe` (found on `PATH`, else next to `git.exe`, else in `C:\Program Files\Git`) |
| `"pwsh"` | `pwsh -NoProfile -NonInteractive -Command <run>` | wherever PowerShell 7 is installed; on Windows it falls back to Windows PowerShell (`powershell.exe`) |
| `"cmd"` | `cmd /D /S /C "<run>"` (the command line exactly as written) | Windows only |

- Keep the default `sh` when the project is worked on from several OSes: the same check file then runs the same way
  on macOS, Linux and Windows.
- Any other value is refused when the check file is read.
- If the shell isn't there (no Git for Windows, no PowerShell, `cmd` off Windows), the check **fails** with a plain
  message saying what to install, e.g. "install Git for Windows, or set `shell = "pwsh"` on this check".
- On a timeout the whole process tree is killed (the process group on Unix; on Windows the check's Job Object, with
  `taskkill /T` as a fallback). On Windows, processes a check leaves running in the background are also stopped when
  it ends, so they can't keep countercheck's output (and an agent's hook) waiting.
- Files checked out with Windows line endings (CRLF) work with every check type: `$` in `forbid` and `coverage`
  patterns also matches before `\r\n`, golden text and csv compare lines whatever their ending, and `METRIC` lines
  may end in `\r\n`.

### Checks that need the internet

On macOS, checks run in a sandbox with no internet (this machine's localhost still works), so a check can't
quietly depend on, or leak to, an outside service. If a check really needs the internet (a live API, a package
registry, a real website), say so on that check:

```toml
[[check]]
name = "live api"
run = "pytest tests/test_live_api.py"
network = true
```

- Only that check gets the internet; writes outside the check folder stay blocked. The receipt marks it
  (`internet allowed`).
- It lives in the locked check file, so a worker can't grant itself the internet.
- When a check fails while it ran without the internet, Countercheck adds a hint saying so. The verdict stays
  FAILED: a failure that might be caused by the missing internet is still a failure until the check file says the
  check may use it.
- The hint only appears when the output looks like a connection error (blocked, can't resolve, unreachable).
- On Linux and Windows there is no sandbox yet, so checks always have the internet there, and the hint never shows.

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

## Budget

After the command passes, a number it printed must be within a limit.

```toml
[[check]]
name = "export speed"
run = "python3 scripts/bench.py"          # prints a line like  METRIC seconds=12.4
budget = { metric = "seconds", max = 30 }  # or min = ..., or both
```

- The command prints `METRIC <name>=<number>` on a line of its own; the last such line for the metric counts, wherever
  it is in the output.
- Limits are inclusive: `max = 30` passes 30 and fails 30.1.
- The output shows the value and the limit, e.g. `budget: seconds = 12.4 (max 30; within the limit)`. A missing
  metric fails with a plain message.
- Works with `matrix` (one value per target).
- **A budget alone can be faked:** an agent can simply print `METRIC accuracy=0.99`. Pair it with a `[[forbid]]` on
  hard-coded metric lines, or a `golden` on the real output.
- With `golden` on the same check, the order is: the command must exit 0, then the budget, then the golden file. The
  first failure is the one reported.

## Coverage

Every item in a list you keep must be accounted for in the worker's files. Countercheck scans the clean copy itself,
before any check command runs.

```toml
[[coverage]]
name = "every procedure converted"
items = ".countercheck/inventory/procedures.txt"   # one item per line, under .countercheck/
paths = ["snowflake/**/*.sql"]
pattern = 'CREATE\s+OR\s+REPLACE\s+PROCEDURE\s+{item}\b'   # {item} = the item, matched literally
case_insensitive = true
```

- For each item, at least one file matching `paths` must match the pattern with `{item}` replaced by the item.
  Regex characters in an item mean themselves (`a.b` doesn't match `aXb`).
- The pattern is matched against each whole file, so it may span lines; `^` and `$` mean the start and end of a
  line.
- The items file must be under `.countercheck/`, and is read from the base revision like the check file, so a worker
  can't shorten the list. Blank lines and lines starting with `#` are skipped; at most 10,000 items.
- On the receipt it is a check named `coverage: <name>`. A failure lists up to 50 missing items plus the total.
