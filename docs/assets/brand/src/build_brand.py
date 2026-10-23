"""Build the Agent Countercheck logo set from one source geometry (v5 look: Inter 600, one-colour wordmark).

Outputs (in OUT): mark.svg (app icon, 1024), mark-small.svg (simplified for <= 48 px),
glyph.svg (no background tile), glyph-mono.svg, lockup-light.svg, lockup-dark.svg,
social-preview.svg (1280x640), readme-header.svg. Text is converted to outlines (Inter and JetBrains Mono,
OFL; the variable fonts from datacrate360.com's public/fonts) so every SVG renders the same everywhere.
"""
import math
import sys
from pathlib import Path

from fontTools.pens.svgPathPen import SVGPathPen
from fontTools.pens.transformPen import TransformPen
from fontTools.ttLib import TTFont
from fontTools.varLib.instancer import instantiateVariableFont

OUT = Path(sys.argv[1])
FONTS = Path(sys.argv[2])
OUT.mkdir(parents=True, exist_ok=True)

# v5 palette (datacrate360.com docs/design-direction.md): logo teal is the one accent, coral only on the seal.
TEAL, TEAL_D, TEAL_L, CORAL, INK = "#0F4C4C", "#082B2B", "#1E7A72", "#C2410C", "#141414"
NIGHT, PAPER, MUTED = "#0E1211", "#FAFAF9", "#6B6B66"

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
class Face:
    """A font at one weight (variable fonts are instanced), drawn as SVG outlines."""

    def __init__(self, file, weight):
        f = TTFont(FONTS / file)
        if "fvar" in f:
            f = instantiateVariableFont(f, {"wght": weight})
        self.gs, self.cmap = f.getGlyphSet(), f.getBestCmap()
        self.upm, self.hmtx = f["head"].unitsPerEm, f["hmtx"]

    def path(self, text, x, baseline, size, tracking=0.0):
        """`text` as one SVG path, glyph advances plus tracking (em fraction); returns (d, width)."""
        scale = size / self.upm
        pen = SVGPathPen(self.gs)
        cur = 0.0
        for ch in text:
            name = self.cmap[ord(ch)]
            self.gs[name].draw(TransformPen(pen, (scale, 0, 0, -scale, x + cur, baseline)))
            cur += self.hmtx[name][0] * scale + tracking * size
        return pen.getCommands(), cur - tracking * size


INTER600 = Face("inter-latin.woff2", 600)
INTER400 = Face("inter-latin.woff2", 400)
MONO = Face("jetbrains-mono-latin.woff2", 500)
NAME = "Agent Countercheck"


def wordmark(size, ink):
    """One colour, tight tracking like the site's headlines (never a two-colour wordmark)."""
    d, w = INTER600.path(NAME, 0, 0, size, tracking=-0.035)
    return f'<path d="{d}" fill="{ink}"/>', w


def lockup(ink, name):
    size = 150
    body, total = wordmark(size, ink)
    mark_h, gap, h = 260, 52, 300
    w = int(20 + mark_h + gap + total + 24)
    s = (
        f'<g transform="translate(20 20) scale({mark_h / 1024})">{TILE + figure()}</g>'
        # Optical centre: cap height of Inter is ~0.727 em.
        f'<g transform="translate({20 + mark_h + gap} {h / 2 + size * 0.727 / 2:.1f})">{body}</g>'
    )
    (OUT / name).write_text(svg(w, h, s))


lockup(INK, "lockup-light.svg")
lockup(PAPER, "lockup-dark.svg")


def fit(face, text, want, max_w, tracking=0.0):
    """The size (<= want) at which `text` fits max_w."""
    _, w = face.path(text, 0, 0, want, tracking)
    return want if w <= max_w else want * max_w / w


# --------------------------------------------- GitHub social preview 1280x640
# v5 "C" marketing look: near-black teal-tinted night band, faint 44 px grid, one soft teal light behind the product.
TEXT_X = 476
MAX_W = 1280 - TEXT_X - 72
grid = "".join(f'<line x1="{x}" y1="0" x2="{x}" y2="640"/>' for x in range(0, 1281, 44)) + "".join(
    f'<line x1="0" y1="{y}" x2="1280" y2="{y}"/>' for y in range(0, 641, 44)
)
wm_size = fit(INTER600, NAME, 76, MAX_W, -0.035)
wm, _ = wordmark(wm_size, PAPER)
tag_text = "Proof that AI agents' work really happened."
tag_d, _ = INTER400.path(tag_text, 0, 0, fit(INTER400, tag_text, 34, MAX_W, -0.01), -0.01)
lines = ["Re-runs your checks in a clean copy the agent can't edit,", "and writes a sealed receipt. macOS, Linux, Windows, CI."]
line_size = min(fit(INTER400, t, 24, MAX_W) for t in lines)
cmd = "pip install agent-countercheck"
cmd_d, cmd_w = MONO.path(cmd, 0, 0, 22)
social = (
    f'<rect width="1280" height="640" fill="{NIGHT}"/>'
    f'<g stroke="#FFFFFF" stroke-opacity=".045" stroke-width="1">{grid}</g>'
    '<radialGradient id="glow" cx=".5" cy=".5" r=".5"><stop offset="0" stop-color="#1E7A72" stop-opacity=".55"/>'
    '<stop offset="1" stop-color="#1E7A72" stop-opacity="0"/></radialGradient>'
    '<circle cx="262" cy="318" r="300" fill="url(#glow)"/>'
    f'<g transform="translate(92 148) scale({340 / 1024})">{TILE + figure()}</g>'
    f'<g transform="translate({TEXT_X} 246)">{wm}</g>'
    f'<g transform="translate({TEXT_X + 2} 308)"><path d="{tag_d}" fill="#D6DBD8"/></g>'
)
y = 374
for t in lines:
    d, _ = INTER400.path(t, 0, 0, line_size)
    social += f'<g transform="translate({TEXT_X + 2} {y})"><path d="{d}" fill="#9AA39F"/></g>'
    y += 36
social += (
    f'<rect x="{TEXT_X}" y="434" width="{cmd_w + 40:.0f}" height="48" rx="24" fill="#FFFFFF" fill-opacity=".06" '
    'stroke="#FFFFFF" stroke-opacity=".14"/>'
    f'<g transform="translate({TEXT_X + 20} 465)"><path d="{cmd_d}" fill="#8FD1C6"/></g>'
)
(OUT / "social-preview.svg").write_text(svg(1280, 640, social))

# README header: compact lockup on a transparent background.
hdr_body, hdr_w = wordmark(60, INK)
hdr = (
    f'<g transform="translate(8 8) scale({112 / 1024})">{TILE + figure()}</g>'
    f'<g transform="translate(140 {64 + 60 * 0.727 / 2:.1f})">{hdr_body}</g>'
)
(OUT / "readme-header.svg").write_text(svg(int(140 + hdr_w + 16), 128, hdr))
print("built", sorted(p.name for p in OUT.iterdir()))
