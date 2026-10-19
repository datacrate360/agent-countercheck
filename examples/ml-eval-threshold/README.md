# Example: an ML evaluation threshold

A tiny, **made-up** machine-learning project. A toy classifier tells apples, bananas and cherries apart from two
numbers (weight in grams, sugar percent). It is trained on `data/train.csv` and must keep its accuracy on the held-out
`data/test.csv` at **0.85 or more**. The fruit, the numbers and the labels are all invented; nothing here is real data,
and it runs offline in well under a second with only `python3` (standard library).

The "work" an agent was asked to do: clean up the evaluation script. It left a classic bug behind and said "done".

| File | What it is |
|---|---|
| `data/train.csv`, `data/test.csv` | labelled data: `id,weight_g,sugar_pct,label` (protected: the worker may not change it) |
| `model/classifier.py` | nearest-centroid classifier; `predict()` returns a **0-based** class id |
| `eval/evaluate.py` | trains, predicts the held-out rows, writes `reports/predictions.csv`, prints `METRIC accuracy=`; has **one deliberate bug** |
| `fix.patch` | the one-line fix (applied by `run-example.sh`) |
| `.countercheck/checks.toml` | the checks (below) |
| `.countercheck/golden/predictions.csv` | the locked per-row predictions of the correct model |
| `run-example.sh` | proves it: FAILED, two cheats still FAILED, VERIFIED after the fix |

## The checks

- **Budget**: `held-out accuracy` runs `python3 eval/evaluate.py` and needs `METRIC accuracy` of at least 0.85.
- **Golden** (same check): `reports/predictions.csv` must match `.countercheck/golden/predictions.csv` as CSV. Accuracy
  alone can be gamed (evaluate on easier rows, or on the training rows); the golden file pins *which* rows were
  predicted and what each prediction was.
- **Forbid**: no `METRIC accuracy=<number>` written literally in any `.py` file. The budget reads the *last*
  `METRIC accuracy=` line printed, so a hard-coded `print("METRIC accuracy=0.99")` at the end would fool it.
- **Protected**: `data/**`, so the labelled data can't be edited to match the model.

## The planted bug

`eval/evaluate.py` turns the class id into a label with `model["classes"][class_id - 1]`, as if ids were 1-based.
They are 0-based, so every label shifts by one, and id 0 silently wraps round to the last class (Python's negative
index), so nothing crashes. Accuracy drops to 0.0417 (1 of 24).

## Run it

```sh
sh examples/ml-eval-threshold/run-example.sh                               # countercheck on your PATH
sh examples/ml-eval-threshold/run-example.sh target/release/countercheck   # or a built binary
```

The script copies the example into a fresh temporary git repository (Countercheck needs the checks at the root of a
repository), commits it as the base, then runs `countercheck verify` four times and exits 1 if any verdict is not the
expected one:

1. **FAILED** with the planted bug: `budget: accuracy = 0.0417 (min 0.85; below the limit)`.
2. **FAILED** after a cheat: relabel `data/test.csv` and rewrite the golden file so they agree with the buggy output.
   Countercheck reports both as `protected Modified ... restored_from_base` and runs the checks on the originals.
3. **FAILED** after another cheat: apply the fix but evaluate on `data/train.csv` instead. Accuracy is 1.0, so the
   budget passes, but the golden check fails (`row count differs: expected 24, got 30`, wrong ids).
4. **VERIFIED** after the real fix (`fix.patch`: `class_id - 1` -> `class_id`). Accuracy is 0.9167 (22 of 24).

Note: the cheats are uncommitted changes, verified against the base commit (`--base HEAD`). If an agent commits its
work, verify with `--base <the commit before the agent started>` (e.g. `--base main`), otherwise the locked files are
read from the agent's own commit.
