"""Builds docs/img/a-run.svg: one run, animated (CSS only, loops). python3 build_run.py <out.svg>

Phase 1: the agent skipped a failing test and says done -> locked, clean copy, test put back, checks fail,
sealed FAILED, sent back. Phase 2: it fixes the code -> all checks pass -> sealed VERIFIED -> delivered.
"""
import sys
from html import escape

T = 18.0  # seconds per loop
W, H = 960, 470
NIGHT, GRID, PANEL, LINE = "#0E1211", "#1A2322", "#151C1B", "#26302E"
TEAL, TEAL_L, CORAL, CORAL_L = "#0F4C4C", "#5FB3A9", "#C2410C", "#F08A5D"
TXT, MUTED = "#E9EEEC", "#8E9A97"
MONO = "'JetBrains Mono', ui-monospace, SFMono-Regular, Menlo, Consolas, monospace"

css, body = [], []
n = [0]


def pct(s):
    return f"{max(0.0, min(100.0, s / T * 100)):.2f}%"


def anim(points, prop="opacity"):
    """A keyframe animation through (time, value) points; returns the class name."""
    n[0] += 1
    name = f"k{n[0]}"
    frames = " ".join(f"{pct(t)}{{{prop}:{v}}}" for t, v in points)
    css.append(f"@keyframes {name}{{{frames}}}.{name}{{animation:{name} {T}s linear infinite;{prop}:{points[0][1]}}}")
    return name


def vis(*intervals, fade=0.25):
    pts = [(0, 0)]
    for a, b in intervals:
        pts += [(max(0, a - fade), 0), (a, 1), (b, 1), (b + fade, 0)]
    pts.append((T, 0))
    return anim(pts)


def g(cls, inner):
    body.append(f'<g class="{cls}">{inner}</g>')


def text(x, y, s, size=13, w=400, fill=TXT, anchor="start", mono=False, italic=False):
    f = f' font-family="{MONO}"' if mono else ""
    it = ' font-style="italic"' if italic else ""
    return f'<text x="{x}" y="{y}" font-size="{size}" font-weight="{w}" fill="{fill}" text-anchor="{anchor}"{f}{it}>{escape(s)}</text>'


def rect(x, y, w, h, fill, stroke="none", rx=10, sw=1):
    return f'<rect x="{x}" y="{y}" width="{w}" height="{h}" rx="{rx}" fill="{fill}" stroke="{stroke}" stroke-width="{sw}"/>'


def ok(x, y):
    return f'<circle cx="{x}" cy="{y}" r="8" fill="{TEAL_L}"/><path d="M{x-3.6} {y} l2.6 2.7 l5-5.4" fill="none" stroke="{NIGHT}" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"/>'


def bad(x, y):
    return f'<circle cx="{x}" cy="{y}" r="8" fill="{CORAL_L}"/><path d="M{x-3} {y-3} l6 6 M{x+3} {y-3} l-6 6" stroke="{NIGHT}" stroke-width="2" stroke-linecap="round"/>'


def warn(x, y):
    return f'<circle cx="{x}" cy="{y}" r="8" fill="none" stroke="{CORAL_L}" stroke-width="1.8"/><path d="M{x} {y-4} v4.5" stroke="{CORAL_L}" stroke-width="2" stroke-linecap="round"/><circle cx="{x}" cy="{y+3.6}" r="1.2" fill="{CORAL_L}"/>'


# ---- background: night band with a 44 px grid
body.append(rect(0, 0, W, H, NIGHT, rx=16))
body.append('<g stroke="%s" stroke-width="1">' % GRID + "".join(f'<path d="M{x} 0 V{H}"/>' for x in range(44, W, 44)) + "".join(f'<path d="M0 {y} H{W}"/>' for y in range(44, H, 44)) + "</g>")
body.append(text(32, 40, "ONE RUN", 11, 700, MUTED))
body.append(text(96, 40, "what happens when an agent says “done”", 13, 500, TXT))

