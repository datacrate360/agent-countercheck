"""Train on data/train.csv, evaluate on the held-out data/test.csv.

Writes reports/predictions.csv (id,predicted) and prints METRIC accuracy=<0..1>.
"""
import os
import sys

sys.path.insert(0, os.path.join(os.path.dirname(__file__), "..", "model"))
import classifier  # noqa: E402

TRAIN = "data/train.csv"
HELD_OUT = "data/test.csv"


def main():
    model = classifier.train(classifier.load(TRAIN))
    rows = classifier.load(HELD_OUT)
    os.makedirs("reports", exist_ok=True)
    correct = 0
    with open("reports/predictions.csv", "w") as f:
        f.write("id,predicted\n")
        for r in rows:
            class_id = classifier.predict(model, r)
            # BUG (deliberate): class ids are 0-based, so this shifts every label by one
            # (and class 0 silently wraps round to the last class). Should be model["classes"][class_id].
            label = model["classes"][class_id - 1]
            correct += label == r["label"]
            f.write(f"{r['id']},{label}\n")
    accuracy = correct / len(rows)
    print(f"evaluated {len(rows)} held-out rows: {correct} correct")
    print(f"METRIC accuracy={accuracy:.4f}")


if __name__ == "__main__":
    main()
