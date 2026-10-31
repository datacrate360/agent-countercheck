"""The pull-request comment of the GitHub Action (../../action.yml, input `comment`).

  comment.py body <receipt.json or ""> <verdict> <stderr file>   the comment's markdown, on stdout
  comment.py payload <body.md>                                    {"body": ...} for the REST API, on stdout
  comment.py find                                                 our comment's id, from the listed comments on stdin

Everything from the pull request (claim, check names, output, reasons, paths) is hostile text: inline
text is escaped so it can't add markdown, HTML or @mentions; check output goes in a code fence longer
than any backtick run inside it. The body is capped at MAX_LINES lines and MAX_BYTES bytes.
"""
import json
import os
import re
import sys

MARKER = "<!-- agent-countercheck -->"
# Only our own comments are updated: the job token posts as this account.
BOT = "github-actions[bot]"
MAX_LINES = 60
MAX_BYTES = 60_000
MAX_INLINE = 200
MAX_CHECKS = 15
MAX_REASONS = 8
MAX_PROTECTED = 5
MAX_OUTPUT_LINES = 12
ZWSP = "​"


def inline(s, limit=MAX_INLINE):
    """Hostile text as one line of plain markdown: no HTML, no formatting, no @mentions."""
    s = re.sub(r"[\x00-\x1f\x7f  ]+", " ", str(s)).strip()
    if len(s) > limit:
        s = s[: limit - 1] + "…"
    s = s.replace("&", "&amp;").replace("<", "&lt;").replace(">", "&gt;")
    s = re.sub(r"([\\`*_\[\]()#|~!{}+\-.:=])", r"\\\1", s)
    # A zero-width space after @ stops GitHub from pinging a user or team.
    return s.replace("@", "@" + ZWSP)


def fenced(text):
    """Hostile text as a code block: the fence is longer than any backtick run inside it."""
    text = re.sub(r"[\x00-\x08\x0b-\x1f\x7f]", "", text)
    lines = [l[:MAX_INLINE] for l in text.splitlines()][-MAX_OUTPUT_LINES:]
    body = "\n".join(lines)
    longest = max((len(m) for m in re.findall(r"`+", body)), default=0)
    fence = "`" * max(3, longest + 1)
    return [fence + "text", *lines, fence]


def run_link():
    server, repo, run = (os.environ.get(k, "") for k in ("GITHUB_SERVER_URL", "GITHUB_REPOSITORY", "GITHUB_RUN_ID"))
    ok = re.fullmatch(r"https://[A-Za-z0-9.\-]+(:[0-9]+)?", server) and re.fullmatch(r"[\w.\-]+/[\w.\-]+", repo)
    if ok and re.fullmatch(r"[0-9]+", run):
        return f"{server}/{repo}/actions/runs/{run}"
    return ""


def check_line(c):
    if c.get("passed"):
        res = "✅ pass"
    elif c.get("timed_out"):
        res = "❌ FAIL (timed out)"
    else:
        code = c.get("exit_code")
        res = "❌ FAIL" + (f" (exit {int(code)})" if isinstance(code, int) else "")
    ms = c.get("duration_ms", 0)
    ms = ms if isinstance(ms, (int, float)) else 0
    return f"- {res}: {inline(c.get('name', ''))} ({ms / 1000:.1f} s)"


CLAIM = {
    "supported": "supported by the checks",
    "contradicted": "**CONTRADICTED** by the checks",
    "not_checked": "not proven",
}
ICON = {"verified": "✅", "failed": "❌", "unverified": "⚠️"}


def body(receipt_path, verdict, errlog):
    """The comment; built again with shorter fields in the rare case it is over MAX_BYTES."""
    global MAX_INLINE
    for MAX_INLINE in (200, 40):
        text = _body(receipt_path, verdict, errlog)
        if len(text.encode("utf-8")) <= MAX_BYTES:
            break
    return text


