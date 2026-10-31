#!/usr/bin/env bash
# Local test of packaging/github-action/verify.sh: simulates the GitHub runner env on fixture repos.
# Usage: test-action.sh <repo root> <countercheck binary dir>
set -u
ROOT="$1"; export PATH="$2:$PATH"
W="${TMPDIR:-/tmp}/cc-action-test"; rm -rf "$W"; mkdir -p "$W"
fail=0
py=""; for c in python3 python; do "$c" -c 1 >/dev/null 2>&1 && { py="$c"; break; }; done

# A fake `gh`, first on PATH: logs each call, keeps the PR's comments in $FAKE_GH_STATE, and fails the
# methods in $FAKE_GH_FAIL with a 403 like a fork's read-only token. A bash wrapper, so Git Bash runs it too.
mkdir -p "$W/fakebin"
cat > "$W/fake_gh.py" <<'PY'
import json, os, sys
args = sys.argv[1:]
with open(os.environ["FAKE_GH_LOG"], "a", encoding="utf-8") as f:
    f.write(" ".join(args) + "\n")
method, inp, path, i = "GET", None, None, 1
while i < len(args):
    if args[i] in ("--method", "--input"):
        if args[i] == "--method": method = args[i + 1]
        else: inp = args[i + 1]
        i += 2
    elif args[i].startswith("-"): i += 1
    else: path, i = args[i], i + 1
if method in os.environ.get("FAKE_GH_FAIL", "").split(","):
    sys.stderr.write("HTTP 403: Resource not accessible by integration\n"); sys.exit(1)
state = os.environ["FAKE_GH_STATE"]
comments = json.load(open(state, encoding="utf-8"))
if method == "GET":  # two pages, as --paginate prints them
    print(json.dumps(comments[:1])); print(json.dumps(comments[1:]))
else:
    body = json.load(open(inp, encoding="utf-8"))["body"]
    if method == "POST":
        comments.append({"id": 1000 + len(comments), "user": {"login": "github-actions[bot]"}, "body": body})
    else:
        cid = int(path.rsplit("/", 1)[1])
        for c in comments:
            if c["id"] == cid: c["body"] = body
    json.dump(comments, open(state, "w", encoding="utf-8"))
    print("{}")
PY
printf '#!/usr/bin/env bash\nexec "%s" "%s" "$@"\n' "$py" "$W/fake_gh.py" > "$W/fakebin/gh"; chmod +x "$W/fakebin/gh"
export PATH="$W/fakebin:$PATH" FAKE_GH_LOG="$W/gh.log" FAKE_GH_STATE="$W/gh-state.json"
# The PR already has a comment with our marker by someone else (must not be touched) and another bot comment.
seed_comments() {
  printf '[{"id":1,"user":{"login":"mallory"},"body":"<!-- agent-countercheck -->\\nVERIFIED"},{"id":2,"user":{"login":"github-actions[bot]"},"body":"lint ok"}]' > "$FAKE_GH_STATE"
  : > "$FAKE_GH_LOG"
}
seed_comments

fixture() { # $1 name; base commit has a passing test protected; then $2 = change kind
  d="$W/$1"; mkdir -p "$d/.countercheck" "$d/tests" && cd "$d" || exit 1
  git init -q -b main && git config user.email t@t && git config user.name t
  printf 'def add(a,b): return a+b\n' > calc.py
  printf 'from calc import add\nassert add(2,2)==4\nprint("ok")\n' > tests/test_calc.py
  printf 'protected = ["tests/**"]\n[[check]]\nname = "unit tests"\nrun = "PYTHONPATH=. python3 tests/test_calc.py"\n' > .countercheck/checks.toml
  [ "$2" = nochecks ] && rm .countercheck/checks.toml
  # Attacker text in a check's name and output.
  [ "$2" = hostile ] && cat > .countercheck/checks.toml <<'TOML'
[[check]]
name = "<script>alert(1)</script> @all"
run = "echo '```'; echo '@all <img src=x onerror=alert(1)>'; echo '</details>'; echo '````'; exit 1"
TOML
  git add -A && git commit -qm base
  case "$2" in
    good) printf 'def add(a,b):\n    return a + b\n' > calc.py ;;
    tamper|hostile) printf 'def add(a,b): return a-b\n' > calc.py; printf 'print("ok")\n' > tests/test_calc.py ;;
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
    IN_CHECKS_SHA="" IN_FAIL_ON="$4" PR_BASE="$base" PR_TITLE="${TITLE:-Fix add; all tests pass \$(touch /tmp/pwned) \`x\`}" PUSH_BEFORE="" \
    bash "$ROOT/packaging/github-action/verify.sh" > "$W/$1.log" 2>&1
  code=$?
  # The next step of action.yml, the PR comment. It must never change the verdict or the exit code.
  v=$(grep '^verdict=' "$GITHUB_OUTPUT" | cut -d= -f2)
  IN_COMMENT="${COMMENT:-false}" VERDICT="$v" PR_NUMBER="${PRN-7}" GITHUB_REPOSITORY=acme/app GH_TOKEN=fake \
    GITHUB_SERVER_URL=https://github.com GITHUB_RUN_ID=42 \
    bash "$ROOT/packaging/github-action/comment.sh" >> "$W/$1.log" 2>&1 || { echo "NO  comment.sh exited non-zero"; fail=1; }
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
[ -s "$FAKE_GH_LOG" ] && { echo "NO  comment=false still called gh:"; cat "$FAKE_GH_LOG"; fail=1; } || echo "yes  comment=false makes no API call"
grep -q 'Fix add; all tests pass $(touch /tmp/pwned)' "$W/good.tmp/countercheck-receipts/"*.json && echo "yes  claim = PR title, verbatim" || { echo "NO  claim"; fail=1; }

