"""Prove every task is fair before any agent sees it.

For each task:
  starting code: visible tests FAIL, hidden tests FAIL, Countercheck says FAILED (2)
  reference solution: visible tests PASS, hidden tests PASS, Countercheck says VERIFIED (0)
Exception: "unverifiable" tasks need the network, which Countercheck's sandbox blocks, so on the
solution Countercheck is expected to say FAILED; that is the known limitation the eval measures.

Usage: python3 eval/validate.py [task_id ...]
"""

from __future__ import annotations

import sys
import tempfile
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from taskkit import COUNTERCHECK, countercheck_verify, judge, materialize, visible  # noqa: E402
from tasks import BY_ID, TASKS  # noqa: E402


def check(task) -> list[str]:
    problems = []
    with tempfile.TemporaryDirectory(prefix=f"val-{task.id}-") as tmp:
        start = Path(tmp) / "start"
        base = materialize(task, start)
        if visible(task, start)[0]:
            problems.append("starting code passes the visible tests")
        if judge(task, start)[0]:
            problems.append("starting code passes the hidden tests")
        if countercheck_verify(start, base) != 2:
            problems.append("Countercheck did not say FAILED on the starting code")

        sol = Path(tmp) / "sol"
        base = materialize(task, sol, with_solution=True)
        ok, out = visible(task, sol)
        if not ok:
            problems.append("solution fails the visible tests:\n" + out[-800:])
        ok, out = judge(task, sol)
        if not ok:
            problems.append("solution fails the hidden tests:\n" + out[-800:])
        want = 2 if task.kind == "unverifiable" else 0
        got = countercheck_verify(sol, base)
        if got != want:
            problems.append(f"Countercheck on the solution: exit {got}, expected {want}")
    return problems


def main() -> int:
    if not COUNTERCHECK.exists():
        print(f"build countercheck first: {COUNTERCHECK} missing (cargo build --release)")
        return 1
    tasks = [BY_ID[i] for i in sys.argv[1:]] if sys.argv[1:] else TASKS
    bad = 0
    for t in tasks:
        problems = check(t)
        print(f"{'ok  ' if not problems else 'FAIL'} {t.id:18} {t.kind}")
        for p in problems:
            print("     - " + p.replace("\n", "\n       "))
        bad += bool(problems)
    kinds = {}
    for t in tasks:
        kinds[t.kind] = kinds.get(t.kind, 0) + 1
    print(f"\n{len(tasks) - bad}/{len(tasks)} tasks valid  {kinds}")
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main())
