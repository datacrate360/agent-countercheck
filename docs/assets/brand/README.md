# Agent Countercheck brand

The mark: the agent (a ghost) hands in its receipt, and Countercheck's coral seal is the verdict.

| File | Use |
|---|---|
| `mark.svg`, `png/mark-{128,180,256,512,1024}.png` | App icon, avatars, social profiles (1024 px master) |
| `mark-small.svg`, `png/mark-{16,32,48,64}.png`, `favicon.ico` | Favicons and anything 64 px or smaller (simplified: no text lines, bigger seal) |
| `glyph.svg` | The figure without its tile, on your own background |
| `glyph-mono.svg` | One colour (teal on white): stamps, watermarks, print |
| `lockup-light.svg`, `lockup-dark.svg`, `png/lockup-*.png` | Mark + wordmark on light / dark backgrounds |
| `readme-header.svg` | Compact lockup for documents |
| `social-preview.svg`, `png/social-preview.png` | GitHub social preview (1280 x 640): Settings > General > Social preview |

Look: the v5 style of datacrate360.com (its `docs/design-direction.md`). Logo teal is the one accent; coral appears
only on the seal (the verdict); the wordmark is one colour.

| Colour | Hex | Use |
|---|---|---|
| Teal | `#0F4C4C` (tile `#1E7A72` → `#082B2B`) | the tile, links, accents |
| Coral | `#C2410C` | the seal only |
| Ink | `#141414` | wordmark on light |
| Paper | `#FAFAF9` | wordmark on dark |
| Night | `#0E1211` | dark bands (social preview), with a faint 44 px grid |

Type: wordmark **Inter 600**, tracking −0.035 em; code and commands **JetBrains Mono**. All text in these files is
converted to outlines.

Rebuild everything after a change: `python src/build_brand.py <out> <fonts>` where `<fonts>` holds
`inter-latin.woff2` and `jetbrains-mono-latin.woff2` (the variable fonts datacrate360.com self-hosts; needs
`fonttools` and `brotli`), then `python src/export_png.py <out>` (needs Pillow and Google Chrome).

## Licence

The code in this repository is MIT licensed. **The Agent Countercheck name and these logo files are not covered by the
MIT licence**: you may use them to refer to the project (in articles, talks, a "works with Agent Countercheck" badge),
but not as the logo of another product or in a way that suggests endorsement.
