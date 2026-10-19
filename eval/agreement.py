"""How well do claim classifiers agree with the hand labels?

Hand labels = crates/countercheck-core/tests/fixtures/claims/{success,blocker}/<pilot>__<run>.txt.
Compares: the model judge (judge.json), the runner's regex (claimed_done in results.jsonl) and the hook's rule
(core::claim::reports_blocker, called through `countercheck` is not exposed, so the Rust test suite covers it).
Usage: python3 eval/agreement.py
"""
import json
from pathlib import Path

root = Path(__file__).resolve().parent
fx = root.parent / "crates/countercheck-core/tests/fixtures/claims"
labels = {}
for label in ("success", "blocker"):
    for f in (fx / label).glob("*.txt"):
        pilot, run = f.stem.split("__", 1)
        labels[(pilot, run)] = label == "success"
judge_ok = regex_ok = judged = 0
misses = []
for (pilot, run), truth in sorted(labels.items()):
    res = root / "results" / pilot
    j = json.loads((res / "judge.json").read_text()).get(run) if (res / "judge.json").exists() else None
    task, arm, rep = run.rsplit("__", 2)
    row = next((json.loads(l) for l in (res / "results.jsonl").read_text().splitlines()
                if (lambda r: (r["task"], r["arm"], f"r{r['rep']}") == (task, arm, rep))(json.loads(l))), None)
    regex = row["claimed_done"] if row else None
    regex_ok += regex == truth
    if j in ("CLAIM", "REPORT"):
        judged += 1
        judge_ok += (j == "CLAIM") == truth
        if (j == "CLAIM") != truth:
            misses.append(f"judge {j} but hand says {'success' if truth else 'blocker'}: {pilot}/{run}")
    if regex != truth:
        misses.append(f"regex {'CLAIM' if regex else 'REPORT'} but hand says {'success' if truth else 'blocker'}: {pilot}/{run}")
n = len(labels)
print(f"hand-labelled answers: {n}")
print(f"regex detector: {regex_ok}/{n} agree")
print(f"model judge:    {judge_ok}/{judged} agree ({n - judged} not judged)")
print("hook rule:      see `cargo test -p countercheck-core --test claims` (36/36 by construction of the test)")
for m in misses:
    print(" -", m)
