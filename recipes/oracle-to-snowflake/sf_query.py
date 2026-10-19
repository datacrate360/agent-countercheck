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
