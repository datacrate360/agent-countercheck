"""Build the Agent Countercheck logo set from one source geometry.

Outputs (in OUT): mark.svg (app icon, 1024), mark-small.svg (simplified for <= 48 px),
glyph.svg (no background tile), glyph-mono.svg, lockup-light.svg, lockup-dark.svg,
social-preview.svg (1280x640), readme-header.svg. Text is converted to outlines (Poppins,
OFL) so every SVG renders the same everywhere.
"""
import math
import sys
from pathlib import Path

from fontTools.pens.svgPathPen import SVGPathPen
from fontTools.pens.transformPen import TransformPen
from fontTools.ttLib import TTFont

OUT = Path(sys.argv[1])
FONTS = Path(sys.argv[2])
OUT.mkdir(parents=True, exist_ok=True)

TEAL, TEAL_D, TEAL_L, CORAL, INK = "#0F766E", "#083F3B", "#1BA393", "#C2410C", "#0F172A"

# ---------------------------------------------------------------- the mark
DEFS = f'''
<radialGradient id="bg" cx=".28" cy=".18" r="1.05"><stop offset="0" stop-color="{TEAL_L}"/><stop offset=".5" stop-color="{TEAL}"/><stop offset="1" stop-color="{TEAL_D}"/></radialGradient>
<linearGradient id="gloss" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#fff" stop-opacity=".16"/><stop offset=".45" stop-color="#fff" stop-opacity="0"/></linearGradient>
<radialGradient id="ghost" cx=".35" cy=".22" r=".95"><stop offset="0" stop-color="#FFFFFF"/><stop offset=".7" stop-color="#EEF4F5"/><stop offset="1" stop-color="#C9D8DB"/></radialGradient>
<radialGradient id="wax" cx=".34" cy=".3" r=".8"><stop offset="0" stop-color="#FDBA74"/><stop offset=".35" stop-color="#F97316"/><stop offset=".75" stop-color="{CORAL}"/><stop offset="1" stop-color="#8A2C0B"/></radialGradient>
<linearGradient id="paper" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#FFFFFF"/><stop offset="1" stop-color="#E7EEEC"/></linearGradient>
<filter id="sh" x="-30%" y="-30%" width="160%" height="170%"><feDropShadow dx="0" dy="24" stdDeviation="26" flood-color="#021F1D" flood-opacity=".45"/></filter>
<filter id="shs" x="-40%" y="-40%" width="180%" height="190%"><feDropShadow dx="0" dy="14" stdDeviation="14" flood-color="#3B1204" flood-opacity=".5"/></filter>
<filter id="soft"><feGaussianBlur stdDeviation="12"/></filter>'''


def ghost_path(cx, top, w, h, lobes=3):
    r = w / 2
    yb = top + h
    d = f"M{cx - r:.1f},{top + r:.1f} A{r:.1f},{r:.1f} 0 0 1 {cx + r:.1f},{top + r:.1f} V{yb:.1f}"
    lw = w / lobes
    x = cx + r
    for _ in range(lobes):
        nx = x - lw
        d += f" A{lw / 2:.1f},{lw / 2 * 0.85:.1f} 0 0 1 {nx:.1f},{yb:.1f}"
        x = nx
    return d + " Z"


def receipt_path(x0, x1, ytop, ybot, teeth=7, depth=30, r=26):
    w = (x1 - x0) / teeth
    zig = " ".join(
        f"L{x1 - (i + 0.5) * w:.1f},{ybot - depth} L{x1 - (i + 1) * w:.1f},{ybot}" for i in range(teeth)
    )
    return (
        f"M{x0 + r},{ytop} H{x1 - r} Q{x1},{ytop} {x1},{ytop + r} V{ybot} {zig} "
        f"V{ytop + r} Q{x0},{ytop} {x0 + r},{ytop} Z"
    )


def eyes(cx, cy, gap, rx, ry, shine=True):
    s = ""
    for dx in (-gap / 2, gap / 2):
        s += f'<ellipse cx="{cx + dx:.1f}" cy="{cy:.1f}" rx="{rx}" ry="{ry}" fill="{INK}"/>'
        if shine:
            s += f'<circle cx="{cx + dx - rx * 0.12:.1f}" cy="{cy - ry * 0.32:.1f}" r="{rx * 0.34:.1f}" fill="#fff"/>'
    return s


def tick(cx, cy, r):
    t = r * 0.42
    return f"{cx - t * 1.05:.1f},{cy + t * 0.05:.1f} {cx - t * 0.3:.1f},{cy + t * 0.8:.1f} {cx + t * 1.1:.1f},{cy - t * 0.65:.1f}"


