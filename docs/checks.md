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

Also: [Which files the checks cover](#which-files-the-checks-cover) lists changed files no check looks at,
[Watched files](#watched-files-changes-allowed-always-listed) lists every change to files you want to review, and
[Lock the test runner's settings](#lock-the-test-runners-settings) closes a gap every test command has.

[examples/data-migration](../examples/data-migration) uses all of them on a small made-up migration. For a real
Oracle → Snowflake setup with Cortex Code, see the recipe
[recipes/oracle-to-snowflake](../recipes/oracle-to-snowflake/).

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
- The hint only appears when the output looks like a connection error (can't resolve, unreachable).
- **Writes outside the check folder** (and the temp dir) are refused too. When the output names the refused path
  (`Operation not permitted: '/var/tmp/x'`, `EPERM … open '/var/tmp/x'`), the hint says the check tried to write
  there, not that it lacked the internet. A bare "Operation not permitted" with no path is what both a refused
  connection to an IP address and a refused write print, so then the hint names both. Hints never change a verdict.
- On Linux and Windows there is no sandbox yet, so checks always have the internet there, and the hint never shows.

### Lock the test runner's settings

`protected = ["tests/**"]` puts back edited tests, but test runners also read settings from files **outside**
`tests/`. An agent can switch a failing test off there, and the checks pass with `tests/` untouched. With pytest,
each of these turned a real bug's FAILED into VERIFIED (tested, [examples/python-pytest](../examples/python-pytest/)):

| The agent adds or edits | Effect |
|---|---|
| `conftest.py` in the root, or in **any** folder pytest scans (`src/conftest.py`) | marks every test as skipped |
| `pyproject.toml`: `addopts = "--deselect tests/test_x.py::test_y"` | the failing test never runs |
| `pytest.ini` / `tox.ini` / `setup.cfg`: `addopts = -k "not y"` | the same |

Fix it in two parts: protect the files that only configure tests, and pin the runner's settings in the locked
command, so files you can't protect are ignored.

```toml
protected = ["tests/**", "**/conftest.py", "pytest.ini", "tox.ini", "setup.cfg"]

[[check]]
name = "tests"
run = "pytest -q -c tests/pytest.ini --rootdir=. --confcutdir=tests -o addopts= tests"
```

- `-c tests/pytest.ini`: settings come from a protected file (put `pythonpath = .` and your other settings there);
  `pyproject.toml` and any new `pytest.ini` are ignored.
- `--rootdir=.` keeps paths relative to the project. `--confcutdir=tests` loads only `conftest.py` files under
  `tests/`. `-o addopts=` drops extra options from anywhere. `tests` at the end runs that folder, whatever
  `testpaths` says.
- **Don't protect `pyproject.toml` or `package.json`.** They also list dependencies; putting them back would undo an
  honest new dependency and fail the checks for no reason.

**Other runners have the same gap.** Find where the runner reads its settings and either protect that file (if it only
configures tests) or name the settings in the command:

| Runner | Settings outside the tests | In the command |
|---|---|---|
| jest / vitest | `jest.config.*`, `vitest.config.*`, `package.json` (`"jest"`) | `npx jest --config tests/jest.config.js` |
| go test | build tags, `TestMain` in any test file | protect `**/*_test.go` |
| cargo test | `#[ignore]` in source files, `.cargo/config.toml` | protect `.cargo/**`; `cargo test -- --include-ignored` |

A [budget](#budget) on the number of tests that ran (the runner's summary, printed by a protected script as
`METRIC tests=<n>`) catches skipped tests whatever the trick.

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
scope = "added"                                    # optional: only lines added since the base (default "all")
```

- On the receipt it is a check named `forbid: <name>` (`forbid (added lines): <name>` with `scope = "added"`).
- The pattern is matched line by line ([regex syntax](https://docs.rs/regex/latest/regex/#syntax); it always runs
  in linear time). Use single quotes in TOML so backslashes stay as written.
- A failure lists up to 20 hits as `path:line: text` (lines cut to 200 characters) and the total count, then the
  `message`.
- Binary files and files over 5 MiB are skipped and listed as skipped.
- When no file matches `paths`, the check passes and says "nothing to scan".
- An invalid regex is refused when the check file is read.

### Only new lines: `scope = "added"`

Real code often already has the lines you want to forbid in *new* code: a legitimate `pytest.skip(...)` for a missing
database, an error message printed to stderr. A whole-file rule fails on those, so it gets weakened until it no
longer catches the agent's new skip. With `scope = "added"`, a line counts only if it's new compared with the base:

```toml
[[forbid]]
name = "no newly skipped tests"
pattern = 'pytest\.mark\.(skip|xfail)\b|pytest\.skip\('
paths = ["tests/**"]
scope = "added"
```

```text
  [FAIL] forbid (added lines): no newly skipped tests
         | 1 new (added since the base) forbidden line(s) in 1 file(s); 12 file(s) scanned
         | tests/test_pricing.py:41:     pytest.skip("flaky")
```

- Lines are compared **by text with the same file at the base revision**: a matching line counts when the worker's
  file has more copies of that exact line than the base did. Moving a line doesn't count; copying an existing
  forbidden line does. A changed line is a new line. New files count in full.
- The base is read from git's objects (checked against their ids), and protected files are put back before the scan,
  so editing a protected file can't make a line look old.
- Without `scope` (or `scope = "all"`) every line counts, as before.

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

## Which files the checks cover

A VERIFIED means every check passed. It doesn't say whether the checks **looked at the files the agent changed**. Tell
Countercheck which files each check covers, and the receipt lists every changed file that none of them covers:

```toml
uncovered = "note"                  # optional: "note" (default) or "unverified"

[[check]]
name = "backend tests"
run = "cd backend && pytest -q"
covers = ["backend/**"]             # globs, like `protected`

[[check]]
name = "frontend build"
run = "cd frontend && npm run build"
covers = ["frontend/src/**", "frontend/package.json"]
```

```text
  uncovered: 2 changed file(s) no check covers: docs/setup.md, frontend/functions/_middleware.js
  - 2 changed file(s) no check covers (listed, not checked): docs/setup.md, frontend/functions/_middleware.js
```

- It's **off** until at least one `[[check]]` has `covers`; then every changed file is compared.
- `[[forbid]]` and `[[coverage]]` cover their `paths`.
- **Not counted as uncovered:** protected files (they're put back and listed under "protected"), everything under
  `.countercheck/`, and build junk (`__pycache__`, `.DS_Store`, `*.pyc`). **Deleted files count:** deleting is a change.
- `uncovered = "unverified"` turns a VERIFIED with uncovered changes into UNVERIFIED (exit 3). It never touches a
  FAILED. It needs at least one `covers`; turn it on once your `covers` are complete.
- `covers` and `uncovered` come from the check file at the base revision, like every rule, so a worker can't widen them
  to hide a change.
- "Covered" means a check claims those files, not that its tests run every line of them.
- Agent hooks push back only on FAILED. Uncovered files are for the person reading the receipt: the agent can't fix them
  by itself, because only a new commit to `checks.toml` changes `covers`.
- The receipt keeps up to 100 paths plus the total; `show`, `report` and `ui` list them.

## Watched files: changes allowed, always listed

`protected` puts a file back, so honest work that has to update a test double fails. Leaving the file unprotected lets
the agent change an assertion or a fake without the receipt saying a word. `watched` sits in between: the change
stays, and the receipt lists it.

```toml
protected = ["**/conftest.py", ".github/workflows/**"]
watched = ["tests/**"]              # changes allowed, always listed
watched_mode = "note"               # optional: "note" (default) or "unverified"
```

```text
  watched:  2 file(s) changed (1 existing modified or deleted); review them:
            M tests/test_bootstrap.py (+73 -0)
            A tests/test_happy_eyeballs.py (+40 -0)
  - 2 watched file(s) changed (review them): M tests/test_bootstrap.py (+73 -0), A tests/test_happy_eyeballs.py (+40 -0)
```

- `A` added, `M` modified, `D` deleted. Line counts compare the file with the base by line text (a moved line
  counts as neither); binary files and files over 5 MiB get no counts.
- **Adding a test is normal; modifying or deleting an existing one is the signal.** `watched_mode = "unverified"`
  turns a VERIFIED into UNVERIFIED (exit 3) when an existing watched file was modified or deleted. New files never do,
  and a FAILED stays FAILED.
- A path that is both protected and watched is protected (put back, listed under "protected").
- `watched` and `watched_mode` come from the check file at the base revision, so a worker can't take its own edits off
  the list. Not listed: `.countercheck/**` and build junk.
- Agent hooks push back only on FAILED: watched changes are for the person reading the receipt.
- The receipt keeps up to 100 changes plus the totals; `show`, `report` and `ui` list them.
