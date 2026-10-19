"""A fake snowflake.connector for the recipe test: SQLite in memory, offline.

On connect it loads fake/seed.sql, creates every view found in snowflake/**/*.sql and remembers every procedure
body ($$ ... $$), so `CALL NAME()` runs the converted code. Just enough Snowflake for run-test.sh.
"""
import glob
import os
import re
import sqlite3


class _Cursor:
    def __init__(self, cur):
        self._cur = cur
        self.description = cur.description

    def fetchall(self):
        return self._cur.fetchall()


class _Connection:
    def __init__(self):
        self.db = sqlite3.connect(":memory:")
        with open(os.environ["FAKE_SF_SEED"]) as f:
            self.db.executescript(f.read())
        self.procs = {}
        for path in sorted(glob.glob("snowflake/**/*.sql", recursive=True)):
            with open(path) as f:
                text = f.read()
            for m in re.finditer(r"(?is)CREATE\s+OR\s+REPLACE\s+VIEW\s+(?:\w+\.)*(\w+)\s+AS\s+(.*?);", text):
                self.db.execute(f"CREATE VIEW {m.group(1)} AS {m.group(2)}")
            for m in re.finditer(r"(?is)CREATE\s+OR\s+REPLACE\s+PROCEDURE\s+(?:\w+\.)*(\w+)\s*\(.*?\$\$(.*?)\$\$", text):
                self.procs[m.group(1).upper()] = m.group(2)

    def execute_string(self, sql):
        out = []
        for stmt in [s.strip() for s in sql.split(";") if s.strip()]:
            call = re.match(r"(?i)CALL\s+(\w+)", stmt)
            if call:
                body = self.procs.get(call.group(1).upper())
                if body is None:
                    raise RuntimeError(f"SQL compilation error: Unknown procedure {call.group(1)}")
                self.db.executescript(body)
                continue
            out.append(_Cursor(self.db.execute(stmt)))
        return out

    def close(self):
        self.db.close()


def connect(**_kwargs):
    return _Connection()