def seal(cx, cy, r, w, detail=True):
    pts = tick(cx, cy, r)
    s = f'<g filter="url(#shs)"><circle cx="{cx}" cy="{cy}" r="{r}" fill="url(#wax)"/></g>'
    if detail:
        s += (
            f'<circle cx="{cx}" cy="{cy}" r="{r * 0.82:.1f}" fill="none" stroke="#fff" stroke-opacity=".28" stroke-width="{r * 0.03:.1f}"/>'
            f'<ellipse cx="{cx - r * 0.35:.1f}" cy="{cy - r * 0.48:.1f}" rx="{r * 0.42:.1f}" ry="{r * 0.24:.1f}" fill="#fff" '
            f'opacity=".28" filter="url(#soft)" transform="rotate(-28 {cx - r * 0.35:.1f} {cy - r * 0.48:.1f})"/>'
        )
    s += (
        f'<polyline points="{pts}" transform="translate(0 {r * 0.05:.1f})" fill="none" stroke="#6B1F08" stroke-opacity=".45" '
        f'stroke-width="{w}" stroke-linecap="round" stroke-linejoin="round"/>'
        f'<polyline points="{pts}" fill="none" stroke="#fff" stroke-width="{w}" stroke-linecap="round" stroke-linejoin="round"/>'
    )
    return s


def figure(small=False, shadows=True):
    """The ghost holding up its receipt, sealed. 1024 coordinate space."""
    sh = ' filter="url(#sh)"' if shadows else ""
    if small:
        # Fewer, bigger shapes: no text lines, a larger seal, bigger eyes.
        return (
            f'<g{sh}><path d="{ghost_path(512, 128, 440, 600)}" fill="url(#ghost)"/></g>'
            + eyes(512, 318, 150, 40, 54, shine=False)
            + f'<g{sh}><path d="{receipt_path(232, 792, 430, 892, teeth=5, depth=40)}" fill="url(#paper)"/></g>'
            + seal(640, 720, 176, 62, detail=False)
        )
    return (
        f'<g{sh}><path d="{ghost_path(512, 150, 400, 560)}" fill="url(#ghost)"/></g>'
        + f'<ellipse cx="452" cy="232" rx="70" ry="34" fill="#fff" opacity=".7" filter="url(#soft)" transform="rotate(-24 452 232)"/>'
        + eyes(512, 334, 132, 31, 43)
        + f'<g{sh}><path d="{receipt_path(250, 774, 420, 880)}" fill="url(#paper)"/></g>'
        + '<ellipse cx="338" cy="428" rx="44" ry="34" fill="url(#ghost)" stroke="#BFD0D3" stroke-width="3"/>'
        + '<ellipse cx="686" cy="428" rx="44" ry="34" fill="url(#ghost)" stroke="#BFD0D3" stroke-width="3"/>'
        + '<g stroke="#C9D6D3" stroke-width="24" stroke-linecap="round"><line x1="320" y1="532" x2="600" y2="532"/>'
        + '<line x1="320" y1="596" x2="520" y2="596"/></g>'
        + seal(652, 742, 130, 41)
    )


TILE = '<rect width="1024" height="1024" rx="228" fill="url(#bg)"/><rect width="1024" height="1024" rx="228" fill="url(#gloss)"/>'


def svg(w, h, body, view=None):
    v = view or f"0 0 {w} {h}"
    return f'<svg xmlns="http://www.w3.org/2000/svg" width="{w}" height="{h}" viewBox="{v}"><defs>{DEFS}</defs>{body}</svg>\n'


(OUT / "mark.svg").write_text(svg(1024, 1024, TILE + figure()))
(OUT / "mark-small.svg").write_text(svg(1024, 1024, TILE + figure(small=True)))
(OUT / "glyph.svg").write_text(svg(1024, 1024, figure()))

# Mono: one colour, for a terminal banner, a stamp, a watermark.
mono = (
    f'<path d="{ghost_path(512, 150, 400, 560)}" fill="{TEAL}"/>'
    + eyes(512, 334, 132, 31, 43, shine=False).replace(INK, "#fff")
    + f'<path d="{receipt_path(250, 774, 420, 880)}" fill="#fff" stroke="{TEAL}" stroke-width="26" stroke-linejoin="round"/>'
    + f'<g stroke="{TEAL}" stroke-width="22" stroke-linecap="round" opacity=".55"><line x1="320" y1="532" x2="600" y2="532"/><line x1="320" y1="596" x2="520" y2="596"/></g>'
    + f'<circle cx="652" cy="742" r="138" fill="#fff"/><circle cx="652" cy="742" r="124" fill="{TEAL}"/>'
    + f'<polyline points="{tick(652, 742, 130)}" fill="none" stroke="#fff" stroke-width="41" stroke-linecap="round" stroke-linejoin="round"/>'
)
(OUT / "glyph-mono.svg").write_text(svg(1024, 1024, mono))

