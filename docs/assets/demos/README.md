# Demo recordings

The GIFs in the main README are real runs of `countercheck`, recorded from scripts in this folder, so anyone can
re-record them after a change.

| GIF | Shows | Recorded with |
|---|---|---|
| `verify.gif` | an agent skipped a failing test; its own run says OK; `countercheck verify` says FAILED, then VERIFIED after the fix | `vhs docs/assets/demos/verify.tape` ([VHS](https://github.com/charmbracelet/vhs): `brew install vhs`) |
| `ui.gif` | the live `countercheck ui` page updating as those two runs land | `node docs/assets/demos/record-ui.mjs` (needs Chrome, ffmpeg and `puppeteer-core`) |

Both use the demo project from `setup-pricing.sh` (created in `/tmp/countercheck-demo`) and the `countercheck` on
your `PATH`: record with a release build (`cargo build --release`, then put `target/release` first on `PATH`). Run them
from the repository root. Keep GIFs under ~1 MB; GitHub shows them at up to 880 px wide.
