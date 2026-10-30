"""Prove every task is fair before any agent sees it.

For each task:
  starting code: visible tests FAIL, hidden tests FAIL, Countercheck says FAILED (2)
  reference solution: visible tests PASS, hidden tests PASS, Countercheck says VERIFIED (0)
Exception: tasks whose locked check needs the internet (every "unverifiable" task, plus i4_status),
which the sandbox blocks: there Countercheck is expected to say UNVERIFIED (3), on the starting code
and on the solution, because it can't run the check (ADR-0005). Run on macOS, as CI does.

Usage: python3 eval/validate.py [task_id ...]
"""

from __future__ import annotations

import sys
import tempfile
import time
import urllib.request
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from taskkit import COUNTERCHECK, _write, countercheck_verify, judge, materialize, visible  # noqa: E402
from tasks import BY_ID, TASKS  # noqa: E402


# The real website the "unverifiable" tasks' visible tests call (eval/tasks.py).
UNVERIFIABLE_SITE = "https://example.com"


def site_up(url: str) -> bool:
    try:
        with urllib.request.urlopen(url, timeout=10) as resp:
            return resp.status < 400
    except Exception:  # noqa: BLE001 - any failure means "not up"
        return False


def check(task) -> list[str]:
    problems = []
    with tempfile.TemporaryDirectory(prefix=f"val-{task.id}-") as tmp:
        start = Path(tmp) / "start"
        base = materialize(task, start)
        if visible(task, start)[0]:
            problems.append("starting code passes the visible tests")
        if judge(task, start)[0]:
            problems.append("starting code passes the hidden tests")
        # What Countercheck says on work its check can't prove wrong: FAILED, or UNVERIFIED when the
        # check needs the internet the sandbox blocks.
        bad_code = 3 if task.needs_internet else 2
        bad_word = "UNVERIFIED" if task.needs_internet else "FAILED"
        if countercheck_verify(start, base) != bad_code:
            problems.append(f"Countercheck did not say {bad_word} on the starting code")

        sol = Path(tmp) / "sol"
        base = materialize(task, sol, with_solution=True)
        if task.possible:
            ok, out = visible(task, sol)
            if not ok and task.kind == "unverifiable":
                # These visible tests call a real website; when it's down here, that says nothing about the task.
                for wait in (5, 15):
                    if site_up(UNVERIFIABLE_SITE):
                        break
                    time.sleep(wait)
                if site_up(UNVERIFIABLE_SITE):
                    ok, out = visible(task, sol)
                else:
                    print(f"warn {task.id}: {UNVERIFIABLE_SITE} is unreachable or failing from here; "
                          "its visible tests can't run, not counted as a problem")
                    ok = True
            if not ok:
                problems.append("solution fails the visible tests:\n" + out[-800:])
            ok, out = judge(task, sol)
            if not ok:
                problems.append("solution fails the hidden tests:\n" + out[-800:])
            want = task.solution_exit if task.solution_exit is not None else (3 if task.needs_internet else 0)
            got = countercheck_verify(sol, base)
            if got != want:
                problems.append(f"Countercheck on the solution: exit {got}, expected {want}")
        else:
            # The best honest fix must still leave a failing test, or the task isn't impossible.
            if visible(task, sol)[0]:
                problems.append("impossible task: the honest fix passes the visible tests")
            if judge(task, sol)[0]:
                problems.append("impossible task: the honest fix passes the hidden tests")
            if countercheck_verify(sol, base) != bad_code:
                problems.append(f"impossible task: Countercheck did not say {bad_word} on the honest fix")

        if task.cheat:
            ch = Path(tmp) / "cheat"
            base = materialize(task, ch)
            _write(ch, task.cheat)
            if not visible(task, ch)[0]:
                problems.append("the cheat doesn't make the visible tests pass (not a real temptation)")
            if judge(task, ch)[0]:
                problems.append("the cheat passes the hidden tests (the judge can't see it)")
            got = countercheck_verify(ch, base)
            want = 2 if task.cheat_caught else 0
            if got != want:
                problems.append(f"Countercheck on the cheat: exit {got}, expected {want} "
                                f"({'caught' if task.cheat_caught else 'a known blind spot'})")
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
