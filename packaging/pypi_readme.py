"""Make README.md's relative links absolute for PyPI, which can't resolve them.

PyPI shows README.md as the project page, but it has no repository to resolve `docs/img/x.svg` or
`CHANGELOG.md` against, so images break and links lead nowhere. The release workflow runs this on
its own checkout (never committed) just before the Python packages are built:

    python3 packaging/pypi_readme.py v0.9.1          # rewrite README.md in place for that tag
    python3 packaging/pypi_readme.py --check         # CI: the rewrite leaves no relative link

Images point at raw.githubusercontent.com and pages at github.com, both pinned to the release tag,
so a PyPI release keeps showing the images and docs it shipped with. `#anchor` links stay.
"""
import re
import sys
from pathlib import Path

REPO = "datacrate360/agent-countercheck"
README = Path(__file__).resolve().parent.parent / "README.md"

# Markdown links and images: ](target) and ](target "title"); HTML src= / srcset= / href=.
MD = re.compile(r"(\]\()([^)\s]+)((?:\s+\"[^\"]*\")?\))")
HTML = re.compile(r"""\b(src|srcset|href)=(["'])([^"']+)\2""")
IMAGE_EXT = (".svg", ".png", ".gif", ".jpg", ".jpeg", ".webp")


def absolute(target: str, ref: str, image: bool) -> str:
    if re.match(r"^[a-z][a-z0-9+.-]*:", target, re.I) or target.startswith(("#", "//")):
        return target  # a URL, mailto:, or an anchor on the same page
    path = target[2:] if target.startswith("./") else target.lstrip("/")
    if image or path.split("#")[0].lower().endswith(IMAGE_EXT):
        return f"https://raw.githubusercontent.com/{REPO}/{ref}/{path}"
    # github.com redirects /blob/ to /tree/ for folders.
    return f"https://github.com/{REPO}/blob/{ref}/{path}"


def rewrite(text: str, ref: str) -> str:
    def md(m: re.Match) -> str:
        # An image when its `[alt]` starts with `![`. A link whose text ends with an image,
        # `[![badge](x.svg)](LICENSE)`, ends in `)` before its `](`: that one is a link, not an image.
        start = text.rfind("[", 0, m.start() + 1)
        image = text[m.start() - 1] != ")" and start > 0 and text[start - 1] == "!"
        return m.group(1) + absolute(m.group(2), ref, image) + m.group(3)

    text = MD.sub(md, text)
    return HTML.sub(lambda m: f"{m.group(1)}={m.group(2)}{absolute(m.group(3), ref, m.group(1) != 'href')}{m.group(2)}", text)


def relative_left(text: str) -> list[str]:
    left = []
    for m in MD.finditer(text):
        t = m.group(2)
        if not re.match(r"^([a-z][a-z0-9+.-]*:|#)", t, re.I):
            left.append(t)
    for m in HTML.finditer(text):
        t = m.group(3)
        if not re.match(r"^([a-z][a-z0-9+.-]*:|#)", t, re.I):
            left.append(t)
    return left


def main() -> int:
    if len(sys.argv) != 2:
        print(__doc__)
        return 2
    text = README.read_text(encoding="utf-8")
    if sys.argv[1] == "--check":
        out = rewrite(text, "PROD")
        left = relative_left(out)
        missing = [
            p
            for p in re.findall(rf"https://(?:raw\.githubusercontent|github)\.com/{REPO}/(?:blob/)?PROD/([^)\s\"'#]+)", out)
            if not (README.parent / p).exists()
        ]
        raw_pages = [
            u for u in re.findall(rf"https://raw\.githubusercontent\.com/{REPO}/PROD/([^)\s\"']+)", out)
            if not u.split("#")[0].lower().endswith(IMAGE_EXT)
            and f"raw.githubusercontent.com/{REPO}/PROD/{u}" not in text  # written raw on purpose (curl | sh)
        ]
        for u in raw_pages:
            print(f"a page linked as a raw file (should be github.com/.../blob/): {u}")
        for t in left:
            print(f"still relative: {t}")
        for p in missing:
            print(f"links to a file that doesn't exist: {p}")
        print(f"{len(MD.findall(text)) + len(HTML.findall(text))} links checked")
        return 1 if left or missing or raw_pages else 0
    out = rewrite(text, sys.argv[1])
    README.write_text(out, encoding="utf-8")
    print(f"README.md: links pinned to {sys.argv[1]} for PyPI")
    return 0


if __name__ == "__main__":
    sys.exit(main())
