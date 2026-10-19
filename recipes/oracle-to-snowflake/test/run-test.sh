#!/bin/sh
# Proves the oracle-to-snowflake recipe works, offline: a made-up Oracle schema, a fake Snowflake (SQLite), the real
# recipe files. Builds the inventory, then: nothing converted (FAILED), a bad conversion (FAILED: skipped view,
# truncated totals, leftover ROWNUM), a cheat on the Oracle answers (FAILED, restored), the fix (VERIFIED).
# Exits 1 on any surprise. Usage: sh recipes/oracle-to-snowflake/test/run-test.sh [path/to/countercheck]
set -e
COUNTERCHECK=${1:-countercheck}
TEST=$(cd "$(dirname "$0")" && pwd)
RECIPE=$(dirname "$TEST")

# The recipe page must show the same checks as the files people download.
python3 - "$RECIPE" <<'EOF'
import re, sys
from pathlib import Path
recipe = Path(sys.argv[1])
blocks = [b for lang, b in re.findall(r"```(\w+)\n(.*?)```", (recipe / "README.md").read_text(), re.S) if lang == "toml"]
for block, file in zip(blocks, ["checks.toml", "compare-with-oracle.toml"]):
    if block != (recipe / file).read_text():
        sys.exit(f"run-test: README.md shows a different {file} than the file itself")
print("run-test: README.md matches checks.toml and compare-with-oracle.toml")
EOF

# A Python without any real Snowflake driver, so the fake one is used.
VENV=$(mktemp -d)
python3 -m venv --without-pip "$VENV"
PATH="$VENV/bin:$PATH"
export PATH
PYTHONPATH="$TEST/fake"
FAKE_SF_SEED="$TEST/fake/seed.sql"
export PYTHONPATH FAKE_SF_SEED

DIR=$(mktemp -d)
cp -R "$TEST/project/." "$DIR/"
cd "$DIR"
mkdir -p .countercheck/bin .countercheck/inventory snowflake/SALES
python3 "$RECIPE/oracle-inventory.py" oracle/SALES
cat "$RECIPE/checks.toml" "$RECIPE/compare-with-oracle.toml" > .countercheck/checks.toml
cp "$RECIPE/sf_query.py" .countercheck/bin/sf_query.py
EXPECTED="AS_EPOCH CALC_ORDER_TOTALS ORDER_API_CREATE_ORDER V_MONTHLY_REVENUE"
GOT=$(grep -v '^#' .countercheck/inventory/objects.txt | tr '\n' ' ' | sed 's/ $//')
if [ "$GOT" != "$EXPECTED" ]; then
  echo "run-test: inventory is \"$GOT\", expected \"$EXPECTED\"" >&2
  exit 1
fi
echo "run-test: inventory has the 4 expected objects (overloads listed once)"
printf 'out/\n.countercheck/receipts/\n' > .gitignore
git init -q -b main
git -c user.email=test@example.com -c user.name=test add -A
git -c user.email=test@example.com -c user.name=test commit -qm "Oracle schema, locked checks and Oracle answers"
echo "test repository: $DIR"

# expect <VERDICT> <exit code> <step> [text the receipt must contain]...
expect() {
  verdict=$1 code=$2 step=$3
  shift 3
  set +e
  OUT=$("$COUNTERCHECK" verify --claim "Done, everything is converted and matches Oracle.")
  CODE=$?
  set -e
  echo "$OUT"
  if [ "$CODE" -ne "$code" ] || ! echo "$OUT" | head -n 1 | grep -q " $verdict\$"; then
    echo "run-test: expected $verdict (exit $code) $step, got exit $CODE" >&2
    exit 1
  fi
  for want in "$@"; do
    if ! echo "$OUT" | grep -qF -- "$want"; then
      echo "run-test: $step: the receipt doesn't say \"$want\"" >&2
      exit 1
    fi
  done
  echo "run-test: $step: $verdict (exit $CODE), as expected"
  echo
}

proc() {   # the converted procedure; $1 = the expression for TOTAL
  cat > snowflake/SALES/calc_order_totals.sql <<EOF
CREATE OR REPLACE PROCEDURE SALES.CALC_ORDER_TOTALS()
RETURNS VARCHAR
LANGUAGE SQL
AS
\$\$
DROP TABLE IF EXISTS ORDER_TOTALS;
CREATE TABLE ORDER_TOTALS AS SELECT REGION, $1 AS TOTAL FROM ORDERS;
\$\$;
EOF
}

# 1. Nothing converted yet: the checks must bite.
expect FAILED 2 "before any conversion" "missing: CALC_ORDER_TOTALS" "missing: V_MONTHLY_REVENUE"

# 2. A plausible bad conversion: totals cut to integers, a ROWNUM note left in, the view skipped.
proc "CAST(AMOUNT AS INTEGER)"
echo "-- paged with ROWNUM in Oracle" >> snowflake/SALES/calc_order_totals.sql
printf 'CREATE OR REPLACE FUNCTION SALES.AS_EPOCH(D DATE)\nRETURNS NUMBER\nAS $$ DATE_PART(EPOCH_SECOND, D) $$;\n' > snowflake/SALES/as_epoch.sql
printf 'CREATE OR REPLACE PROCEDURE SALES.ORDER_API_CREATE_ORDER(P_REGION VARCHAR, P_AMOUNT NUMBER)\nRETURNS VARCHAR\nLANGUAGE SQL\nAS $$ INSERT INTO ORDERS VALUES (:P_REGION, :P_AMOUNT) $$;\n' > snowflake/SALES/order_api.sql
expect FAILED 2 "with a bad conversion" "missing: V_MONTHLY_REVENUE" "column TOTAL: expected 150.50, got 150" "calc_order_totals.sql:9"

# 3. Cheat: make Oracle's locked answer agree with the wrong numbers.
printf 'REGION,ORDERS,TOTAL\nEAST,2,150\nWEST,1,99\n' > .countercheck/golden/order_totals.csv
expect FAILED 2 "after editing Oracle's locked answer" "restored_from_base" "column TOTAL: expected 150.50, got 150"
git checkout -q -- .countercheck/golden

# 4. The real fix.
proc "AMOUNT"
printf "CREATE OR REPLACE VIEW SALES.V_MONTHLY_REVENUE AS SELECT '2026-01' AS MONTH, SUM(AMOUNT) AS REVENUE FROM ORDERS;\n" > snowflake/SALES/v_monthly_revenue.sql
expect VERIFIED 0 "after the fix"

echo "run-test: all verdicts as expected"