# timeline
P1, P2 = 0.0, 9.0
S = [1.8, 2.6, 3.4, 4.2, 4.8, 5.4, 6.0]  # lock, copy, put back, check1..3, seal (phase 1 offsets)
END1, END2 = 8.6, 17.5

# ---- agent (always there)
AX, AY = 32, 150
body.append(rect(AX, AY, 210, 58, PANEL, LINE, 12))
body.append(f'<circle cx="{AX+28}" cy="{AY+29}" r="14" fill="#F2D9C7"/>' + text(AX + 28, AY + 34, "D", 13, 700, NIGHT, "middle"))
body.append(text(AX + 52, AY + 25, "Discount fix", 14, 650))
body.append(text(AX + 52, AY + 43, "claude-code · claude-opus-5", 11, 400, MUTED))
# the claim bubbles
for (a, b, line1, line2, note) in [
    (P1 + 0.2, END1, "“Fixed the discount bug.", "All tests pass.”", "(it skipped the failing test)"),
    (P2 + 0.2, END2, "“Fixed the discount bug.", "All tests pass.”", "(this time it fixed pricing.py)"),
]:
    g(vis((a, b)), rect(AX, AY + 72, 210, 74, "#1B2423", LINE, 12)
      + f'<path d="M{AX+30} {AY+72} l8 -9 l8 9" fill="#1B2423" stroke="{LINE}"/><path d="M{AX+31} {AY+73} h14" stroke="#1B2423" stroke-width="2"/>'
      + text(AX + 14, AY + 96, line1, 14, 400, TXT, italic=True) + text(AX + 14, AY + 115, line2, 14, 400, TXT, italic=True)
      + text(AX + 14, AY + 135, note, 11, 400, CORAL_L if "skipped" in note else TEAL_L))

# ---- edge agent -> countercheck (draws in each phase)
CX, CY, CW, CH = 284, 70, 356, 310
edge = f"M{AX+210} {AY+29} C{AX+250} {AY+29}, {CX-40} {CY+60}, {CX} {CY+60}"
for start, end in [(P1 + 1.0, END1), (P2 + 0.8, END2)]:
    cls = anim([(0, 300), (start, 300), (start + 0.7, 0), (end, 0), (end + 0.01, 300), (T, 300)], "stroke-dashoffset")
    body.append(f'<path class="{cls}" d="{edge}" fill="none" stroke="{TEAL_L}" stroke-width="2.2" stroke-dasharray="300" />')

# ---- countercheck panel
body.append(rect(CX, CY, CW, CH, PANEL, LINE, 14))
body.append(text(CX + 20, CY + 30, "countercheck verify", 15, 700))
rows = [
    ("Checks locked", "from the base commit"),
    ("Clean copy", "in a new folder"),
    ("Protected files", None),
    ("compiles", "check"),
    ("unit tests", "check"),
    ("full suite", "check"),
    ("Receipt sealed", "SHA-256"),
]
RY0, RH = CY + 50, 35
glow_at = len(body)
for i, (title, sub) in enumerate(rows):
    y = RY0 + i * RH
    is_check = sub == "check"
    body.append(f'<circle cx="{CX+30}" cy="{y+12}" r="8" fill="none" stroke="{LINE}" stroke-width="1.6"/>')
    if is_check:
        body.append(text(CX + 48, y + 16, title, 12.5, 500, TXT, mono=True))
    else:
        body.append(text(CX + 48, y + 16, title, 13, 600))
    if sub and not is_check:
        body.append(text(CX + CW - 20, y + 16, sub, 11, 400, MUTED, "end"))
    t1, t2 = S[i], S[i] - P1 + P2 + 0.0
    t2 = P2 + (S[i] - 0.6) * 0.95 + 0.6  # phase 2 runs a touch faster
    # the current step glows
    for t in (t1, t2):
        body.insert(glow_at, f'<g class="{vis((t, t + 0.7))}">' + rect(CX + 12, y - 6, CW - 24, 36, "#1F2A28", "none", 8) + "</g>")
        glow_at += 1
    # its result
    if i == 2:
        g(vis((t1, END1)), warn(CX + 30, y + 12) + text(CX + CW - 20, y + 16, "test edited → put back", 11, 600, CORAL_L, "end"))
        g(vis((t2, END2)), ok(CX + 30, y + 12) + text(CX + CW - 20, y + 16, "none changed", 11, 500, TEAL_L, "end"))
    elif i in (4, 5):
        g(vis((t1, END1)), bad(CX + 30, y + 12) + text(CX + CW - 20, y + 16, "FAIL · -5.0 != 0" if i == 4 else "FAIL", 11, 700, CORAL_L, "end", mono=True))
        g(vis((t2, END2)), ok(CX + 30, y + 12) + text(CX + CW - 20, y + 16, "pass", 11, 700, TEAL_L, "end", mono=True))
    elif i == 3:
        g(vis((t1, END1), (t2, END2)), ok(CX + 30, y + 12) + text(CX + CW - 20, y + 16, "pass", 11, 700, TEAL_L, "end", mono=True))
    else:
        g(vis((t1, END1), (t2, END2)), ok(CX + 30, y + 12))
