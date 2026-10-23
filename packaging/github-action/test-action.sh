#!/usr/bin/env bash
# Local test of packaging/github-action/verify.sh: simulates the GitHub runner env on fixture repos.
# Usage: test-action.sh <repo root> <countercheck binary dir>
set -u
ROOT="$1"; export PATH="$2:$PATH"
W="${TMPDIR:-/tmp}/cc-action-test"; rm -rf "$W"; mkdir -p "$W"
fail=0

fixture() { # $1 name; base commit has a passing test protected; then $2 = change kind
  d="$W/$1"; mkdir -p "$d/.countercheck" "$d/tests" && cd "$d" || exit 1
  git init -q -b main && git config user.email t@t && git config user.name t
  printf 'def add(a,b): return a+b\n' > calc.py
  printf 'from calc import add\nassert add(2,2)==4\nprint("ok")\n' > tests/test_calc.py
  printf 'protected = ["tests/**"]\n[[check]]\nname = "unit tests"\nrun = "PYTHONPATH=. python3 tests/test_calc.py"\n' > .countercheck/checks.toml
  [ "$2" = nochecks ] && rm .countercheck/checks.toml
  git add -A && git commit -qm base
  case "$2" in
    good) printf 'def add(a,b):\n    return a + b\n' > calc.py ;;
    tamper) printf 'def add(a,b): return a-b\n' > calc.py; printf 'print("ok")\n' > tests/test_calc.py ;;
    nochecks) printf '# x\n' >> calc.py ;;
  esac
  git add -A && git commit -qm change
  git rev-parse HEAD~1
}

run() { # $1 name, $2 expected verdict, $3 expected exit, $4 fail-on
  base=$(fixture "$1" "$5" | tail -1)
  export GITHUB_OUTPUT="$W/$1.out" GITHUB_STEP_SUMMARY="$W/$1.md" RUNNER_TEMP="$W/$1.tmp"
  mkdir -p "$RUNNER_TEMP"; : > "$GITHUB_OUTPUT"; : > "$GITHUB_STEP_SUMMARY"
  IN_BASE="" IN_CLAIM="" IN_AGENT="claude-code" IN_MODEL="" IN_PATH="$W/$1" IN_SANDBOX=auto IN_CHECKS_FROM="${CHECKS_FROM:-}" \
    IN_CHECKS_SHA="" IN_FAIL_ON="$4" PR_BASE="$base" PR_TITLE='Fix add; all tests pass $(touch /tmp/pwned) `x`' PUSH_BEFORE="" \
    bash "$ROOT/packaging/github-action/verify.sh" > "$W/$1.log" 2>&1
  code=$?
  v=$(grep '^verdict=' "$GITHUB_OUTPUT" | cut -d= -f2)
  id=$(grep '^receipt-id=' "$GITHUB_OUTPUT" | cut -d= -f2)
  ok=yes
  [ "$v" = "$2" ] && [ "$code" = "$3" ] || ok=NO
  if [ "$2" != error ]; then
    [ -n "$id" ] && [ -f "$RUNNER_TEMP/countercheck-receipts/$id.json" ] && [ -f "$RUNNER_TEMP/countercheck-receipts/report.html" ] || ok=NO
  fi
  grep -q "Countercheck: $(echo "$2" | tr a-z A-Z)" "$GITHUB_STEP_SUMMARY" || ok=NO
  echo "$ok  $1: verdict=$v exit=$code receipt=$id"
  [ "$ok" = yes ] || { fail=1; cat "$W/$1.log"; }
}

run good verified 0 unverified good
run tamper failed 1 unverified tamper
run tamper-report failed 0 never tamper
run nochecks error 1 unverified nochecks
run nochecks-lenient error 1 failed nochecks
grep -q "no .countercheck/checks.toml found" "$W/nochecks.md" && echo "yes  the error's reason is in the summary" || { echo "NO  error summary"; fail=1; }
# Checks from a file outside the repo without --checks-sha256: they run, but aren't locked, so UNVERIFIED.
mkdir -p "$W/suite"; printf '[[check]]\nname = "unit tests"\nrun = "PYTHONPATH=. python3 tests/test_calc.py"\n' > "$W/suite/checks.toml"
CHECKS_FROM="$W/suite/checks.toml" run unpinned unverified 1 unverified good
CHECKS_FROM="$W/suite/checks.toml" run unpinned-lenient unverified 0 failed good
[ -e /tmp/pwned ] && { echo "NO  the PR title ran a command"; fail=1; } || echo "yes  the PR title was not executed"
grep -q 'Fix add; all tests pass $(touch /tmp/pwned)' "$W/good.tmp/countercheck-receipts/"*.json && echo "yes  claim = PR title, verbatim" || { echo "NO  claim"; fail=1; }
echo "--- summary of the tamper case ---"; cat "$W/tamper.md"
exit $fail
