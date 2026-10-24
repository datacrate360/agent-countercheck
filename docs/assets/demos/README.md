# Demo recordings

The GIFs in the main README are real runs of `countercheck`, recorded from scripts in this folder, so anyone can
re-record them after a change.

| GIF | Shows | Recorded with |
|---|---|---|
| `verify.gif` | an agent skipped a failing test; its own run says OK; `countercheck verify` says FAILED, then VERIFIED after the fix | `vhs docs/assets/demos/verify.tape` ([VHS](https://github.com/charmbracelet/vhs): `brew install vhs`) |
| `ui.gif` | the live `countercheck ui` page with three agents in one folder: one checking now and sent back, two checking at once and delivered, the replay of the failed run, then all three delivered | `node docs/assets/demos/record-ui.mjs` (needs Chrome, ffmpeg and `puppeteer-core`; `THEME=dark` for the dark page) |

`verify.gif` uses the demo project from `setup-pricing.sh` (created in `/tmp/countercheck-demo`); `ui.gif` uses
`setup-team.sh` (`/tmp/countercheck-team`: the same project with three checks, one slow so a run in progress shows).
Both use the `countercheck` on your `PATH`: record with a release build (`cargo build --release`, then put
`target/release` first on `PATH`). Run them from the repository root. Keep GIFs under ~1 MB; GitHub shows them at up
to 880 px wide.

The diagrams in `docs/img/` are SVG. `architecture.svg` and the animated `a-run.svg` are built by the scripts in
`docs/img/src/` (`python3 docs/img/src/build_architecture.py docs/img/architecture.svg`, and the same for
`build_run.py`); the others are edited by hand.
