#!/usr/bin/env bash
# The Action's pull-request comment (input `comment`): the step after verify.sh, the only one given the token
# (the verify step runs the PR's checks, so it never sees it). Inputs arrive as environment variables:
# IN_COMMENT, VERDICT (verify's output), PR_NUMBER, GITHUB_REPOSITORY, GH_TOKEN.
# Creates one comment on the pull request, or updates the one an earlier run made (found by the marker in
# comment.py, posted by github-actions[bot]). Any problem is a warning: this script always exits 0, so the
# comment never changes the verdict or the step's result.
set -u
here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
work="${RUNNER_TEMP:-/tmp}/countercheck-comment"
mkdir -p "$work"

warn() { # one line, so it can't add workflow commands
  local msg
  msg="$(printf '%s' "$1" | tr '\r\n' '  ' | cut -c1-300)"
  echo "::warning title=Countercheck::the PR comment was not posted: $msg"
  { echo; echo "> ⚠️ The PR comment was not posted: $msg"
    echo "> It needs \`permissions: pull-requests: write\`; on a pull request from a fork the token is read-only."
    echo "> The verdict above is unchanged."; } >> "${GITHUB_STEP_SUMMARY:-/dev/null}"
  exit 0
}

case "${IN_COMMENT:-false}" in
  false) exit 0 ;;
  true) ;;
  *) echo "::warning title=Countercheck::the comment input must be true or false; no PR comment"; exit 0 ;;
esac
# What verify.sh wrote (the same paths as there).
verdict="${VERDICT:-error}"
json="${RUNNER_TEMP:-/tmp}/countercheck-latest.json"
errlog="${RUNNER_TEMP:-/tmp}/countercheck-stderr.txt"
receipt=""; [ "$verdict" != error ] && [ -s "$json" ] && receipt="$json"
py=""
for c in python3 python; do "$c" -c 1 >/dev/null 2>&1 && { py="$c"; break; }; done

case "${PR_NUMBER:-}" in
  ''|*[!0-9]*) echo "::notice title=Countercheck::not a pull request, so no PR comment"; exit 0 ;;
esac
[ -n "$py" ] || warn "Python was not found (it builds the comment)"
command -v gh >/dev/null 2>&1 || warn "the GitHub CLI (gh) is not installed on this runner"
printf '%s' "${GITHUB_REPOSITORY:-}" | grep -Eq '^[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+$' || warn "GITHUB_REPOSITORY is not set"
[ -n "${GH_TOKEN:-}" ] || warn "no token (GH_TOKEN) was given"

"$py" "$here/comment.py" body "$receipt" "$verdict" "$errlog" > "$work/body.md" 2> "$work/err.txt" \
  && "$py" "$here/comment.py" payload "$work/body.md" > "$work/payload.json" 2>> "$work/err.txt" \
  || warn "could not build it: $(cat "$work/err.txt")"

api="repos/$GITHUB_REPOSITORY/issues"
gh api --paginate "$api/$PR_NUMBER/comments?per_page=100" > "$work/list.json" 2> "$work/err.txt" \
  || warn "could not list the PR's comments: $(cat "$work/err.txt")"
id="$("$py" "$here/comment.py" find < "$work/list.json" 2> "$work/err.txt")" \
  || warn "could not read the PR's comments: $(cat "$work/err.txt")"

if [ -n "$id" ]; then
  gh api --method PATCH "$api/comments/$id" --input "$work/payload.json" > /dev/null 2> "$work/err.txt" \
    || warn "$(cat "$work/err.txt")"
  echo "Countercheck: updated the PR comment ($id)"
else
  gh api --method POST "$api/$PR_NUMBER/comments" --input "$work/payload.json" > /dev/null 2> "$work/err.txt" \
    || warn "$(cat "$work/err.txt")"
  echo "Countercheck: posted the PR comment"
fi
exit 0