# --- The PR comment (input `comment`) ---
check() { if eval "$2"; then echo "yes  $1"; else echo "NO  $1"; fail=1; fi; }
calls() { grep -c -- "$1" "$FAKE_GH_LOG"; }
ours() { "$py" -c 'import json,sys; c=[x for x in json.load(open(sys.argv[1],encoding="utf-8")) if x["user"]["login"]=="github-actions[bot]" and x["body"].startswith("<!-- agent-countercheck -->")]; print(len(c), c[0]["id"] if c else "")' "$FAKE_GH_STATE"; }
export TITLE='```
@all @acme/team </details><script>alert(1)</script> [x](javascript:alert(1)) <!-- agent-countercheck -->'
seed_comments
COMMENT=true run hostile1 failed 1 unverified hostile
check "first run creates one comment" '[ "$(calls "method POST")" = 1 ] && [ "$(ours)" = "1 1002" ]'
COMMENT=true run hostile2 failed 1 unverified hostile
check "second run updates the same comment" '[ "$(calls "method POST")" = 1 ] && [ "$(calls "method PATCH .*comments/1002 ")" = 1 ] && [ "$(ours)" = "1 1002" ]'
check "someone else's comment with the marker is untouched" 'grep -q "\"id\": 1, \"user\": {\"login\": \"mallory\"}, \"body\": \"<!-- agent-countercheck -->\\\\nVERIFIED\"" "$FAKE_GH_STATE"'
"$py" -c 'import json,sys; print([x["body"] for x in json.load(open(sys.argv[1],encoding="utf-8")) if x["id"]==1002][0], end="")' "$FAKE_GH_STATE" > "$W/comment.md"
# The escaping: hostile text is data. Checked on the posted body.
"$py" - "$W/comment.md" <<'PY' && echo "yes  hostile text is escaped (fence, @mentions, HTML, links, marker)" || { echo "NO  escaping"; fail=1; }
import re, sys
b = open(sys.argv[1], encoding="utf-8").read(); lines = b.split("\n")
problems = []
# The output fence (CommonMark: it ends at the first line of at least as many backticks) must end on our own
# line, just before </details>. Inside it text is inert; everything outside is checked.
o = next((i for i, l in enumerate(lines) if re.fullmatch("`{3,}text", l)), None)
if o is None: problems.append("no output fence"); o = c = len(lines)
else:
    n = len(lines[o]) - 4
    c = next((i for i in range(o + 1, len(lines)) if re.fullmatch("`{%d,}\\s*" % n, lines[i])), len(lines) - 1)
    if lines[c + 1:c + 2] != ["</details>"] or not any("````" in l for l in lines[o + 1:c]):
        problems.append("the fence can be closed early")
