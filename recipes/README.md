# Recipes

A recipe is a ready starting point for one real job: a `checks.toml`, the small helper scripts that job needs, a
step-by-step README, and a test that proves it works. Copy it into your project, change the names, run it.

| Recipe | For | Check types | Start |
|---|---|---|---|
| [oracle-to-snowflake](oracle-to-snowflake/) | converting Oracle PL/SQL (procedures, functions, views, packages) to Snowflake with an AI agent such as Cortex Code | coverage, forbid, golden per query (matrix) | 5 minutes |

## Core, recipes and your project

| Layer | What it is | Knows your domain? |
|---|---|---|
| **Core** (the `countercheck` binary) | locks the checks, makes a clean copy, puts protected files back, runs the checks, seals a receipt; six check types ([docs/checks.md](../docs/checks.md)) | **no**, on purpose |
| **Recipes** (this folder) | the checks and helpers for one kind of work | yes |
| **Your project** | your object lists, queries, expected answers, folder names | fully |

Domain knowledge lives in recipes, never in the core. When several recipes need the same helper, that's the signal to
turn it into a core feature (for example, coverage lists built from source files; see [ROADMAP](../docs/ROADMAP.md)).

## Recipes vs examples

- [examples/](../examples) are tiny made-up projects that **teach** the check types: everything is included, run one
  script, see FAILED then VERIFIED.
- Recipes are for **your real project**: they bring the checks and helpers, you bring the data.

## Add a recipe

1. `recipes/<job>/` with a `README.md` (what it catches, a 5-minute start, limits), the check file(s), helper scripts
   (Python standard library or the domain's own tool), and `test/run-test.sh`.
2. The test runs offline on made-up data: nothing done (FAILED), a realistic bad result (FAILED, with the exact
   failure lines), a cheat (FAILED), the fix (VERIFIED). Fake any outside service, as
   [oracle-to-snowflake/test/fake](oracle-to-snowflake/test/fake/) fakes Snowflake.
3. If the README shows a check file inline, the test must check it matches the file.
4. Add a CI step next to the existing one in `.github/workflows/ci.yml`, and a row to the table above.

Never include real customer data, names or code: recipes are public.
