"""Build a Countercheck inventory (one object per line) from Oracle DDL already extracted to files.

Expects one folder per schema with procedures.sql, functions.sql, views_ddl.sql, package_bodies.sql (missing files
are skipped). Package routines are listed as <PACKAGE>_<ROUTINE> by default (Snowflake has no packages); change it
with --package-format to match how your team names them in Snowflake.

Usage:
  python3 oracle-inventory.py Oracle/SALES                      # everything in the schema
  python3 oracle-inventory.py Oracle/SALES --package ORDER_API   # one package only (a good first pilot)
  python3 oracle-inventory.py Oracle/SALES --package-format "{package}.{routine}"

Part of the Countercheck recipe recipes/oracle-to-snowflake. Standard library only (Python 3.9+).
"""
from __future__ import annotations

import argparse
import re
import sys
from pathlib import Path

# Oracle writes the options in any order: CREATE OR REPLACE FORCE EDITIONABLE VIEW, NONEDITIONABLE PROCEDURE...
CREATE = r"(?im)^\s*CREATE\s+(?:OR\s+REPLACE\s+)?(?:(?:NON)?EDITIONABLE\s+|(?:NO\s+)?FORCE\s+)*{kind}\s+"
NAME = r'((?:"?\w+"?\.)?"?[\w$#]+"?)'
ROUTINE = re.compile(r"(?im)^\s*(?:PROCEDURE|FUNCTION)\s+\"?([\w$#]+)\"?")


def clean(name: str) -> str:
    return name.replace('"', "").split(".")[-1].upper()


def read(folder: Path, file: str) -> str:
    p = folder / file
    return p.read_text(errors="replace") if p.is_file() else ""


def standalone(folder: Path) -> set[str]:
    found = set()
    for file, kind in [("procedures.sql", "PROCEDURE"), ("functions.sql", "FUNCTION"),
                       ("views_ddl.sql", "(?:MATERIALIZED\\s+)?VIEW")]:
        found |= {clean(n) for n in re.findall(CREATE.format(kind=kind) + NAME, read(folder, file))}
    return found


def package_routines(folder: Path, only: str | None, fmt: str) -> set[str]:
    text = read(folder, "package_bodies.sql")
    parts = re.split(CREATE.format(kind=r"PACKAGE\s+BODY") + NAME, text)
    found = set()
    for name, body in zip(parts[1::2], parts[2::2]):
        package = clean(name)
        if only and package != only.upper():
            continue
        found |= {fmt.format(package=package, routine=r.upper()) for r in ROUTINE.findall(body)}
    return found


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("schema_folder", type=Path)
    ap.add_argument("--package", help="only the routines of this package")
    ap.add_argument("--package-format", default="{package}_{routine}")
    ap.add_argument("-o", "--output", type=Path, default=Path(".countercheck/inventory/objects.txt"))
    args = ap.parse_args()
    if not args.schema_folder.is_dir():
        print(f"no such folder: {args.schema_folder}", file=sys.stderr)
        return 1
    items = package_routines(args.schema_folder, args.package, args.package_format)
    if not args.package:
        items |= standalone(args.schema_folder)
    if not items:
        print("nothing found: check the folder and --package name", file=sys.stderr)
        return 1
    args.output.parent.mkdir(parents=True, exist_ok=True)
    header = f"# from {args.schema_folder.name}" + (f", package {args.package.upper()}" if args.package else "")
    args.output.write_text(header + "\n" + "\n".join(sorted(items)) + "\n")
    print(f"{len(items)} objects -> {args.output}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
