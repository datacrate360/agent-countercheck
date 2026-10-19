# ADR-0001: Stack for Countercheck and the DataCrate360 Launcher

Status: proposed (2026-10-18). Reference studied: the Craft App Suite (`creative-studios/`), especially
`craft-launcher-main` (ArtCraft Launcher v0.4.1, MIT OR Apache-2.0).

## What the reference does (read from its source)

- **Launcher**: 100% Rust, ~18.7k lines. A native egui/eframe window plus a tray/menu-bar icon (no Electron, no webview),
  with a WebAssembly build of the same UI. Three crates: `core` (pure, wasm-safe: app catalog, versions, GitHub release
  parsing, picking the right file per OS/CPU), `native` (HTTP, detect installed apps, download, install, self-update),
  `ui-egui`.
- **Catalog**: a built-in list of 13 apps (`crates/core/src/catalog.rs` `SUITE`). It downloads only from one GitHub
  owner's Releases (`GITHUB_OWNER = "storytold"`) and checks every file against the release's `SHA256SUMS.txt` before
  installing. Agents may never add a third-party app.
- **Mac packaging**: universal binary (Apple silicon + Intel), `.app` + DMG, Developer ID signing, notarization, stapled
  ticket, and `verify.sh` checks Gatekeeper accepts it (`packaging/macos/`). Release = push to the `release` branch,
  CI builds every platform and drafts a GitHub Release with fixed asset names.
- **Rules**: "never crash" (clippy denies unwrap/expect/panic, `unsafe` forbidden), all network input is hostile,
  and every string is translatable.
- **The apps** are Rust + egui too; ArtCraft itself is Tauri + React. Shared AI connection crates
  (`craft-ai-integrations`): one AI setup in the launcher, used by every app; secrets in the OS keychain.

## Decision

| Part | Choice | Why |
|---|---|---|
| **Countercheck core** (CLI `countercheck`, local daemon, verifier, receipts, adapters, MCP server) | **Rust** | One signed binary for Mac (and later Windows/Linux) with nothing else to install; fast start and low memory for an always-on daemon; a referee has to be trustworthy, and Rust removes whole classes of crash and memory bugs; strong compiler + clippy gate suits AI-written code |
| Checks and plugins | **Any language, run in the sandbox** (Python first) | Verification runs the *project's own* commands (pytest, npm test, dbt test…), so the core never needs Python itself; Python tools like MarkItDown or Strix run as separate processes |
| Storage | SQLite (`rusqlite`), receipts signed with Ed25519 | Local-first, one file, portable; Postgres only for a later team server |
| HTTP / MCP | `axum` + `tokio`; official Rust MCP SDK (`rmcp`) | One stack for the daemon API and the MCP server agents call |
| Sandbox on macOS | Phase 1: run checks in a clean copy of the repo with Seatbelt limits; Phase 2: containers (Docker/OrbStack, or Apple's `container`) | Needs a spike: confirm what works on a stock Mac without admin rights |
| **Countercheck UI** (receipts, diffs, approve) | **Tauri 2 + React + TypeScript** | Rich diff/table/markdown views and OpenUI (React) reuse; the same React app can be served by the daemon in a browser for teams; Tauri has signing, notarization and an updater built in |
| **DataCrate360 Launcher** | **Rust + egui**, modelled on craft-launcher | Small native tray app; installs/updates/opens every DataCrate360 tool from `github.com/datacrate360/*` Releases with SHA-256 checks. The code is MIT/Apache, so it can be adapted with attribution; ArtCraft's name, logo and "A" mark are trademarks and must not be reused |
| Distribution | GitHub Releases + `SHA256SUMS.txt`; signed + notarized universal DMG; later a Homebrew tap and `curl \| sh` for the CLI | Same proven path as the reference |

## Why not Python for the core

Python is the owner's strongest language, but shipping it to Macs is the weak point. Our Hermes research found that
its installer must set up Python, Node, ripgrep and FFmpeg. For a tool whose whole promise is *trust*, a single
notarized binary is the better first impression. OpenAI rewrote the Codex CLI in Rust for the same reason
(distribution and speed). Python keeps a first-class place in the checks and plugins.

## Costs and risks

- **Learning curve**: Rust is new to the owner. Mitigation: a small, strict core (craft-style lint rules, `cargo xtask ci`),
  the coding agents write most of it, and the compiler is the first reviewer.
- **Two UI stacks** (egui launcher, Tauri+React app). Accepted: the launcher must be tiny; the app needs rich views.
  Alternative if this proves heavy: the launcher in Tauri too.
- **Apple Developer Program** (paid yearly) is required for signing and notarization; until then, ad-hoc signed builds
  show a Gatekeeper warning.
- **Sandbox on macOS** is the least proven part. Spike it before promising isolation in public copy.

## Update (2026-10-19): the local UI

The local UI shipped as `countercheck report` (one HTML file) and `countercheck ui` (a live page on 127.0.0.1), built
into the binary: HTML from Rust (every receipt value escaped), CSS and one small script as files in
`crates/countercheck-core/assets/` embedded with `include_str!`, a server on the Rust standard library. Not Tauri +
React, because for a read-only view of receipts:

- installs stay one file through brew, pip, uvx and curl, with no Node toolchain in the release pipeline;
- no npm packages inside a security tool (enterprise reviews count every dependency);
- the static report has no script at all, and the live page runs only its own nonced script.

React (or Tauri) remains the choice for the hosted team dashboard, which needs logins, many repositories and rich
views. Measured: the release binary is about 3.4 MB and starts in under 10 ms.
