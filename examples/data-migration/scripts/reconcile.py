"""Toy reconciliation: the converted tenant must keep every row and the same total amount."""
import argparse
import csv
import subprocess
import sys

p = argparse.ArgumentParser()
p.add_argument("--tenant", required=True)
tenant = p.parse_args().tenant

subprocess.run([sys.executable, "scripts/convert.py", tenant], check=True, stdout=subprocess.DEVNULL)
with open(f"source/{tenant}.csv", newline="") as f:
    src = list(csv.DictReader(f))
with open(f"target/{tenant}.csv", newline="") as f:
    dst = list(csv.DictReader(f))

src_total = sum(int(r["amount_cents"]) for r in src)
dst_total = round(sum(float(r["amount"]) for r in dst) * 100)
print(f"{tenant}: rows {len(src)} -> {len(dst)}, total cents {src_total} -> {dst_total}")
if len(src) != len(dst) or src_total != dst_total:
    print(f"{tenant}: MISMATCH")
    sys.exit(1)
print(f"{tenant}: ok")