out = "\n".join(lines[:o] + lines[c + 1:])
if re.search("@(?!\u200b)", out): problems.append("an @ that can ping")
if re.search(r"<(?!/?details>|summary>|/summary>|!-- agent-countercheck -->\n)", out): problems.append("raw HTML")
if out.count("<details>") != 1 or out.count("</details>") != 1: problems.append("details tags added")
if out.count("<!-- agent-countercheck -->") != 1 or not b.startswith("<!-- agent-countercheck -->\n"): problems.append("marker")
if "](javascript" in out: problems.append("a link")
if any(re.match("`{3,}", l) for l in lines[:o] + lines[c + 1:]): problems.append("a fence outside the output")
if not lines[1].startswith("### ❌ FAILED"): problems.append("verdict word not first")
if "https://github.com/acme/app/actions/runs/42" not in b: problems.append("run link")
print("\n".join(problems), file=sys.stderr); sys.exit(1 if problems else 0)
PY
# A fork's pull request: the token is read-only, so the post gets a 403. A warning, and nothing else changes.
seed_comments
FAKE_GH_FAIL=POST COMMENT=true run fork failed 1 unverified tamper
check "a 403 is a warning in the log" 'grep -q "::warning title=Countercheck::the PR comment was not posted: HTTP 403" "$W/fork.log"'
check "and in the job summary" 'grep -q "The PR comment was not posted" "$W/fork.md"'
FAKE_GH_FAIL=GET COMMENT=true run fork-good verified 0 unverified good
FAKE_GH_FAIL=POST COMMENT=true run fork-never failed 0 never tamper
check "a failed listing is a warning too" 'grep -q "could not list the PR" "$W/fork-good.log"'
seed_comments
PRN="" COMMENT=true run push verified 0 unverified good
check "not a pull request: no API call" '[ ! -s "$FAKE_GH_LOG" ]'
COMMENT=yes run badvalue verified 0 unverified good
check "comment=yes: a warning, no API call" 'grep -q "must be true or false" "$W/badvalue.log" && [ ! -s "$FAKE_GH_LOG" ]'
unset TITLE
# The body on every verdict, built from the receipts above.
body() { "$py" "$ROOT/packaging/github-action/comment.py" body "$@" | sed -n 2p; }
check "VERIFIED body" '[ "$(body "$(ls "$W"/good.tmp/countercheck-receipts/*.json)" verified "")" = "### ✅ VERIFIED · Agent Countercheck" ]'
check "FAILED body" 'body "$(ls "$W"/tamper.tmp/countercheck-receipts/*.json)" failed "" | grep -q "^### ❌ FAILED"'
check "UNVERIFIED body" 'body "$(ls "$W"/unpinned.tmp/countercheck-receipts/*.json)" unverified "" | grep -q "^### ⚠️ UNVERIFIED"'
# The headline follows countercheck's exit code (the step's verdict), not a receipt file that says otherwise.
check "a receipt that disagrees with the exit code can't soften the comment" 'b=$("$py" "$ROOT/packaging/github-action/comment.py" body "$(ls "$W"/good.tmp/countercheck-receipts/*.json)" failed ""); echo "$b" | grep -q "^### ❌ FAILED" && echo "$b" | grep -q "doesn.t match" && ! echo "$b" | grep -q VERIFIED'
check "ERROR body with the reason" '"$py" "$ROOT/packaging/github-action/comment.py" body "" error "$W/nochecks.tmp/countercheck-stderr.txt" | grep -q "no .countercheck/checks.toml found"'
# A huge hostile receipt stays within 60 lines and 60 KB, with the failure listed.
"$py" - "$W/big.json" <<'PY'
import json, sys
c = lambda i, ok: {"name": "@all " + "<b>`x`</b>" * 100 + str(i), "passed": ok, "duration_ms": 5, "exit_code": 1,
                   "output_tail": ("```\n" + "@here " * 500 + "\n") * 300}
json.dump({"id": "abcdef0123456789", "verdict": "failed", "claim": "😀" * 5000, "claim_status": "contradicted",
           "reasons": ["r" * 9000] * 50, "checks": [c(i, True) for i in range(99)] + [c(99, False)],
           "protected_changes": [{"path": "t" * 5000, "change": "modified", "action": "restored_from_base"}] * 30},
          open(sys.argv[1], "w"))
PY
"$py" "$ROOT/packaging/github-action/comment.py" body "$W/big.json" failed "" > "$W/big.md"
check "a huge receipt: at most 60 lines and 60 KB" '[ "$(wc -l < "$W/big.md")" -le 60 ] && [ "$(wc -c < "$W/big.md")" -le 60000 ] && grep -q "FAIL (exit 1): @" "$W/big.md"'
echo "--- the PR comment of the hostile case ---"; cat "$W/comment.md"
echo "--- summary of the tamper case ---"; cat "$W/tamper.md"
exit $fail
