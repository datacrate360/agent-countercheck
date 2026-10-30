"""Builds docs/img/architecture.svg (run: python3 build_architecture.py <repo>/docs/img/architecture.svg)."""
import sys
from html import escape

W, H = 960, 760
TEAL, TEAL_D, CORAL, INK, MUTED, LINE, SOFT = "#0F4C4C", "#0B3A3A", "#C2410C", "#141414", "#6B6B66", "#E7E5E0", "#F4F3F0"
MONO = "'JetBrains Mono', ui-monospace, SFMono-Regular, Menlo, Consolas, monospace"
o = []


def t(x, y, s, size=12, w=400, fill=INK, anchor="start", mono=False, ls=None):
    f = f' font-family="{MONO}"' if mono else ""
    l = f' letter-spacing="{ls}"' if ls else ""
    o.append(f'<text x="{x}" y="{y}" font-size="{size}" font-weight="{w}" fill="{fill}" text-anchor="{anchor}"{f}{l}>{escape(s)}</text>')


def box(x, y, w, h, fill="#FFFFFF", stroke=LINE, rx=10, sw=1.2, dash=None):
    d = f' stroke-dasharray="{dash}"' if dash else ""
    o.append(f'<rect x="{x}" y="{y}" width="{w}" height="{h}" rx="{rx}" fill="{fill}" stroke="{stroke}" stroke-width="{sw}"{d}/>')


def arrow(d, color="#55554F", marker="a", dash=None, sw=1.5):
    ds = f' stroke-dasharray="{dash}"' if dash else ""
    o.append(f'<path d="{d}" fill="none" stroke="{color}" stroke-width="{sw}"{ds} marker-end="url(#{marker})"/>')


def head(x, s):
    t(x, 36, s, 12, 700, MUTED, "middle", ls=1)


# Columns
AX, AW = 24, 206          # who asks
BX, BW = 262, 404         # countercheck verify
CX, CW = 704, 232         # where it goes
head(AX + AW / 2, "WHO ASKS")
head(BX + BW / 2, "WHAT ONE RUN DOES")
head(CX + CW / 2, "WHERE IT GOES")

# --- who asks
callers = [
    (["Claude Code · Cortex Code"], ["hook session-start", "hook claude-stop"], "set up by hook install"),
    (["Cursor · Codex · Copilot", "Goose · Kiro"], ["hook session-start", "hook <agent>-stop"], "set up by hook install"),
    (["Hermes Agent"], ["hook hermes-pre-verify"], "runs before the agent may finish"),
    (["Any agent, a script, you"], ["countercheck verify"], "agent named from its shell or --agent"),
    (["CI · GitHub Action"], ["verify --base origin/main"], "on every pull request"),
]
y = 56
caller_mid = []
for titles, cmds, note in callers:
    extra = 17 * (len(titles) - 1)
    h = 40 + extra + 17 * len(cmds) + 16
    box(AX, y, AW, h)
    for i, title in enumerate(titles):
        t(AX + 14, y + 22 + 17 * i, title, 13, 650)
    for i, c in enumerate(cmds):
        t(AX + 14, y + 42 + extra + 17 * i, c, 11, 500, TEAL, mono=True)
    t(AX + 14, y + h - 12, note, 10.5, 400, MUTED)
    caller_mid.append(y + h / 2)
    y += h + 14
callers_bottom = y - 14

# --- the repo (input to the run)
RY = 56
box(BX, RY, BW, 74, SOFT, LINE)
t(BX + 16, RY + 22, "Your git repository", 13, 650)
t(BX + 16, RY + 42, "base commit (before the agent):", 11, 500, MUTED)
t(BX + 196, RY + 42, ".countercheck/checks.toml · tests", 11, 500, TEAL, mono=False)
t(BX + 16, RY + 60, "working tree (now):", 11, 500, MUTED)
t(BX + 196, RY + 60, "the agent's files, committed or not", 11, 500, INK)