def _body(receipt_path, verdict, errlog):
    out = [MARKER]
    d = {}
    if receipt_path:
        try:
            with open(receipt_path, encoding="utf-8") as f:
                d = json.load(f)
        except (OSError, ValueError):
            d = {}
    step = verdict
    if not isinstance(d, dict) or not d.get("verdict"):
        verdict = "error"
    else:
        verdict = str(d["verdict"])
    # The step's verdict comes from countercheck's exit code, which fail-on also uses; the JSON is a file
    # in the runner's temp folder. If they disagree, the exit code wins and no detail from the file is shown.
    if step in ICON and verdict != step:
        out.append(f"### {ICON[step]} {step.upper()} · Agent Countercheck")
        out.append("")
        out.append("The receipt file doesn't match countercheck's result, so no details are shown here; see the "
                   "job log and the uploaded receipt.")
        return "\n".join(out) + "\n"
    word = verdict.upper() if verdict in ICON else "ERROR"
    out.append(f"### {ICON.get(verdict, '❔')} {word} · Agent Countercheck")
    out.append("")
    tail = []
    if word == "ERROR":
        out.append("Countercheck could not run, so this change is not verified.")
        try:
            with open(errlog, encoding="utf-8", errors="replace") as f:
                err = f.read()
        except OSError:
            err = ""
        if err.strip():
            out += ["", *fenced(err)]
    else:
        claim = d.get("claim")
        if isinstance(claim, str) and claim.strip():
            status = CLAIM.get(d.get("claim_status"), "not proven")
            out.append(f"**Claim:** {inline(claim)} — {status}")
            out.append("")
        reasons = [r for r in d.get("reasons") or [] if isinstance(r, str)]
        for r in reasons[:MAX_REASONS]:
            out.append(f"- {inline(r, MAX_INLINE * 3 // 2)}")
        if len(reasons) > MAX_REASONS:
            out.append(f"- …and {len(reasons) - MAX_REASONS} more (see the run)")
        checks = [c for c in d.get("checks") or [] if isinstance(c, dict)]
        if checks:
            out += ["", "**Checks**"]
            # Failures first, so a long list never hides them.
            shown = sorted(checks, key=lambda c: bool(c.get("passed")))[:MAX_CHECKS]
            out += [check_line(c) for c in shown]
            if len(checks) > MAX_CHECKS:
                out.append(f"- …and {len(checks) - MAX_CHECKS} more (see the run)")
        pc = [p for p in d.get("protected_changes") or [] if isinstance(p, dict)]
        if pc:
            out += ["", "**Protected files put back from the base** (the checks ran on the base's copy)"]
            for p in pc[:MAX_PROTECTED]:
                action = inline(str(p.get("action", "")).replace("_", " "))
                out.append(f"- {inline(p.get('path', ''))}: {inline(p.get('change', ''))}, {action}")
            if len(pc) > MAX_PROTECTED:
                out.append(f"- …and {len(pc) - MAX_PROTECTED} more (see the run)")
        failing = [c for c in checks if not c.get("passed") and isinstance(c.get("output_tail"), str)]
        failing = [c for c in failing if c["output_tail"].strip()]
        if failing:
            c = failing[0]
            tail = ["", f"<details><summary>Last lines of {inline(c.get('name', ''))}</summary>", "",
                    *fenced(c["output_tail"]), "</details>"]
    foot = [""]
    rid = d.get("id") if isinstance(d.get("id"), str) and re.fullmatch(r"[0-9a-f]{1,64}", d["id"]) else ""
    link = run_link()
    parts = [f"Receipt `{rid}` (sealed)" if rid else "", f"[run and full receipt]({link})" if link else ""]
    foot.append(" · ".join(p for p in parts if p) or "See the workflow run.")
    # Room for the output only if the whole body still fits the cap.
    if len(out) + len(tail) + len(foot) > MAX_LINES:
        tail = []
    return "\n".join(out + tail + foot) + "\n"


def find(stream):
    """The id of our comment: `gh api --paginate` prints one JSON array per page."""
    raw, dec, pos = stream.read(), json.JSONDecoder(), 0
    while pos < len(raw):
        while pos < len(raw) and raw[pos].isspace():
            pos += 1
        if pos >= len(raw):
            break
        page, pos = dec.raw_decode(raw, pos)
        for c in page if isinstance(page, list) else []:
            user = (c.get("user") or {}).get("login") if isinstance(c, dict) else None
            if user == BOT and str(c.get("body", "")).startswith(MARKER) and isinstance(c.get("id"), int):
                return str(c["id"])
    return ""


def main(argv):
    if len(argv) >= 4 and argv[0] == "body":
        sys.stdout.write(body(argv[1], argv[2], argv[3]))
    elif len(argv) == 2 and argv[0] == "payload":
        with open(argv[1], encoding="utf-8") as f:
            sys.stdout.write(json.dumps({"body": f.read()}))
    elif argv == ["find"]:
        print(find(sys.stdin))
    else:
        sys.stderr.write(__doc__)
        return 2
    return 0


if __name__ == "__main__":
    # The runner's console may not be UTF-8 (Windows); the body is.
    for s in (sys.stdout, sys.stdin):
        if hasattr(s, "reconfigure"):
            s.reconfigure(encoding="utf-8")
    sys.exit(main(sys.argv[1:]))
