# Guide: check an Oracle → Snowflake conversion done by Cortex Code

You are converting Oracle PL/SQL (procedures, functions, views) to Snowflake with an AI agent such as Snowflake's
Cortex Code. The agent says "done, results match Oracle". This guide sets up Countercheck so that claim is checked
before you believe it.

> **In plain words.** Before the agent starts, you lock three things in git: the list of objects to convert, the
> answers Oracle gives today, and the rules for what Snowflake code may contain. When the agent says "done",
> Countercheck runs those checks on its work. If anything is missing, wrong or still Oracle-only, the agent gets the
> exact failure back and keeps working. Nothing it edits can change the locked answers.

| Check | Catches | Needs Snowflake |
|---|---|---|
| **every object converted** (coverage) | a procedure, function or view the agent skipped | no |
| **no PL/SQL-only syntax** (forbid) | `ROWNUM`, `DBMS_*`, `BULK COLLECT`, `%TYPE`… left in the Snowflake code | no |
| **matches Oracle** (golden, one result per query) | different numbers: truncated amounts, lost rows, wrong joins | yes |

Time: about 30 minutes for the first package. Nothing leaves your machine except the queries your own checks send to
Snowflake; Countercheck has no telemetry.

## 1. Before you start

- **Git**, **Python 3**, and a Snowflake connection in `~/.snowflake/connections.toml` (the file Cortex Code and the
  Snowflake CLI already use).
