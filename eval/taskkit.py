"""Build task repos and judge results with the hidden tests."""

from __future__ import annotations

import os
import shutil
import subprocess
import tempfile
from pathlib import Path

from tasks import Task

ROOT = Path(__file__).resolve().parent.parent
COUNTERCHECK = Path(os.environ.get("COUNTERCHECK_BIN", ROOT / "target" / "release" / "countercheck"))
GIT_ENV = {"GIT_AUTHOR_NAME": "eval", "GIT_AUTHOR_EMAIL": "eval@localhost",
           "GIT_COMMITTER_NAME": "eval", "GIT_COMMITTER_EMAIL": "eval@localhost"}


def _write(root: Path, files: dict[str, str]) -> None:
    for rel, text in files.items():
        p = root / rel
        p.parent.mkdir(parents=True, exist_ok=True)
        p.write_text(text)


def git(dest: Path, *args: str) -> str:
    out = subprocess.run(["git", "-C", str(dest), *args], capture_output=True, text=True,
                         env={**os.environ, **GIT_ENV}, check=True)
    return out.stdout.strip()


def materialize(task: Task, dest: Path, with_solution: bool = False) -> str:
    """Create the starting repo (optionally with the solution applied, uncommitted). Returns the base commit."""
    dest.mkdir(parents=True, exist_ok=True)
    _write(dest, task.files)
    _write(dest, task.tests)
    if task.lang == "py":
        _write(dest, {"tests/__init__.py": ""})
    _write(dest, {".countercheck/checks.toml": task.checks_toml(),
                  ".gitignore": "__pycache__/\n*.pyc\n.countercheck/receipts/\nnode_modules/\n"})
    git(dest, "init", "-q", "-b", "main")
    git(dest, "add", "-A")
    git(dest, "commit", "-qm", "start")
    base = git(dest, "rev-parse", "HEAD")
    if with_solution:
        _write(dest, task.solution)
    return base


def run(cmd: str, cwd: Path, timeout: int = 300) -> tuple[bool, str]:
    try:
        p = subprocess.run(cmd, shell=True, cwd=cwd, capture_output=True, text=True, timeout=timeout)
        return p.returncode == 0, (p.stdout + p.stderr)[-3000:]
    except subprocess.TimeoutExpired:
        return False, "timed out"


def judge(task: Task, workdir: Path) -> tuple[bool, str]:
    """Run the hidden tests on a copy of the worker's final files (the original is never touched)."""
    with tempfile.TemporaryDirectory(prefix="judge-") as tmp:
        copy = Path(tmp) / "w"
        shutil.copytree(workdir, copy, ignore=shutil.ignore_patterns(".git", "__pycache__", "receipts"))
        _write(copy, task.hidden)
        if task.lang == "py":
            _write(copy, {"hidden_tests/__init__.py": ""})
        return run(task.hidden_cmd, copy)


def visible(task: Task, workdir: Path) -> tuple[bool, str]:
    return run(task.check_cmd, workdir)


def countercheck_verify(workdir: Path, base: str, sandbox: str = "auto") -> int:
    # "auto": Seatbelt on macOS, Landlock + namespaces on Linux, the job sandbox on Windows.
    p = subprocess.run([str(COUNTERCHECK), "verify", "--repo", str(workdir), "--base", base,
                        "--sandbox", sandbox], capture_output=True, text=True)
    return p.returncode
