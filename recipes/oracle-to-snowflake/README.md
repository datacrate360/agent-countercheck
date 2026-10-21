# Recipe: check an Oracle → Snowflake conversion done by Cortex Code

You are converting Oracle PL/SQL (procedures, functions, views) to Snowflake with an AI agent such as Snowflake's
Cortex Code. The agent says "done". This guide sets up Countercheck so that claim is checked before you believe it.

> **In plain words.** Before the agent starts, you lock a few rules in git. When the agent says "done", Countercheck
> runs them on its work. If something is missing or wrong, the agent gets the exact failure back and keeps working.
> Nothing it edits can change the locked rules.

| | Time | Checks | Catches |
|---|---|---|---|
| **[Part 1: 5-minute start](#part-1-5-minute-start)** | 5 min | every object converted · no PL/SQL-only syntax | skipped procedures/views, leftover `ROWNUM`, `DBMS_*`, `%TYPE`… |
| **[Part 2: compare with Oracle](#part-2-compare-the-results-with-oracle-later)** (later) | ~20 min | + matches Oracle, per query | wrong numbers: truncated amounts, lost rows, bad joins |

Files in this recipe (download what a step needs; every step says how):

| File | What it is |
|---|---|
| [`checks.toml`](checks.toml) | Part 1 checks: every object converted, no PL/SQL-only syntax |
| [`oracle-inventory.py`](oracle-inventory.py) | builds the object list from Oracle DDL already extracted to files |
| [`compare-with-oracle.toml`](compare-with-oracle.toml) | Part 2 check: each locked query on Snowflake must match Oracle's answer |
| [`sf_query.py`](sf_query.py) | Part 2 helper: runs one locked query on Snowflake, saves the result as CSV |
| [`test/run-test.sh`](test/run-test.sh) | proves the recipe works (offline, with a fake Snowflake); runs in CI |

Start with Part 1. It needs no Snowflake or Oracle connection, so the macOS sandbox stays on. Add Part 2 when you
want the numbers checked too. Countercheck runs only on your machine and has no telemetry.

## Part 1: 5-minute start

You need Git and Python 3 (on Windows also [Git for Windows](https://git-scm.com/download/win)), and your company's
OK to install an open-source tool (MIT licence) if it asks for one.

**1. Install** (in your repo):

```sh
pip install agent-countercheck
mkdir -p .countercheck/inventory
R=https://raw.githubusercontent.com/datacrate360/agent-countercheck/PROD/recipes/oracle-to-snowflake
```

`R` is just a shortcut for the recipe's address in the next steps (PowerShell: `$R = "https://…"`, same address).

Not a git repository yet? Run `git init` in the project folder first (it stays on your machine).

**2. List what must be converted**, one object per line, in `.countercheck/inventory/objects.txt`.

*Already have your Oracle DDL extracted to files* (one folder per schema with `procedures.sql`, `functions.sql`,
`views_ddl.sql`, `package_bodies.sql`)? Let the inventory script read them; no Oracle connection needed:

```sh
curl -fsSLO $R/oracle-inventory.py
python3 oracle-inventory.py Oracle/SALES --package ORDER_API
```

- It lists standalone procedures, functions and views, plus every routine inside each package body (overloads once).
- **Start with one package** (`--package`): a few routines make a short first session. Drop `--package` for the
  whole schema.
- Snowflake has no packages, so a package routine is listed as `ORDER_API_CREATE_ORDER` by default. If your team
  names them differently, say so: `--package-format "{package}.{routine}"` gives `ORDER_API.CREATE_ORDER`. The names
  must match how they're created in Snowflake, or coverage reports them all as missing.

*Otherwise*, run this on Oracle and save the names:

```sql
SELECT object_name FROM user_objects
WHERE object_type IN ('PROCEDURE', 'FUNCTION', 'VIEW')
ORDER BY object_name;
```

**3. Get the checks:** `curl -fsSL $R/checks.toml -o .countercheck/checks.toml`. It assumes the Oracle code is in `oracle/` and the agent writes
Snowflake code to `snowflake/`; change the two folder names if yours differ (folder names are case-sensitive in the
patterns, e.g. `"Oracle/**"` and `"Snowflake/**/*.sql"`). What's inside:

```toml
# Oracle PL/SQL -> Snowflake: checks locked before the agent starts.
protected = ["oracle/**"]   # the original Oracle code is the reference; the agent may not change it

# Nothing skipped: every object in the inventory is created somewhere in the Snowflake code.
[[coverage]]
name = "every object converted"
items = ".countercheck/inventory/objects.txt"
paths = ["snowflake/**/*.sql"]
pattern = 'CREATE\s+(OR\s+REPLACE\s+)?(SECURE\s+)?(PROCEDURE|FUNCTION|VIEW|TABLE)\s+(\w+\.)*{item}\b'
case_insensitive = true

# No PL/SQL-only constructs left in the Snowflake code (a starting list: add your team's rules).
[[forbid]]
name = "no PL/SQL-only syntax"
pattern = '\bROWNUM\b|\bDBMS_\w+|\bUTL_\w+|\bBULK\s+COLLECT\b|\bFORALL\b|%ROWTYPE\b|%TYPE\b|\bPRAGMA\b'
paths = ["snowflake/**/*.sql"]
message = "Rewrite these for Snowflake."
case_insensitive = true
```

**4. Commit and try it:**

```sh
echo ".countercheck/receipts/" >> .gitignore
git add -A
git commit -m "Lock the conversion checks"
countercheck verify --claim "nothing converted yet"
```

Before any conversion it must say **FAILED** and list every object as `missing`. That proves the check bites.

**5. Connect Cortex Code.** Cortex Code reads Claude Code's hook format. Save this as `.cortex/settings.json` in the
repo (Cortex Code also reads `.claude/settings.json`):

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

Start each session like this, so the checks stay locked to where you began even if the agent commits:

```sh
export COUNTERCHECK_BASE=$(git rev-parse HEAD)
cortex
```

On Windows (PowerShell): `$env:COUNTERCHECK_BASE = (git rev-parse HEAD)`, then `cortex`.

Then ask for the work as usual, e.g. *"Convert everything in oracle/ to Snowflake in snowflake/, one file per object."*
When Cortex Code says "done", the hook runs the checks. If they fail, the agent can't stop yet: it gets the failing
output and keeps working (at most 3 times per session). If it honestly says it is blocked ("this can't pass,
because …"), it may stop, and the receipt still says FAILED. See every run with `countercheck report --open`.

That's it. **Part 1 checks the code, not the numbers**: a conversion can be complete and free of Oracle-only syntax
but still compute the wrong totals. Part 2 checks those.

## Part 2: compare the results with Oracle (later)

For each thing worth comparing, you write the **same question twice**: once for Oracle (you run it now and save the
answer) and once for Snowflake (Countercheck runs it at every "done" and compares).

```text
.countercheck/
├── bin/sf_query.py           runs one locked query on Snowflake, saves the result as CSV
├── targets/queries.txt       the names of the queries to compare
├── queries/<name>.sql        the Snowflake side of each comparison
└── golden/<name>.csv         Oracle's answer to the same question
```

**1. Install the Snowflake driver.** It uses the connection in `~/.snowflake/connections.toml` (the file Cortex Code
and the Snowflake CLI already use):

```sh
pip install snowflake-connector-python
mkdir -p .countercheck/bin .countercheck/targets .countercheck/queries .countercheck/golden
echo "out/" >> .gitignore
```

**2. Write one query pair.** `.countercheck/queries/order_totals.sql` (Snowflake; it may `CALL` the converted procedure
first, the last statement must return rows):

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

Add the name to `.countercheck/targets/queries.txt` (one per line). Tips:
- **Same column aliases on both sides, upper case** (`AS TOTAL`): the header must match exactly.
- **Compare totals, not every row**: counts and sums per group catch most conversion bugs and stay fast.
- Numbers within 0.005 count as equal and row order doesn't matter. Format dates the same way on both sides
  (e.g. `TO_CHAR(d, 'YYYY-MM-DD')`).

**3. Get the query script.** It runs one locked query on Snowflake and saves the result as `out/<name>.csv`:

```sh
curl -fsSL $R/sf_query.py -o .countercheck/bin/sf_query.py
```

Point it at a **development schema**, never production (`SNOWFLAKE_CONNECTION=<name>` picks a connection).

**4. Add the comparison** to the end of the check file: `curl -fsSL $R/compare-with-oracle.toml >> .countercheck/checks.toml`.
What it adds:

```toml
# Same answers as Oracle: each locked query, run on Snowflake, must match the result exported from Oracle.
[[check]]
name = "matches Oracle"
run = "python3 .countercheck/bin/sf_query.py {query}"
matrix_file = { query = ".countercheck/targets/queries.txt" }
golden = { output = "out/{query}.csv", expected = ".countercheck/golden/{query}.csv", mode = "csv", tolerance = 0.005, sort_rows = true }
budget = { metric = "rows", min = 1 }
network = true
timeout_secs = 300
```

**5. Commit, and turn the sandbox off for these checks.** On macOS, checks normally run in a sandbox with no internet
and no writes outside the project. `network = true` lets this check reach Snowflake, but the Snowflake driver may also
write caches in your home folder, which the sandbox blocks ("permission denied" or "read-only" errors). So with Part 2:

```sh
git add -A
git commit -m "Compare results with Oracle"
countercheck verify --sandbox none --claim "trying the Oracle comparison"
```

Before starting Cortex Code, also `export COUNTERCHECK_SANDBOX=none` (PowerShell: `$env:COUNTERCHECK_SANDBOX =
"none"`). The receipt then says `sandbox: none`. On Linux and Windows there is no sandbox yet, so this changes nothing.

## What the agent gets back

A conversion that cut decimals, kept a `ROWNUM` and skipped a view (from a test run of both parts):

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

## Limits: what this setup can't see

- **Part 1 checks the code, not the results.** Wrong numbers pass until you add Part 2.
- **The Snowflake checks query what is deployed**, not the files. If the agent deploys something different from what
  is in `snowflake/`, or forgets to deploy, the numbers come from whatever is in the dev schema. Ask the agent to
  deploy from the files, and keep the schema just for this work.
- **Only what your queries ask.** A bug in a column no query looks at passes.
- **`forbid` also reads comments**: a comment that mentions `ROWNUM` fails the check. Reword the comment.
- **The forbid list is a starting point.** Snowflake accepts many Oracle-style functions (`NVL`, `DECODE`); add a rule
  only for what your team wants gone.
- **No sandbox with `--sandbox none`** (Part 2): the checks run as a normal process with your permissions. They are
  your checks, in your repo, so that is the same trust as running them yourself.
- **Cortex Code's hooks are taken from Snowflake's documentation**; this setup hasn't yet been run in a real Cortex
  Code session, and Snowflake's CLI docs list the Stop hook as unable to block, so the push-back may not reach the
  agent (the receipt is written either way; [details](../../docs/hooks/claude-code.md#cortex-code)). If the hook doesn't fire, run `countercheck verify` yourself when the agent says done, and please
  [open an issue](https://github.com/datacrate360/agent-countercheck/issues) with what you saw.
