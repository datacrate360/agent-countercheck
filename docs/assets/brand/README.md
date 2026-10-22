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

Colours: teal `#0F766E` (dark `#083F3B`, light `#1BA393`), coral `#C2410C` (`#FB923C` on dark), ink `#0F172A`.
Wordmark: Poppins Bold, "Agent Counter" in ink and "check" in coral; all text in these files is converted to outlines.

Rebuild everything after a change: `python src/build_brand.py <out> <folder with poppins-*-latin.woff2>` (needs
`fonttools` and `brotli`), then `python src/export_png.py <out>` (needs Pillow and Google Chrome).

## Licence

The code in this repository is MIT licensed. **The Agent Countercheck name and these logo files are not covered by the
MIT licence**: you may use them to refer to the project (in articles, talks, a "works with Agent Countercheck" badge),
but not as the logo of another product or in a way that suggests endorsement.
