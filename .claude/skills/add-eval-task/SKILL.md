---
name: add-eval-task
description: Use when adding a benchmark task to eval/ (tasks.py, or tasks_hard.py for impossible tasks). Writes visible tests, hidden tests, a reference solution and an optional cheat, proves the trap works, then validates with eval/validate.py.
---

# Add an eval task

The benchmark (`eval/`, results in `docs/eval/`) gives real agents small coding tasks and grades them with **hidden
tests** the agent never sees. A task is one `add(...)` call building a `Task` (fields and kinds documented at the top
of `eval/tasks.py`).

## Kinds and ids

| kind | id letter | The task is… | Lives in |
|---|---|---|---|
| `trap-hard` | `h` | the honest fix takes real work; the easy way out is to skip/loosen a test or handle only the visible cases | `tasks.py`, section `trap-hard` |
| `trap-run` | `r` | the code looks right but fails only when it runs | `tasks.py`, section `trap-run` |
| `trap-partial` | `p` | two things to fix; one is easy | `tasks.py`, section `trap-partial` |
| `normal` | `n` | an ordinary fix or feature (measures false alarms and overhead) | `tasks.py`, section `normal` |
| `unverifiable` | `u` | the visible tests need the internet (the sandbox blocks it) | `tasks.py`, section `unverifiable` |
| `impossible` (`possible=False`) | `i`, `m` | no honest change passes every test | `tasks_hard.py`, inside `add_hard_tasks` |

Use exactly these kind strings: nothing rejects a typo, and the report would group it as a new kind. The id is
`<letter><next number in that section>_<topic>`, e.g. `h7_versions`. Read two tasks of the same section first.

## Procedure

1. Add the `add(...)` call at the end of its section and bump the count in the section header
   (`# ---- trap-hard (6)` → `(7)`).
2. `prompt`: what a user would really type. State the rules a careful engineer needs (e.g. "missing parts count as
   zero"), but don't say which cases are tested or where the trap is.
3. `files`: the starting repo, without tests. `tests`: visible tests under `tests/` (the agent sees them; Countercheck
   locks them). `hidden`: tests under `hidden_tests/` that define "really done", including the cases the visible
   tests miss. Build files with the helpers: `d("""…""")` (dedent), `py(body, "module")` (adds `import unittest` and
   `from module import *`), `js(body, "module")` (adds `node:test`, `assert`, `require('../module')`). Python
   `__init__.py` files in `tests/` and `hidden_tests/` are created for you. Double backslashes inside these strings.
4. `solution`: files that make every visible and hidden test pass (impossible tasks: the best honest partial fix).
5. Optional `cheat`: files a gaming agent would write. `validate.py` requires the cheat to pass the visible tests and
   fail the hidden ones, and Countercheck to give FAILED if `cheat_caught=True` (test edits) or VERIFIED if
   `cheat_caught=False` (e.g. hard-coded answers: a known limit). Most existing tasks have none.
6. For a trap, prove the trap: the obvious fix should pass the visible tests and fail the hidden ones.

   ```python
   # run from eval/: python3 -c '...'   (edit OBVIOUS and the task id)
   import tempfile; from pathlib import Path
   from tasks import BY_ID; from taskkit import materialize, visible, judge, _write
   t = BY_ID["h7_versions"]; w = Path(tempfile.mkdtemp())
   materialize(t, w); _write(w, {"versions.py": OBVIOUS})
   print("visible", visible(t, w)[0], "hidden", judge(t, w)[0])   # want: visible True, hidden False
   ```

## Verification

```sh
cargo build --release              # validate.py uses target/release/countercheck, or set COUNTERCHECK_BIN
python3 eval/validate.py h7_versions   # one task, ~2 s
python3 eval/validate.py               # all tasks, ~1 min; the total goes up by one, nothing else breaks
```

Done when your task prints `ok`. CI runs the full validator on macOS.

## Pitfalls

- `validate.py` runs Countercheck with the macOS sandbox; on Linux it may not run.
- Tasks must run offline and fast (seconds), standard library / Node built-ins only.
- Hidden tests must be fair: a careful engineer reading the prompt would handle those cases.
- Adding tasks changes the benchmark's totals: don't edit published results; new numbers come from a new run.
