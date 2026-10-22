"""Render the SVGs to PNG with headless Chrome (transparent where the SVG is) and build favicon.ico."""
import subprocess
import sys
from pathlib import Path

from PIL import Image

D = Path(sys.argv[1])
CHROME = "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"


def render(svg: str, w: int, h: int, out: str):
    page = D / "_render.html"
    page.write_text(
        f'<html><body style="margin:0;background:transparent">'
        f'<img src="{svg}" width="{w}" height="{h}" style="display:block"></body></html>'
    )
    subprocess.run(
        [CHROME, "--headless=new", "--disable-gpu", "--hide-scrollbars",
         "--default-background-color=00000000", f"--window-size={w},{h}",
         f"--screenshot={D / out}", f"file://{page}"],
        check=True, capture_output=True,
    )
    page.unlink()


png = D / "png"
png.mkdir(exist_ok=True)
for size in (1024, 512, 256, 180, 128):
    render("mark.svg", size, size, f"png/mark-{size}.png")
for size in (64, 48, 32, 16):
    render("mark-small.svg", size, size, f"png/mark-{size}.png")
render("social-preview.svg", 1280, 640, "png/social-preview.png")
import re


def dims(svg):
    m = re.search(r'width="(\d+)" height="(\d+)"', (D / svg).read_text())
    return int(m.group(1)), int(m.group(2))


for name in ("lockup-light", "lockup-dark", "readme-header"):
    w, h = dims(f"{name}.svg")
    render(f"{name}.svg", w, h, f"png/{name}.png")

# favicon.ico: 16, 32, 48 from the small variant (sharper than downscaling the 1024).
imgs = [Image.open(png / f"mark-{s}.png").convert("RGBA") for s in (48, 32, 16)]
imgs[0].save(D / "favicon.ico", sizes=[(48, 48), (32, 32), (16, 16)], append_images=imgs[1:])
for p in sorted(png.iterdir()):
    im = Image.open(p)
    print(p.name, im.size, im.mode)
print("favicon.ico", Image.open(D / "favicon.ico").info.get("sizes"))
