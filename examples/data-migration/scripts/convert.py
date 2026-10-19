"""Toy converter: legacy tenant CSV (source/<tenant>.csv) -> new format (target/<tenant>.csv).

Legacy: id,name,amount_cents,status_code (A/I).  New: id,name,amount,status (dollars, active/inactive).
"""
import csv
import os
import sys

STATUS = {"A": "active", "I": "inactive"}


def main(tenant):
    os.makedirs("target", exist_ok=True)
    with open(f"source/{tenant}.csv", newline="") as f:
        rows = list(csv.DictReader(f))
    out = []
    for r in rows:
        cents = int(r["amount_cents"])
        # BUG (deliberate): integer division drops the cents. Should be cents / 100.
        amount = cents // 100
        out.append([r["id"], r["name"], f"{amount:.2f}", STATUS[r["status_code"]]])
    out.sort(key=lambda row: row[1])  # by name; the golden file is by id (sort_rows handles it)
    with open(f"target/{tenant}.csv", "w", newline="") as f:
        w = csv.writer(f, lineterminator="\n")
        w.writerow(["id", "name", "amount", "status"])
        w.writerows(out)
    print(f"converted {len(out)} rows for {tenant}")
    print(f"METRIC rows={len(out)}")


if __name__ == "__main__":
    main(sys.argv[1])