SEAL1, SEAL2 = S[6], P2 + (S[6] - 0.6) * 0.95 + 0.6

# ---- receipt card and the outcome
RX, RYc, RW = 680, 70, 250
recp = f"M{CX+CW} {RY0 + 6*RH + 12} C{CX+CW+24} {RY0 + 6*RH + 12}, {RX-24} {RYc+60}, {RX} {RYc+60}"
for start, end, col in [(SEAL1 + 0.1, END1, CORAL_L), (SEAL2 + 0.1, END2, TEAL_L)]:
    cls = anim([(0, 300), (start, 300), (start + 0.5, 0), (end, 0), (end + 0.01, 300), (T, 300)], "stroke-dashoffset")
    body.append(f'<path class="{cls}" d="{recp}" fill="none" stroke="{col}" stroke-width="2.2" stroke-dasharray="300"/>')


def receipt(verdict, col, lines):
    s = rect(RX, RYc, RW, 150, "#F7F4EE", "none", 6)
    s += text(RX + 16, RYc + 26, "RECEIPT", 10, 700, "#6B6B66") + text(RX + 82, RYc + 26, "9073a384e88f9d37", 10, 500, "#6B6B66", mono=True)
    s += f'<path d="M{RX+16} {RYc+38} H{RX+RW-16}" stroke="#DDD8CE" stroke-dasharray="3 3"/>'
    for i, l in enumerate(lines):
        s += text(RX + 16, RYc + 58 + 18 * i, l, 11.5, 400, "#3A3A36")
    # the wax seal
    s += f'<circle cx="{RX+RW-46}" cy="{RYc+112}" r="28" fill="{col}"/><circle cx="{RX+RW-46}" cy="{RYc+112}" r="22" fill="none" stroke="#FFFFFF" stroke-opacity=".45" stroke-dasharray="2 3"/>'
    s += text(RX + RW - 46, RYc + 116, verdict, 9.5 if len(verdict) > 6 else 11, 800, "#FFFFFF", "middle")
    return s


g(vis((SEAL1 + 0.5, END1)), receipt("FAILED", CORAL, ["claim: CONTRADICTED", "1 protected file put back", "2 of 3 checks failed"]))
g(vis((SEAL2 + 0.5, END2)), receipt("VERIFIED", TEAL, ["claim: matches the evidence", "no protected file changed", "3 of 3 checks passed"]))