# ------------------------------------------------------------- the wordmark
font = TTFont(FONTS / "poppins-700-latin.woff2")
gs = font.getGlyphSet()
cmap = font.getBestCmap()
upm = font["head"].unitsPerEm
hmtx = font["hmtx"]


def text_path(text, x, baseline, size, tracking=-0.02):
    """`text` as one SVG path, glyph advances plus tracking (em fraction)."""
    scale = size / upm
    pen = SVGPathPen(gs)
    cur = 0.0
    for ch in text:
        name = cmap[ord(ch)]
        tp = TransformPen(pen, (scale, 0, 0, -scale, x + cur, baseline))
        gs[name].draw(tp)
        cur += hmtx[name][0] * scale + tracking * size
    return pen.getCommands(), cur


def wordmark(x, baseline, size, ink, coral=CORAL):
    a, wa = text_path("Agent Counter", x, baseline, size)
    b, wb = text_path("check", x + wa, baseline, size)
    return f'<path d="{a}" fill="{ink}"/><path d="{b}" fill="{coral}"/>', wa + wb


def lockup(ink, coral, name, tile=True):
    size = 168
    body_w, total = wordmark(0, 0, size, ink, coral)
    mark_h = 260
    gap = 56
    w = int(mark_h + gap + total + 20)
    h = 300
    m = TILE + figure() if tile else figure()
    s = (
        f'<g transform="translate(20 20) scale({mark_h / 1024})">{m}</g>'
        f'<g transform="translate({20 + mark_h + gap} {h / 2 + size * 0.36:.1f})">{body_w}</g>'
    )
    (OUT / name).write_text(svg(w, h, s))
    return w, h


lockup(INK, CORAL, "lockup-light.svg")
lockup("#F8FAFC", "#FB923C", "lockup-dark.svg")

# --------------------------------------------- GitHub social preview 1280x640
TEXT_X, MAX_W = 470, 1280 - 470 - 80


def fitted(text_fn, want):
    """Render at `want`; if wider than MAX_W, scale the size down to fit."""
    body, w = text_fn(want)
    if w <= MAX_W:
        return body, w, want
    size = want * MAX_W / w
    body, w = text_fn(size)
    return body, w, size


wm, _, wm_size = fitted(lambda z: wordmark(0, 0, z, INK), 80)
tag, _, _ = fitted(lambda z: text_path("Proof that AI agents' work really happened.", 0, 0, z, tracking=-0.01), 36)
social = (
    '<rect width="1280" height="640" fill="#F7F9F8"/>'
    '<rect x="0" y="600" width="1280" height="40" fill="#0F766E"/>'
    f'<g transform="translate(90 150) scale({340 / 1024})">{TILE + figure()}</g>'
    f'<g transform="translate({TEXT_X} 300)">{wm}</g>'
    f'<g transform="translate({TEXT_X + 3} 368)"><path d="{tag}" fill="#334155"/></g>'
)
small_lines = [
    "Re-runs your checks in a clean copy the agent can't edit,",
    "and writes a sealed receipt. Open source: pip install agent-countercheck",
]
# One size for both lines: the one that fits the longer line.
line_size = min(fitted(lambda z, t=t: text_path(t, 0, 0, z, tracking=0), 24)[2] for t in small_lines)
y = 436
for line in small_lines:
    p, _ = text_path(line, 0, 0, line_size, tracking=0)
    social += f'<g transform="translate({TEXT_X + 3} {y})"><path d="{p}" fill="#64748B"/></g>'
    y += 36
(OUT / "social-preview.svg").write_text(svg(1280, 640, social))

# README header: lockup on a transparent background, wide and short.
hdr_wm, hdr_w = wordmark(0, 0, 64, INK)
hdr = (
    f'<g transform="translate(8 8) scale({112 / 1024})">{TILE + figure()}</g>'
    f'<g transform="translate(144 84)">{hdr_wm}</g>'
)
(OUT / "readme-header.svg").write_text(svg(int(144 + hdr_w + 16), 128, hdr))
print("built", sorted(p.name for p in OUT.iterdir()))
