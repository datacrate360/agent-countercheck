# Example: a Python project tested with pytest

A tiny pricing module with a planted bug (a discount can push the price below zero) and pytest tests. It shows how to
set up Countercheck so an agent can't switch a failing test off from **outside** `tests/`.

## The gap

pytest reads settings from more places than the `tests/` folder. With the usual `protected = ["tests/**"]` and a
plain `pytest -q`, each of these turns the planted bug's FAILED into **VERIFIED** without touching `tests/`:

| The agent adds or edits | What it does |
|---|---|
| `conftest.py` in the project root | marks every test as skipped |
| `src/conftest.py` (any folder pytest scans) | the same: pytest loads conftest files from every folder it collects |
| `pyproject.toml` with `addopts = "--deselect …::test_never_below_zero"` | the failing test never runs |
| `pytest.ini` with `addopts = -k "not below_zero"` | the same |

## The setup

[.countercheck/checks.toml](.countercheck/checks.toml):

```toml
protected = ["tests/**", "**/conftest.py", "pytest.ini", "tox.ini", "setup.cfg"]

[[check]]
name = "tests"
run = "python3 -m pytest -q -p no:cacheprovider -c tests/pytest.ini --rootdir=. --confcutdir=tests -o addopts= tests"
```

- `-c tests/pytest.ini`: pytest's settings come from a protected file, so a `pyproject.toml` or new `pytest.ini` the
  agent writes is ignored.
- `--confcutdir=tests`: only `conftest.py` files under `tests/` are loaded (and `**/conftest.py` is protected, so new
  ones are removed from the clean copy anyway).
- `-o addopts=`: no extra options, wherever they come from.
- **`pyproject.toml` is not protected**, on purpose: it also lists dependencies. Putting it back would undo an honest
  new dependency and fail the checks for no reason.

## Run it

```sh
python3 -m pip install pytest
sh examples/python-pytest/run-example.sh
```

The script makes a fresh git repository from this folder and expects: FAILED with the bug; FAILED after each of the
four tricks above; VERIFIED after the real fix (together with an honest `pyproject.toml` dependency change).