# --- the run
PY = RY + 74 + 26
PH = 62 + 4 * (64 + 8) + 92 + 14
box(BX, PY, BW, PH, TEAL, TEAL, 14, 0)
t(BX + BW / 2, PY + 28, "countercheck verify", 16, 700, "#FFFFFF", "middle")
t(BX + BW / 2, PY + 46, "the worker never grades itself", 11, 400, "#CFE3E0", "middle")
steps = [
    ("1", "Lock the checks", "git.rs · config", "read checks.toml and the protected list from the base commit"),
    ("2", "Clean copy", "snapshot.rs · protect", "copy the working tree to a new folder; put back every protected file from base (Git LFS too)"),
    ("3", "Run each check in a sandbox", "runner.rs", "macOS Seatbelt · Linux Landlock + namespaces · Windows restricted token + job"),
    ("4", "Decide", "receipt · claim", "VERIFIED only if every locked check ran and passed; does the claim match?"),
    ("5", "Seal", "receipt", "SHA-256 over the whole receipt; any later edit shows"),
]
sy = PY + 62
step_y = []
for n, title, mod, desc in steps:
    sh = 64 if n != "3" else 92
    box(BX + 14, sy, BW - 28, sh, "#FFFFFF", "#FFFFFF", 9, 0)
    t(BX + 30, sy + 22, n, 13, 700, TEAL)
    t(BX + 48, sy + 22, title, 13, 650)
    t(BX + BW - 28, sy + 22, mod, 10.5, 500, MUTED, "end", mono=True)
    words, line, lines = desc.split(), "", []
    for w_ in words:
        if len(line) + len(w_) + 1 > 58:
            lines.append(line)
            line = w_
        else:
            line = (line + " " + w_).strip()
    lines.append(line)
    for i, l in enumerate(lines):
        t(BX + 48, sy + 40 + 15 * i, l, 11, 400, "#3A3A36")
    if n == "3":
        types = ["command", "matrix", "forbid", "golden", "budget", "coverage"]
        px = BX + 48
        for ty in types:
            wpx = 10 + len(ty) * 5.9
            box(px, sy + 64, wpx, 18, "#E6F0EE", "#E6F0EE", 9, 0)
            t(px + wpx / 2, sy + 77, ty, 10, 600, TEAL, "middle")
            px += wpx + 5
    step_y.append(sy + sh / 2)
    sy += sh + 8
pipe_bottom = PY + PH

# repo -> run
arrow(f"M{BX + BW / 2} {RY + 74} V{PY - 2}")
# callers -> run
for m in caller_mid:
    arrow(f"M{AX + AW} {m} C{AX + AW + 22} {m}, {BX - 22} {PY + 40}, {BX - 2} {PY + 40}")

# progress side note (while it runs)
t(BX + BW / 2, pipe_bottom + 18, "while it runs, receipts/running/<id>.json says which step (for the page, never a verdict)", 10.5, 400, MUTED, "middle")

# --- where it goes
outs = [
    ("Sealed receipt", ".countercheck/receipts/<id>.json", "claim, each check's output, protected files touched, verdict", TEAL),
    ("Back to the agent", "hook: block or let it stop", "FAILED + “done”: the failing output goes back (at most 3 times)", CORAL),
    ("Exit code", "0 ok · 2 failed · 3 unverified", "CI fails the job on anything but 0", INK),
]
y = 56 + 74 + 26
out_mid = []
for title, sub, note, col in outs:
    h = 86
    box(CX, y, CW, h, "#FFFFFF", col if col != INK else LINE, 10, 1.4)
    t(CX + 14, y + 22, title, 13, 650, col)
    t(CX + 14, y + 41, sub, 10, 500, INK, mono=True)
    words, line, lines = note.split(), "", []
    for w_ in words:
        if len(line) + len(w_) + 1 > 36:
            lines.append(line)
            line = w_
        else:
            line = (line + " " + w_).strip()
    lines.append(line)
    for i, l in enumerate(lines):
        t(CX + 14, y + 59 + 14 * i, l, 10.5, 400, MUTED)
    out_mid.append(y + h / 2)
    y += h + 14
