#!/usr/bin/env bash
# The GitHub Action's verify step (../../action.yml). Inputs arrive as IN_* environment variables.
# Exit: 0 when the verdict passes `fail-on`, 1 otherwise. Writes the step outputs and the job summary.
set -u

out() { echo "$1=$2" >> "$GITHUB_OUTPUT"; }
note() { echo "::notice title=Countercheck::$1"; }

cd "$IN_PATH" || { echo "::error title=Countercheck::path not found: $IN_PATH"; out verdict error; exit 1; }

# The base revision: the input, else the pull request's base, else the commit before a push.
base="$IN_BASE"
if [ -z "$base" ] && [ -n "${PR_BASE:-}" ]; then base="$PR_BASE"; fi
if [ -z "$base" ] && [ -n "${PUSH_BEFORE:-}" ] && [ "$PUSH_BEFORE" != "0000000000000000000000000000000000000000" ]; then
  base="$PUSH_BEFORE"
fi
if [ -z "$base" ]; then
  base="HEAD~1"
  note "no pull request or push base found; using HEAD~1. Set the 'base' input to choose."
fi

# actions/checkout fetches one commit by default: fetch the base if it isn't here.
if ! git cat-file -e "$base^{commit}" 2>/dev/null; then
  git fetch --quiet --no-tags --depth=1 origin "$base" 2>/dev/null \
    || git fetch --quiet --no-tags --unshallow origin 2>/dev/null \
    || true
fi
if ! git cat-file -e "$base^{commit}" 2>/dev/null; then
  echo "::error title=Countercheck::base revision $base is not in this checkout; use actions/checkout with fetch-depth: 0"
  out verdict error
  exit 1
fi

claim="$IN_CLAIM"
if [ -z "$claim" ] && [ -n "${PR_TITLE:-}" ]; then claim="$PR_TITLE"; fi

receipts="${RUNNER_TEMP:-/tmp}/countercheck-receipts"
mkdir -p "$receipts"
args=(verify --base "$base" --sandbox "$IN_SANDBOX" --json --out "$receipts")
[ -n "$claim" ] && args+=(--claim "$claim")
[ -n "$IN_AGENT" ] && args+=(--agent "$IN_AGENT")
[ -n "$IN_MODEL" ] && args+=(--model "$IN_MODEL")
[ -n "$IN_CHECKS_FROM" ] && args+=(--checks-from "$IN_CHECKS_FROM")
[ -n "$IN_CHECKS_SHA" ] && args+=(--checks-sha256 "$IN_CHECKS_SHA")

json="${RUNNER_TEMP:-/tmp}/countercheck-latest.json"
errlog="${RUNNER_TEMP:-/tmp}/countercheck-stderr.txt"
countercheck "${args[@]}" > "$json" 2> "$errlog"
code=$?
cat "$errlog" >&2
case "$code" in
  0) verdict=verified ;;
  2) verdict=failed ;;
  3) verdict=unverified ;;
  *) verdict=error ;;
esac
out verdict "$verdict"
out receipt-dir "$receipts"
if [ "$verdict" = error ]; then
  msg="$(tr '\n' ' ' < "$errlog" | cut -c1-500)"
  echo "::error title=Countercheck could not run::${msg:-exit code $code}"
  { echo "## ❔ Countercheck: ERROR"; echo; echo "Countercheck could not run (exit $code):"; echo; echo '```'
    cat "$errlog"; echo '```'; } >> "$GITHUB_STEP_SUMMARY"
fi

py=""
for c in python3 python; do "$c" -c 1 >/dev/null 2>&1 && { py="$c"; break; }; done
if [ "$verdict" != error ] && [ -n "$py" ]; then
  # The summary: verdict, reasons, each check, protected changes. Reads the JSON receipt only.
  "$py" - "$json" "$receipts" <<'PY' >> "$GITHUB_STEP_SUMMARY"
import json, sys
d = json.load(open(sys.argv[1], encoding="utf-8"))
receipt = f"{sys.argv[2]}/{d['id']}.json"
with open(__import__("os").environ["GITHUB_OUTPUT"], "a", encoding="utf-8") as f:
    f.write(f"receipt-id={d['id']}\nreceipt-path={receipt}\n")
icon = {"verified": "✅", "failed": "❌", "unverified": "⚠️"}.get(d["verdict"], "❔")
def cell(s):
    return str(s).replace("|", "\\|").replace("\n", " ")
print(f"## {icon} Countercheck: {d['verdict'].upper()}\n")
for r in d.get("reasons", []):
    print(f"- {cell(r)}")
print(f"\n**Checks:** `{cell(d.get('checks_source', ''))}` · **Sandbox:** {cell(d.get('sandbox', ''))}\n")
if d.get("checks"):
    print("| Check | Result | Time |\n|---|---|---|")
    for c in d["checks"]:
        res = "pass" if c.get("passed") else ("timed out" if c.get("timed_out") else f"fail (exit {c.get('exit_code')})")
        print(f"| {cell(c.get('name', ''))} | {res} | {c.get('duration_ms', 0)} ms |")
pc = d.get("protected_changes") or []
if pc:
    print("\n**Protected files the change touched** (put back from the base for the checks):\n")
    for p in pc:
        print(f"- `{cell(p.get('path', ''))}`: {cell(p.get('change', ''))}, {cell(p.get('action', '')).replace('_', ' ')}")
failing = [c for c in d.get("checks", []) if not c.get("passed") and c.get("output_tail")]
for c in failing[:3]:
    print(f"\n<details><summary>Output of {cell(c['name'])}</summary>\n\n```\n{c['output_tail'][-3000:]}\n```\n</details>")
print(f"\nReceipt `{d['id']}` (sealed; the full receipt and an HTML report are in the workflow artifact) · countercheck {d.get('countercheck_version', '')}")
PY
  countercheck report --receipts "$receipts" >/dev/null 2>&1 || true
fi

echo "Countercheck verdict: $verdict"
case "$IN_FAIL_ON" in
  never) exit 0 ;;
  failed) [ "$verdict" = failed ] || [ "$verdict" = error ] && exit 1; exit 0 ;;
  *) [ "$verdict" = verified ] && exit 0; exit 1 ;;
esac
