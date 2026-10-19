# Report and UI assets

The page that `countercheck report` writes and `countercheck ui` serves.

| File | What it is |
|---|---|
| `report.css` | All styles (brand teal `#0F766E`, coral `#C2410C`). Light theme. |
| `live.js` | The live UI's only script: polls for new receipts and reloads. Not used by `report`. |

Both are compiled into the `countercheck` binary with `include_str!` (`src/report.rs`), so installs stay one file with
no Node, npm or build step. Edit them here, then `cargo build` and look:

```sh
cargo build && ./target/debug/countercheck report --open      # the static page
./target/debug/countercheck ui                                 # the live page
```

The HTML itself is built in `src/report.rs`, because it is filled with receipt data that must be escaped. Rules:

- No outside resources (fonts, CDNs, images by URL): the page's Content-Security-Policy blocks them, and the report
  must work offline.
- No `</script>` text in `live.js`, and no new scripts: the live page runs only the script carrying its one-time
  nonce, and the static report runs none.
- Check both pages in a real browser after a change (screenshots in the pull request help reviewers).