for i, m in enumerate(out_mid):
    color = CORAL if i == 1 else "#55554F"
    arrow(f"M{BX + BW} {m} H{CX - 2}", color, "ac" if i == 1 else "a")

# readers
ry = y
box(CX, ry, CW, 118, SOFT, LINE)
t(CX + 14, ry + 22, "Read the receipts", 13, 650)
for i, (c, d) in enumerate([("show", "one receipt + seal check"), ("report", "offline HTML, no scripts"), ("ui", "live page on 127.0.0.1"), ("prune", "delete old ones, on request")]):
    t(CX + 14, ry + 44 + 18 * i, c, 11, 600, TEAL, mono=True)
    t(CX + 66, ry + 44 + 18 * i, d, 10.5, 400, MUTED)

# the loop back to the agent
lb = pipe_bottom + 40
gx = BX + BW + 19
o.append(f'<path d="M{CX - 2} {out_mid[1] + 24} H{gx} V{lb} H{AX + AW / 2} V{callers_bottom + 6}" fill="none" stroke="{CORAL}" stroke-width="1.5" stroke-dasharray="5 4" marker-end="url(#ac)"/>')
t(BX + BW / 2, lb - 7, "the agent fixes the code (not the tests) and says “done” again", 11, 600, CORAL, "middle")

# --- code map
CY = H - 74
o.append(f'<line x1="24" y1="{CY - 16}" x2="{W - 24}" y2="{CY - 16}" stroke="{LINE}"/>')
t(24, CY, "IN THE CODE", 11, 700, MUTED, ls=1)
crates = [
    ("crates/countercheck-core", "pure logic, no I/O: config, protect, receipt, claim, report, progress, the six check types"),
    ("crates/countercheck-winsandbox", "the Windows sandbox's Win32 calls; the only crate allowed unsafe"),
    ("apps/countercheck", "the binary: git, snapshot, runner, Linux/Windows sandboxes, hooks, report, ui, prune"),
]
for i, (c, d) in enumerate(crates):
    t(24, CY + 22 + 18 * i, c, 11, 600, TEAL, mono=True)
    t(250, CY + 22 + 18 * i, d, 11, 400, "#3A3A36")

svg = f'''<svg xmlns="http://www.w3.org/2000/svg" width="{W}" height="{H}" viewBox="0 0 {W} {H}" font-family="Inter, -apple-system, BlinkMacSystemFont, 'Segoe UI', Helvetica, Arial, sans-serif">
  <title>Countercheck architecture</title>
  <desc>Who asks for a check: Claude Code and Cortex Code, and Cursor, Codex, GitHub Copilot, Goose and Kiro, through the hooks countercheck hook install sets up, Hermes Agent through its pre-verify hook, any agent, script or person through countercheck verify, and CI or the GitHub Action. One run reads the base commit and the working tree of your git repository, then: 1 locks the checks from the base commit, 2 makes a clean copy with every protected file put back, 3 runs each check in a sandbox (macOS Seatbelt, Linux Landlock and namespaces, Windows restricted token and job object) with six check types, 4 decides the verdict, 5 seals the receipt with SHA-256. The results go to a sealed receipt file, back to the agent through the hook, and to CI as an exit code. Receipts are read with show, report, ui and pruned with prune. If the work failed, the agent fixes the code and says done again. In the code: countercheck-core is pure logic, countercheck-winsandbox holds the Windows calls, apps/countercheck is the binary.</desc>
  <defs>
    <marker id="a" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="7" markerHeight="7" orient="auto-start-reverse"><path d="M0,0 L10,5 L0,10 z" fill="#55554F"/></marker>
    <marker id="ac" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="7" markerHeight="7" orient="auto-start-reverse"><path d="M0,0 L10,5 L0,10 z" fill="{CORAL}"/></marker>
  </defs>
  <rect width="{W}" height="{H}" rx="16" fill="#FFFFFF"/>
  {chr(10).join("  " + x for x in o).strip()}
</svg>
'''
open(sys.argv[1], "w").write(svg)
print("ok", len(svg))
