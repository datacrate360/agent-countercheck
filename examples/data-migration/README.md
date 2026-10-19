# Example: a data migration

A small, **made-up** migration that uses every check type. Two tenants (`alpha`, `beta`) have legacy CSV data in
`source/`. A toy converter (`scripts/convert.py`) turns it into the new format in `target/`, and the SQL views were
hand-converted into `sql/`. Nothing here is real data.

| File | What it is |
|---|---|
| `source/*.csv` | legacy data: `id,name,amount_cents,status_code` (protected: the worker may not change it) |
| `scripts/convert.py` | converts one tenant to `id,name,amount,status`; has **one deliberate bug** |
| `scripts/reconcile.py` | converts a tenant, then checks the row count and the total amount match |
| `sql/*.sql` | the converted views |
| `.countercheck/checks.toml` | the checks (below) |
| `.countercheck/golden/*.csv` | the locked reference output per tenant |
| `.countercheck/inventory/views.txt` | every view the old system had |

## The checks

- **Matrix**: `reconcile` runs once per tenant: `reconcile [tenant=alpha]`, `reconcile [tenant=beta]`.
- **Golden + budget**: `converted file` compares `target/<tenant>.csv` with `.countercheck/golden/<tenant>.csv` as CSV
  (rows sorted, amounts within 0.005), and needs a `METRIC rows=` of at least 1.
- **Forbid**: no `NVL(` or `DECODE(` left in `sql/`.
- **Coverage**: every view in `views.txt` is created in some `sql/` file.

## Run it

`countercheck verify` needs the checks at the root of a git repository, so the script copies the example into a fresh
temporary repository, commits it as the base, and verifies:

```sh
sh examples/data-migration/run-example.sh                              # countercheck on your PATH
sh examples/data-migration/run-example.sh target/debug/countercheck   # or a built binary
```

## What fails, and why

The verdict is **FAILED**, with four failing results:

1. `reconcile [tenant=alpha]`: the converter uses integer division (`cents // 100`), so alpha's amounts lose their
   cents and its total drops from 726324 to 726100 cents. `reconcile [tenant=beta]` **passes**: every beta amount is
   whole dollars, so the bug doesn't show there. That's why one check per target helps.
2. `converted file [tenant=alpha]`: the same bug, seen against the golden file:
   `row 1, column amount: expected 1250.50, got 1250.00` and two more. The rows come out sorted by name while the
   golden file is sorted by id; `sort_rows = true` makes that difference not matter.
3. `forbid: no old-dialect functions`: `sql/tenant_totals.sql:2` still uses `NVL(`.
4. `coverage: every view converted`: `monthly_revenue` is in the inventory but no file creates it.

To see it pass, in the temporary repository: change `cents // 100` to `cents / 100` in `scripts/convert.py`, replace
`NVL(` with `COALESCE(` in `sql/tenant_totals.sql`, add a `CREATE OR REPLACE VIEW monthly_revenue ...` file under
`sql/`, and run `countercheck verify` again: **VERIFIED**. Editing the golden files, the inventory or `source/`
instead doesn't help: Countercheck reads them from the base revision.
