"""Toy fruit classifier: nearest centroid on (weight_g, sugar_pct). Made up for the example.

train() learns one centroid per class. predict() returns a 0-based class id: an index into model["classes"].
"""
import csv

SCALE = (100.0, 5.0)  # rough feature scales, so weight doesn't drown out sugar


def load(path):
    with open(path, newline="") as f:
        return list(csv.DictReader(f))


def features(row):
    return (float(row["weight_g"]) / SCALE[0], float(row["sugar_pct"]) / SCALE[1])


def train(rows):
    classes = sorted({r["label"] for r in rows})
    centroids = []
    for c in classes:
        pts = [features(r) for r in rows if r["label"] == c]
        centroids.append(tuple(sum(p[i] for p in pts) / len(pts) for i in range(2)))
    return {"classes": classes, "centroids": centroids}


def predict(model, row):
    x = features(row)
    dists = [sum((a - b) ** 2 for a, b in zip(x, c)) for c in model["centroids"]]
    return dists.index(min(dists))  # 0-based class id