# phase 1: sent back to the agent (coral loop under everything)
back = f"M{RX+40} {RYc+150} V{H-60} Q{RX+40} {H-46}, {RX+26} {H-46} H{AX+119} Q{AX+105} {H-46}, {AX+105} {H-60} V{AY+150}"
g(vis((SEAL1 + 1.0, END1)), f'<path d="{back}" fill="none" stroke="{CORAL_L}" stroke-width="2" stroke-dasharray="6 5"/>'
  + text(RX + 128, RYc + 186, "SENT BACK", 12, 800, CORAL_L, "middle")
  + text(RX + 128, RYc + 204, "“fix the code, not the tests”", 11.5, 400, TXT, "middle", italic=True)
  + text(RX + 128, RYc + 222, "with the real failure", 11, 400, MUTED, "middle"))
# phase 2: delivered
g(vis((SEAL2 + 1.0, END2)), f'<circle cx="{RX+28}" cy="{RYc+196}" r="12" fill="{TEAL_L}"/>'
  + f'<path d="M{RX+22} {RYc+196} l4 4 l8 -8" fill="none" stroke="{NIGHT}" stroke-width="2.4" stroke-linecap="round" stroke-linejoin="round"/>'
  + text(RX + 50, RYc + 201, "DELIVERED", 14, 800, TEAL_L)
  + text(RX + 50, RYc + 222, "proven on rules it couldn't change", 11.5, 400, TXT))

# ---- caption: what is happening right now
caps = [
    (0.0, 1.0, "The agent says it is done."),
    (1.0, S[0], "Countercheck takes over: it doesn't believe the claim, it re-runs the checks."),
    (S[0], S[1], "1 · The checks are read from the commit before the agent started: it can't weaken them."),
    (S[1], S[2], "2 · The agent's files are copied into a clean folder."),
    (S[2], S[3], "3 · The test the agent edited is put back from the base commit."),
    (S[3], S[6], "4 · Every check runs again, in a sandbox (no internet, writes only in the copy)."),
    (S[6], SEAL1 + 1.0, "5 · The verdict and all the evidence are sealed into a receipt."),
    (SEAL1 + 1.0, END1, "FAILED: the agent can't finish; it gets the real failure back."),
    (P2, P2 + 1.6, "The agent fixes the code this time, and says done again."),
    (P2 + 1.6, SEAL2, "Same rules, same checks, a clean copy again: all three pass."),
    (SEAL2, SEAL2 + 1.0, "The verdict is sealed: a later edit to the receipt would show."),
    (SEAL2 + 1.0, END2, "VERIFIED: the work is delivered, with a receipt anyone can re-check."),
]
for a, b, s in caps:
    col = CORAL_L if s.startswith("FAILED") else (TEAL_L if s.startswith("VERIFIED") else TXT)
    # Fade out before the next one fades in: two captions never overlap.
    g(vis((a + 0.15, b - 0.15), fade=0.12), text(W / 2, H - 14, s, 13.5, 500, col, "middle"))

svg = f'''<svg xmlns="http://www.w3.org/2000/svg" width="{W}" height="{H}" viewBox="0 0 {W} {H}" font-family="Inter, -apple-system, BlinkMacSystemFont, 'Segoe UI', Helvetica, Arial, sans-serif">
<title>One run of Countercheck, animated</title>
<desc>An agent called Discount fix (claude-code) says "Fixed the discount bug. All tests pass." but it skipped the failing test. Countercheck locks the checks from the base commit, makes a clean copy, puts the edited test back, and runs the checks in a sandbox: compiles passes, unit tests and full suite fail (-5.0 != 0). The receipt is sealed FAILED and the agent is sent back: fix the code, not the tests. The agent then fixes pricing.py and says done again; this time no protected file changed, all three checks pass, the receipt is sealed VERIFIED, and the work is delivered.</desc>
<style>{"".join(css)}@media (prefers-reduced-motion:reduce){{*{{animation-duration:0s!important;animation-delay:0s!important}}}}</style>
{chr(10).join(body)}
</svg>
'''
open(sys.argv[1], "w").write(svg)
print("ok", len(svg))
