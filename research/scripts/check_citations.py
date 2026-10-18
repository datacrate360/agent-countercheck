"""Check every `path:line` citation in a research report against the pinned clone.

    python3 research/scripts/check_citations.py <report.md> <repo_root> [--ref <git-ref>]

A citation passes when the file exists and the line number is within the file (with --ref, the file is read from
that git ref, e.g. v0.22.2). Prints totals and up to 15 failures. Exit 1 if more than 2% fail.
"""
from __future__ import annotations

import re
import subprocess
import sys
from pathlib import Path

CITE = re.compile(r"`?([A-Za-z0-9_.@/\-]+\.[A-Za-z0-9]{1,6}):(\d+)(?:[-–](\d+))?(?:@[\w.]+)?`?")


_TREE: dict[str, list[str]] = {}


def resolve(root: Path, path: str, ref: str | None) -> str | None:
    """Short citations ("config.ts:12" after the full path was given once) resolve when exactly one
    tracked file ends with that suffix; ambiguous or unknown suffixes stay unresolved."""
    key = f"{root}@{ref}"
    if key not in _TREE:
        cmd = ["git", "-C", str(root), "ls-tree", "-r", "--name-only", ref] if ref else ["git", "-C", str(root), "ls-files"]
        _TREE[key] = subprocess.run(cmd, capture_output=True, text=True).stdout.splitlines()
    files = _TREE[key]
    if path in files:
        return path
    hits = [f for f in files if f.endswith("/" + path)]
    return hits[0] if len(hits) == 1 else None


def lines_in(root: Path, path: str, ref: str | None) -> int | None:
    path = resolve(root, path, ref) or path
    if ref:
        p = subprocess.run(["git", "-C", str(root), "show", f"{ref}:{path}"], capture_output=True, text=True)
        return p.stdout.count("\n") + 1 if p.returncode == 0 else None
    f = root / path
    return f.read_text(errors="replace").count("\n") + 1 if f.is_file() else None


def main(argv: list[str]) -> int:
    report, root = Path(argv[0]), Path(argv[1]).expanduser()
    ref = argv[argv.index("--ref") + 1] if "--ref" in argv else None
    cache: dict[str, int | None] = {}
    ok = bad = 0
    failures = []
    for path, start, end in CITE.findall(report.read_text()):
        if path.startswith(("http", "docs.", "www.")) or re.fullmatch(r"[\d.]+", path):
            continue
        if path not in cache:
            cache[path] = lines_in(root, path, ref)
        n = cache[path]
        last = int(end or start)
        if n is not None and 1 <= int(start) <= n and last <= n:
            ok += 1
        else:
            bad += 1
            why = "missing or ambiguous file" if n is None else f"file has {n} lines"
            failures.append(f"{path}:{start}{'-' + end if end else ''} ({why})")
    total = ok + bad
    rate = bad / total if total else 0
    print(f"{report.name}: {total} citations, {ok} ok, {bad} failed ({rate:.1%})")
    for f in failures[:15]:
        print("  FAIL", f)
    return 1 if rate > 0.02 else 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