- On Windows: [Git for Windows](https://git-scm.com/download/win) (checks run with its `sh`).
- Your company's OK to install an open-source tool (MIT licence) on a work machine, if it asks for one.

```sh
pip install countercheck snowflake-connector-python
countercheck --version
```

## 2. Lay out the project

```text
your-repo/
├── oracle/                       the original PL/SQL (reference; the agent may read it, not change it)
├── snowflake/                    the converted code the agent writes, one file per object
└── .countercheck/                locked: Countercheck reads all of it from the commit the work started from
    ├── checks.toml
    ├── bin/sf_query.py           runs one locked query on Snowflake, saves the result as CSV
    ├── inventory/objects.txt     every object that must be converted
    ├── targets/queries.txt       the queries to compare
    ├── queries/<name>.sql        the Snowflake side of each comparison
    └── golden/<name>.csv         Oracle's answer to the same question
```

The agent can't change anything under `.countercheck/` or `oracle/`: if it does, Countercheck puts the original back
before running the checks and lists the change on the receipt.

## 3. List what must be converted

Run this on Oracle (as the schema owner) and paste the names into `.countercheck/inventory/objects.txt`, one per line:

```sql
SELECT object_name FROM user_objects
WHERE object_type IN ('PROCEDURE', 'FUNCTION', 'VIEW')
ORDER BY object_name;
```

```text
# .countercheck/inventory/objects.txt: one object per line
CALC_ORDER_TOTALS
V_MONTHLY_REVENUE
```

## 4. Lock Oracle's answers

For each thing worth comparing, write the **same question twice**: once for Oracle (you run it now) and once for
Snowflake (Countercheck runs it at every "done").

`.countercheck/queries/order_totals.sql` (Snowflake; it may `CALL` the converted procedure first, the last statement
must return rows):

```sql
CALL CALC_ORDER_TOTALS();
SELECT REGION, COUNT(*) AS ORDERS, SUM(TOTAL) AS TOTAL FROM ORDER_TOTALS GROUP BY REGION;
```

Run the Oracle version (`EXEC calc_order_totals;` then the same `SELECT`) and export the result as CSV with a header
row to `.countercheck/golden/order_totals.csv`:

```text
REGION,ORDERS,TOTAL
EAST,2,150.50
WEST,1,99.99
```

Then list the query names in `.countercheck/targets/queries.txt`:

```text
order_totals
monthly_revenue
```

Tips:
- **Same column aliases on both sides, upper case** (`AS TOTAL`): the header must match exactly.
- **Compare totals, not every row**: counts and sums per group catch most conversion bugs and stay fast. Add row-level
  queries for the tables that matter most.
- Numbers within 0.005 count as equal (`tolerance` below) and row order doesn't matter (`sort_rows`). Format dates
  the same way on both sides (e.g. `TO_CHAR(d, 'YYYY-MM-DD')`).

## 5. Add the check file and the query script

`.countercheck/checks.toml`:

```toml
# Oracle PL/SQL -> Snowflake: checks locked before the agent starts.
# Countercheck reads this file (and everything under .countercheck/) from the commit the work started from.
protected = ["oracle/**"]   # the original Oracle code is the reference; the agent may not change it

# 1. Nothing skipped: every object in the inventory is created somewhere in the Snowflake code.
[[coverage]]
name = "every object converted"
items = ".countercheck/inventory/objects.txt"
paths = ["snowflake/**/*.sql"]
pattern = 'CREATE\s+(OR\s+REPLACE\s+)?(SECURE\s+)?(PROCEDURE|FUNCTION|VIEW|TABLE)\s+(\w+\.)*{item}\b'
case_insensitive = true

# 2. No PL/SQL-only constructs left in the Snowflake code (a starting list: add your team's rules).
[[forbid]]
name = "no PL/SQL-only syntax"
pattern = '\bROWNUM\b|\bDBMS_\w+|\bUTL_\w+|\bBULK\s+COLLECT\b|\bFORALL\b|%ROWTYPE\b|%TYPE\b|\bPRAGMA\b'
paths = ["snowflake/**/*.sql"]
message = "Rewrite these for Snowflake."
case_insensitive = true

# 3. Same answers as Oracle: each locked query, run on Snowflake, must match the result exported from Oracle.
[[check]]
name = "matches Oracle"
run = "python3 .countercheck/bin/sf_query.py {query}"
matrix_file = { query = ".countercheck/targets/queries.txt" }
golden = { output = "out/{query}.csv", expected = ".countercheck/golden/{query}.csv", mode = "csv", tolerance = 0.005, sort_rows = true }
budget = { metric = "rows", min = 1 }
network = true
timeout_secs = 300
```

`.countercheck/bin/sf_query.py`:

```python
"""Run one locked query on Snowflake and save the result as CSV, for Countercheck to compare with Oracle's.

Usage: python3 .countercheck/bin/sf_query.py <name>
Reads .countercheck/queries/<name>.sql (it may CALL a procedure first; the last statement must return rows) and
writes out/<name>.csv. Connects with your Snowflake connections.toml: the connection named in SNOWFLAKE_CONNECTION,
else your default connection.
"""
import csv
import os
import sys
from pathlib import Path

import snowflake.connector

name = sys.argv[1]
sql = Path(".countercheck/queries", f"{name}.sql").read_text()
conn_name = os.environ.get("SNOWFLAKE_CONNECTION")
conn = snowflake.connector.connect(**({"connection_name": conn_name} if conn_name else {}))
try:
    cursors = conn.execute_string(sql)
    last = cursors[-1] if cursors else None
    if last is None or last.description is None:
        sys.exit(f"{name}.sql: the last statement must return rows (a SELECT)")
    rows = last.fetchall()
    Path("out").mkdir(exist_ok=True)
    with open(Path("out", f"{name}.csv"), "w", newline="") as f:
        out = csv.writer(f)
        out.writerow([col[0].upper() for col in last.description])
        out.writerows(rows)
    print(f"{name}: {len(rows)} row(s) from Snowflake")
    print(f"METRIC rows={len(rows)}")
finally:
    conn.close()
```

Point it at a **development schema**, never production: set the database and schema in the connection you use
(`SNOWFLAKE_CONNECTION=<name>` picks one from `connections.toml`).

## 6. Commit, then run it once by hand

```sh
echo "out/" >> .gitignore
echo ".countercheck/receipts/" >> .gitignore
git add -A
git commit -m "Lock the conversion checks"
countercheck verify --sandbox none --claim "nothing converted yet"
```

Before any conversion this must say **FAILED**: `coverage` lists every object as missing and each `matches Oracle`
query fails (`forbid` passes: there is nothing to scan yet). That proves the checks bite. If `coverage` or a query
passes with nothing converted, it isn't testing anything yet; fix it now.

**Why `--sandbox none`:** on macOS, checks normally run in a sandbox with no internet and no writes outside the
project. `network = true` already lets the Snowflake check reach Snowflake, but the Snowflake driver may also write
caches in your home folder, which the sandbox blocks (you'd see "permission denied" or "read-only" errors). Running
without the sandbox avoids that; the receipt says `sandbox: none`. On Linux and Windows there is no sandbox yet, so
the flag changes nothing there.

## 7. Connect Cortex Code

Cortex Code reads Claude Code's hook format. Add this to `.cortex/settings.json` in the repo (Cortex Code also reads
`.claude/settings.json`):

```json
{
  "hooks": {
    "SessionStart": [
      { "hooks": [ { "type": "command", "command": "countercheck hook session-start", "timeout": 10 } ] }
    ],
    "Stop": [
      { "hooks": [ { "type": "command", "command": "countercheck hook claude-stop", "timeout": 900 } ] }
    ]
  }
}
```

Start each session from a terminal like this (macOS / Linux):

```sh
export COUNTERCHECK_SANDBOX=none
export COUNTERCHECK_BASE=$(git rev-parse HEAD)
cortex
```

Windows (PowerShell):

```powershell
$env:COUNTERCHECK_SANDBOX = "none"
$env:COUNTERCHECK_BASE = (git rev-parse HEAD)
cortex
```

- `COUNTERCHECK_BASE` locks the checks to the commit you started from, even if the agent commits its work. The
  `SessionStart` hook does the same job; setting both is safe, and the variable covers you if Cortex Code doesn't
  run that hook.
- The `Stop` timeout must cover all your queries; 900 seconds leaves room for several. Cortex Code's default (60 s)
  is too short.
- Then ask for the work as usual, e.g. *"Convert CALC_ORDER_TOTALS and V_MONTHLY_REVENUE from oracle/ to Snowflake
  in snowflake/, one file per object, and deploy them to the dev schema."*

When Cortex Code says "done", the hook runs the checks. If they fail, it doesn't let the agent stop: it sends back the
failing output, and the agent keeps working (at most 3 times per session). When the agent honestly says it is
blocked ("this can't pass, because …"), it may stop, and the receipt still says FAILED.

## What the agent gets back

A conversion that cut decimals, kept a `ROWNUM` and skipped a view (from a test run of this setup):

```text
countercheck receipt d05203950c9c447f  FAILED
  claim:    "Done, everything matches Oracle." -> CONTRADICTED by evidence
  [FAIL] matches Oracle [query=order_totals]  $ python3 .countercheck/bin/sf_query.py order_totals
         | golden: out/order_totals.csv does not match .countercheck/golden/order_totals.csv
         | 2 cell(s) differ (rows sorted before comparing)
         | row 1, column TOTAL: expected 150.50, got 150
         | row 2, column TOTAL: expected 99.99, got 99
  [pass] matches Oracle [query=monthly_revenue]
  [FAIL] forbid: no PL/SQL-only syntax
         | snowflake/calc_order_totals.sql:4: -- uses ROWNUM to page
         | Rewrite these for Snowflake.
  [FAIL] coverage: every object converted
         | 1 of 2 item(s) missing; 1 file(s) scanned
         | missing: V_MONTHLY_REVENUE
```

When the agent then edited `.countercheck/golden/order_totals.csv` to match its own numbers, Countercheck put the
Oracle answer back, ran the checks again, and the receipt said `protected Modified: … -> restored_from_base`.

See every run in the browser: `countercheck report --open` (a page of all receipts) or `countercheck ui` (live).

## Limits: what this setup can't see

- **The Snowflake checks query what is deployed**, not the files. If the agent deploys something different from what
  is in `snowflake/`, or forgets to deploy, the numbers come from whatever is in the dev schema. Ask the agent to
  deploy from the files, and keep the schema just for this work.
- **Only what your queries ask.** A bug in a column no query looks at passes. Start with totals, then add queries
  for the columns that matter.
- **`forbid` also reads comments**: a comment that mentions `ROWNUM` fails the check. Reword the comment.
- **The forbid list is a starting point.** Snowflake accepts many Oracle-style functions (`NVL`, `DECODE`); add a rule
  only for what your team wants gone.
- **No sandbox with `--sandbox none`**: the checks run as a normal process with your permissions. They are your
  checks, in your repo, so that is the same trust as running them yourself.
- **Cortex Code's hooks are taken from Snowflake's documentation**; this setup hasn't yet been run in a real Cortex
  Code session. If the hook doesn't fire, run `countercheck verify --sandbox none` yourself when the agent says done,
  and please [open an issue](https://github.com/datacrate360/countercheck/issues) with what you saw.
