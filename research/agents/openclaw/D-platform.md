# OpenClaw — D: platform (Identity, Architecture, Surfaces, Extensibility, Quality/Ops, Repo practices)

- **Repo:** `openclaw/openclaw`, shallow clone at `cache/scratch/agents/openclaw` (all paths below are relative to that root)
- **Commit:** `15ae101a0d854480cfb500a5e241055450da39da`, 2026-10-18 13:39:07 +0000, "refactor(tests): remove obsolete test-only exports (#176210)". The clone is shallow (`git log | wc -l` = 1), so git history and authorship can't be checked locally.
- **Package version at HEAD:** `2026.10.4` (CalVer) — `package.json:3`
- **Method:** static reading plus `git ls-files`/`wc`/`grep`/small Python counters. Nothing was built, installed or run. Every count is a lexical count over tracked files at this commit.
- **Labels:** [Verified] = read in code/config at this commit. [Docs] = in-repo docs or README prose that is not code. [Inference] = my reading. [Context] = facts the task brief supplied (GitHub metadata), not checkable in a shallow clone.
- **Files read in full or in key parts:** `VISION.md`, `AGENTS.md`, `README.md` (1–199), `CONTRIBUTING.md` (30–229), `SECURITY.md` (grep), `docs/concepts/architecture.md`, `docs/gateway/protocol.md`, `docs/start/lore.md` (10–40, 110–139), `src/gateway/AGENTS.md`, `src/plugin-sdk/AGENTS.md`, `packages/gateway-protocol/README.md`, `packages/gateway-protocol/src/version.ts`, `src/gateway/methods/core-descriptors.ts`, `src/gateway/methods/core-method-policy.ts`, `src/gateway/server-methods-list.ts`, `src/gateway/operator-scopes.ts`, `src/plugins/plugin-api.types.ts` (grep), `src/plugins/bundle-manifest.ts` (grep), `packages/plugin-package-contract/src/*`, `src/infra/telemetry.ts` (grep), `src/config/zod-schema.telemetry.ts`, `src/config/schema.help.core.ts`, `src/infra/clawhub-install-trust.ts`/`clawhub-integrity.ts` (grep), all 171 `extensions/*/openclaw.plugin.json` (parsed), `Dockerfile`, `fly.toml`, `render.yaml`, `docker-compose.yml`, `scripts/install.sh` (grep), `.github/workflows/ci.yml` (job list), `.github/CODEOWNERS`, `.github/pull_request_template.md` (headings), `.claude/settings.json`, `qa/README.md`, `qa/maturity-scores.yaml` (parsed), one `qa/scenarios/**.yaml`, `taxonomy.yaml` (head), `docs/reference/database-schemas.md` (head), `docs/reference/database-engines.md` (grep), `apps/linux/README.md` (head), `docs/platforms/windows.md` (grep), `docs/channels/*` (grep).

---

## D1 — Identity

**In plain words.** OpenClaw is a self-hosted personal (and team) AI assistant. A long-running "Gateway" process on your machine connects to your chat apps (WhatsApp, Telegram, Slack, Discord, iMessage…) and to companion apps on your phone and desktop. It is MIT-licensed and run by a non-profit foundation, with no paid tier. It is a very large TypeScript monorepo (≈4.9M non-test TS LOC) with native Swift and Kotlin apps and a small Rust part. It started as a WhatsApp relay called Warelay, then became Clawdbot, then Moltbot, and was renamed OpenClaw on 30 Jan 2026.

### Purpose and positioning
- Tagline: "OpenClaw is the AI that actually does things. It runs on your devices, in your channels, with your rules." — `VISION.md:3-4` [Verified]
- Stated goal: "a personal assistant that is easy to use, supports a wide range of platforms, and respects privacy and security" — `VISION.md:15` [Verified]
- Personal vs team: "A personal install is yours alone; a shared Gateway is a place people work together, so the same session can carry several humans, their credit, and their history." — `VISION.md:17-18` [Verified]. They dogfood on `team.openclaw.ai` — `VISION.md:19` [Docs]
- README pitch: runs on your computer, works in Discord/iMessage/Slack/Teams/Telegram/WhatsApp "and 20+ more", plus native apps for macOS/iOS/Android/Windows/Linux. "One Gateway runs it as a personal assistant on a laptop or as a shared team deployment; configuration is the only difference." — `README.md:18` [Docs]
- npm description: "Multi-channel AI gateway with extensible messaging integrations" — `package.json:22` [Verified]
- Priority order: (1) security and safe defaults, bug fixes, first-run setup, performance/e2e/plugin APIs; (2) runtime (sessions, automations, subagents, memory, all major providers, channels, computer use, harnesses); (3) ops/apps — `VISION.md:24-45` [Verified]
- Architecture thesis: "trusted gateway, untrusted execution, policy enforced by code" — `README.md:20` [Docs]
- Models and harnesses are swappable plugins: "Models and agent harnesses (Claude, Codex, local models) are plugins you can swap" — `README.md:20` [Docs]; backed by the `agent-runtimes` plugin category (acpx, agentsapi, codex, copilot, muse) — `extensions/*/openclaw.plugin.json` [Verified]
- Explicit non-goals ("What We Will Not Merge"): new core skills (use ClawHub), vendor-specific integrations in core, cloud sandbox providers as plugins (send them to Crabbox), wrapper channels, MCP work that duplicates existing paths, heavy orchestration layers — `VISION.md:152-160` [Verified]
- Why TypeScript: "OpenClaw is primarily an orchestration system… TypeScript was chosen to keep OpenClaw hackable by default." — `VISION.md:146-150` [Verified]

### License and governance
- License MIT — `package.json:28`, `LICENSE` [Verified]. Author field: "OpenClaw Foundation (https://openclaw.org)" — `package.json:29` [Verified]
- "Stewarded by the OpenClaw Foundation, an independent 501(c)(3), and has no paid tier, hosted service, or token." — `README.md:20` [Docs]
- "The Foundation employs the core team and signs releases. Donors and infrastructure sponsors… none of them own or direct the project. OpenAI is a donor, not an owner." — `README.md:110` [Docs]
- Donors listed: Amazon, Lobster Computer Company, Offline Holdings, OpenAI, Red Hat, University of Michigan. Infra sponsors: Blacksmith, Convex, GitHub, NVIDIA, Vercel — `README.md:124` [Docs]
- Hosted services "should simplify setup… but stay optional" — `VISION.md:21-22` [Docs]
- Creator credit: "built for Molty, a space lobster AI assistant, by Peter Steinberger and the community"; thanks Mario Zechner for `pi` — `README.md:118-120` [Docs]. The repo depends on `@earendil-works/pi-tui` 1.1.0 — `package.json:2334` [Verified]
- AGENTS.md cites the founder by name as a design authority: "Do less work (Peter, 2026-10-14)" — `AGENTS.md:72` [Verified]
- Review routing: CODEOWNERS teams `@openclaw/openclaw-secops` and `@openclaw/openclaw-release-managers` — `.github/CODEOWNERS` [Verified]
- GitHub metadata (392k stars, created 2025-11-24, top contributor steipete) comes from the brief and can't be checked offline [Context]. The README contributor wall starts with steipete — `README.md:144` [Docs]

### History and names (checked, not guessed)
- "It evolved through several names and shells: Warelay -> Clawdbot -> Moltbot -> OpenClaw." — `VISION.md:13` [Verified]
- Lore: "In the beginning, there was **Warelay** — a sensible name for a WhatsApp gateway." Then Clawd/Clawdbot. "In January 2026, Anthropic sent a polite email asking for a name change (trademark stuff)." — `docs/start/lore.md:14-18` [Docs]
- Moltbot picked by community vote on 27 Jan 2026; renamed to OpenClaw on 30 Jan 2026 (GitHub org, npm and docs migrated "in just 3 hours") — `docs/start/lore.md:24-34`, `docs/start/lore.md:113-125` [Docs]
- The CHANGELOG shows the renames: `CLAWDBOT_STATE_DIR` env var — `CHANGELOG/2026.1.10.md:66`; Bonjour type `_moltbot-gw._tcp` — `CHANGELOG/2026.1.20.md:84`; ".clawdbot to .openclaw" path move — `CHANGELOG/2026.2.1.md:31` [Verified]
- Releases: 131 per-release changelog files in `CHANGELOG/`, earliest `2026.1.5.md`, latest `2026.10.4.md`, plus `Unreleased.md` and `records/` — `CHANGELOG/` listing; the index is generated ("openclaw:split-changelog") — `CHANGELOG.md:3` [Verified]
- Fast cadence: about 9–11 CalVer releases a month in Sept–Oct 2026 (2026.9.1…9.9, 2026.10.1…10.4) — `CHANGELOG.md:9-22` [Verified]

### Languages, stack, size
- Runtime: Node `>=24.16.0 <25 || >=26.1.0` — `package.json:2465`. Package manager pnpm 12.11.1 — `package.json:2468` [Verified]. Bun is also supported (pinned OpenClaw Bun fork is the default for local Vitest and daily CI) — `CONTRIBUTING.md:85`, `AGENTS.md:64` [Verified]
- Key deps: `ws` 8.22, `express` 5.3, `typebox` 1.3.36 (protocol schemas), `zod` 4.6.5 (config schema), `kysely` 0.29.6 (SQL), `tslog` 5.2 (logging), `grammy` 1.46 (Telegram), `baileys` 7.0.0-rc14 (WhatsApp) — `package.json:2363-2431` [Verified]. 73 deps / 59 devDeps at root [Verified]
- Control UI: Solid 2.0.0-rc.14 + Vite 8.3.4 — `ui/package.json:53,69`. 1,265 files import `solid-js` and 0 import `lit` [Verified]. **Doc drift:** `CONTRIBUTING.md:200-213` still says "The Control UI uses Lit with legacy decorators" [Verified]
- Native: Swift (macOS, iOS, watchOS, shared `OpenClawKit`), Kotlin (Android + Wear), Tauri v2 (Linux/Windows backup desktop), Rust workspace `crates/` with `openclaw-gateway-client` and `openclaw-node-host` ("Reusable Rust OpenClaw node runtime and headless host") — `crates/Cargo.toml:1-3`, `crates/openclaw-node-host/Cargo.toml` [Verified]
- Size, tracked files: **57,378** total. By top-level dir: src 27,291 · extensions 12,225 · ui 6,155 · apps 3,091 · test 2,356 · scripts 1,864 · docs 1,447 · packages 1,416 · qa 615 · .agents 293 · .github 242 · CHANGELOG 154 · skills 72 · crates 25 — `git ls-files | cut -d/ -f1 | uniq -c` [Verified]
- Files by extension: .ts 46,411 · .md 2,094 · .swift 1,574 · .tsx 1,112 · .json 933 · .mjs 727 · .mts 654 · .yaml 637 · .kt 620 · .sh 304 · .rs 70 · .sql 67 [Verified]
- LOC (raw `wc -l`, code files only, non-test):

| Area | Tracked files | Non-test code LOC (ts/tsx/mts/mjs/swift/kt/rs) | `*.test.*` files |
|---|---|---|---|
| src | 27,291 | 2,883,272 | 11,613 |
| extensions | 12,225 | 1,117,856 | 4,325 |
| apps | 3,091 | 755,331 | (Swift/Kotlin tests use other names; 957 test-path files) |
| ui | 6,155 | 540,086 | 2,700 |
| scripts | 1,864 | 464,738 | 0 |
| packages | 1,416 | 122,192 | 484 |
| test | 2,356 | 109,792 | 1,520 |
| crates | 25 | 16,029 | 0 |

  [Verified]. Language totals: all non-test TS/TSX ≈ 4.87M LOC; test TS/TSX ≈ **6.26M LOC in 20,596 test files**; Swift 463,771; Kotlin 251,953; Rust 54,415 (includes Cargo.lock) [Verified]
- Largest `src/` subtrees by file count: agents 5,158 · gateway 4,899 · infra 3,031 · config 1,801 · plugins 1,561 · commands 1,475 · cli 1,361 · auto-reply 1,051 · state 795 · plugin-sdk 760 · cron 554 · channels 530 · daemon 285 [Verified]
- 602 npm scripts in root `package.json` (test 205, lint 48, check 41, android 31, ui 21, ios 20, release 20) [Verified]

---

## D2 — Architecture and process model

**In plain words.** One long-running Gateway daemon per host owns everything: chat-channel connections, sessions, the agent loop, tools, databases and config. Everything else connects to it over one WebSocket port (default 18789, loopback only by default): the CLI, the web Control UI, the TUI, the desktop apps, and "nodes" (phones, Macs, headless Linux boxes) that offer device actions like camera, screen or location. The protocol is typed (TypeBox → JSON Schema → generated Swift). Every client must start with a signed `connect` handshake that declares a role and scopes. Plugins run inside the Gateway process, and database work is pushed onto worker threads.

### Components and arrows
```
 Chat platforms (WhatsApp/Baileys, Telegram/grammY, Slack, Discord, Signal, iMessage, Matrix, … 28 channel plugins)
        ▲  (channel plugin owns account/transport/security)
        │
 ┌──────┴──────────────────────────── Gateway daemon (one per host; launchd / systemd / schtasks) ───────────────┐
 │  kernel + GatewayScheduler · ~526 core RPC methods (+ plugin methods) · agent loop & harnesses (pi / Codex / ACP) │
 │  plugin registry (in-process code plugins + bundle plugins) · SQLite state DB + one DB per agent (worker threads)│
 │  HTTP: Control UI, /__openclaw__/canvas/, /__openclaw__/a2ui/, /startupz, POST /tools/invoke, plugin routes        │
 └──────▲───────────────▲──────────────────▲────────────────────────▲───────────────────────────────────────────────┘
        │ WS req/res/event (role=operator + scopes)                │ WS (role=node + caps/commands)   │ WS (role=worker)
   CLI / TUI / Control UI / macOS app / Tauri app / @openclaw/sdk     iOS / Android / macOS / headless / Rust node-host   Cloud workers (Crabbox etc.)
        │
   Model providers (57 "models" plugins) ◄── outbound HTTP from the Gateway
```
[Inference, assembled from the Verified bullets below]

### Gateway
- "A single long-lived Gateway owns all messaging surfaces… Control-plane clients… connect over WebSocket on the configured bind host (default `127.0.0.1:18789`)… One Gateway per host. It is the only place that opens a WhatsApp session." — `docs/concepts/architecture.md:10-17` [Docs]
- Default port in code: `DEFAULT_GATEWAY_PORT = 18789` — `src/config/paths.ts:283`; `port = 18789` defaults — `src/gateway/server.ts:26`, `src/gateway/server-start.ts:20`, `src/gateway/server-kernel.ts:112` [Verified]
- Default bind is loopback: `const mode = bind ?? "loopback"` — `src/gateway/net.ts:251` [Verified]. Remote access: Tailscale/VPN preferred, or an SSH tunnel (`ssh -N -L 18789:127.0.0.1:18789`) — `src/gateway/connection-details.ts:198`, `docs/concepts/architecture.md:126-136` [Verified/Docs]
- Same port also serves hosted widgets `/__openclaw__/canvas/` and the A2UI renderer `/__openclaw__/a2ui/` — `docs/concepts/architecture.md:18-22` [Docs]
- docker-compose also maps 18790 ("bridge") and 3978 (MS Teams webhook) — `docker-compose.yml:68-71` [Verified]
- Kernel owns one `GatewayScheduler` for maintenance and cron. Durable deadlines are rebuilt from owner stores at startup (no second scheduler state). Plugins get a versioned `PluginServiceSchedulerV1` — `docs/concepts/architecture.md:146-169` [Docs]
- Daemon supervision lives in code under `src/daemon/`: launchd (`launchd-*.ts`), systemd (`systemd-*.ts`, including linger and user transport), Windows Task Scheduler (`schtasks-*.ts`), plus service-audit, restart-storm and runtime-pin modules [Verified, directory listing]. Install: `openclaw onboard --install-daemon` — `README.md:56` [Docs]

### Wire protocol
- Package `@openclaw/gateway-protocol`: "Typed schemas, inferred TypeScript types, and runtime validators for the OpenClaw Gateway WebSocket protocol" — `packages/gateway-protocol/README.md:1-4` [Verified]
- Versions: `PROTOCOL_VERSION = 4`, `MIN_CLIENT_PROTOCOL_VERSION = 4`, `MIN_NODE_PROTOCOL_VERSION = 3` (an N-1 window for nodes during rolling upgrades) — `packages/gateway-protocol/src/version.ts:2-6` [Verified]
- Frames: JSON text frames. The first frame must be `connect`. Then `{type:"req",id,method,params}` → `{type:"res",id,ok,payload|error}`, plus `{type:"event",event,payload,seq?,stateVersion?}` — `docs/concepts/architecture.md:76-84` [Docs]. Frame schema has `role` fields — `packages/gateway-protocol/src/schema/frames.ts:58,136` [Verified]
- Idempotency keys are required for side-effecting methods (`send`, `agent`), with a short-lived dedupe cache. Events are not replayed; clients refresh on gaps — `docs/concepts/architecture.md:95-96,175` [Docs]
- Codegen chain: TypeBox → JSON Schema → Swift models — `docs/concepts/architecture.md:120-124` [Docs]. 111 entries in `packages/gateway-protocol/src`, with a `schema/` dir of per-domain schemas (agent, approvals, audit, board, canvas, cron, devices, nodes, worker-admission…) [Verified]
- **Core RPC surface:** one append-only table `CORE_GATEWAY_METHOD_SPECS` — `src/gateway/methods/core-descriptors.ts:16`. I parsed **526 rows** [Verified]. Largest families: sessions 64, skills 42, users 32, mcp 21, talk 19, node 19, plugins 18, session 14, openclaw 12, exec 11, device 11 [Verified]. Scope distribution: operator.read 182, operator.admin 162, operator.write 103, operator.talk 17, operator.approvals 16, dynamic 14, operator.pairing 14, node 11, operator.questions 5, operator.sessions.read 2 [Verified]
- Advertised method list = core advertised methods ∪ channel-plugin `gatewayMethods`/`gatewayMethodDescriptors` — `src/gateway/server-methods-list.ts:19-23` [Verified]. Plugins add their own via `registerGatewayMethod` — `src/plugins/plugin-api.types.ts:269` [Verified]
- Scopes are defined in code: `operator.admin|read|write|approvals|questions|pairing|talk` — `src/gateway/operator-scopes.ts:1-9` [Verified]
- Roles: operator clients, `role: node` devices, and `role: "worker"` cloud workers (`packages/gateway-protocol/src/schema/worker-admission.ts:144`). Presence distinguishes `node` vs `client` — `packages/gateway-protocol/src/schema/presence.ts:80` [Verified]
- In-process agent tools call the Gateway router directly without opening another WS, while keeping scopes and approvals — `docs/gateway/protocol.md:16-20` [Docs]

### Auth, pairing, trust
- Every WS client sends a device identity and signs a `connect.challenge` nonce. Signature v3 binds platform and deviceFamily. New devices need pairing approval, after which a device token is issued. Loopback can be auto-approved; LAN and tailnet still need explicit approval — `docs/concepts/architecture.md:99-115` [Docs]
- Auth modes: shared token/password, Tailscale Serve identity, `trusted-proxy`, and `none` (private ingress only) — `docs/concepts/architecture.md:85-94` [Docs]
- Trust model: "local-first agent infrastructure for trusted operators; it is not designed as a shared multi-tenant boundary" — `SECURITY.md:5` [Verified]. "A shared Gateway is for people who trust each other. Mutually untrusted users need separate Gateways" — `VISION.md:71-72` [Verified]
- `POST /tools/invoke` with a shared-secret bearer token = full operator access; `x-openclaw-scopes` does not narrow it — `SECURITY.md:103` [Verified]
- By default, tools run on the host for the main session unless sandboxing is configured — `README.md:81` [Docs]

### Nodes and devices
- Nodes connect to the same WS server with `role: node`, declaring caps/commands/permissions. They expose `camera.*`, `screen.record`, `location.get`, and on macOS `canvas.*` — `docs/concepts/architecture.md:39-45` [Docs]
- Node host code: `src/node-host/` (211 files: auto-update, exec-policy, computer-command, desktop-stream-command, claude-skill-session…), plus the Rust `crates/openclaw-node-host` [Verified]
- Plugins can extend nodes: `registerNodeHostCommand`, `registerNodeInvokePolicy`, `registerNodeCliFeature` — `src/plugins/plugin-api.types.ts` [Verified]
- Node docs cover audio, camera, computer-use, file transfers, location, media playback, node-exec, session hosting, talk, voicewake — `docs/nodes/` listing [Docs]

### State and storage (matters for D-data)
- `package.json` declares `databaseEngines: ["sqlite","postgres"]`, postgres families `state, agent, workboard, logbook, team-reports`, and schema versions state 28 / agent 31 — `package.json:5-20` [Verified]
- Rules: "State and caches: SQLite, not new JSON/JSONL/sidecar stores"; Kysely for ordinary SQL; synchronous write transactions; "Database access: worker threads… never Gateway main thread"; "Gateway owns every DB/state" — `AGENTS.md:66-70` [Verified]
- Schemas: `src/state/openclaw-state-schema.sql` (131 `CREATE TABLE`) and `src/state/openclaw-agent-schema.sql` (45), plus 47 generated Postgres catalog files in `src/state/postgres-schema.generated/` [Verified]
- "SQLite is the default; agent stores also support the experimental PostgreSQL engine… PostgreSQL shared-state generation requires PostgreSQL 16+" — `docs/reference/database-schemas.md` (intro) [Docs]. Config `database.engine` defaults to `"sqlite"` — `docs/reference/database-engines.md:70` [Docs]

---

## D12 — Surfaces

**In plain words.** You reach OpenClaw through chat apps (28 channel plugins in-repo plus several external catalog plugins), a browser Control UI/dashboard, a terminal TUI and CLI, native apps (macOS, iOS + Watch, Android + Wear, a Tauri app for Linux/Windows, and a separate native Windows Hub repo), voice ("Talk mode", wake word, phone calls, meeting bots), and a "Canvas"/widget surface the agent can draw into. It can also act as an MCP server and as an A2A agent.

### Chat channels (counted from manifests)
- **28 plugins declare `channels`** in `openclaw.plugin.json`: a2a, buzz, clickclack, discord, feishu, googlechat, imessage, irc, line, matrix, mattermost, msteams, nextcloud-talk, nostr, qa-channel, raft, reef, signal, slack, sms, synology-chat, telegram, tlon, twitch, whatsapp, x, zalo, zalouser — parsed from `extensions/*/openclaw.plugin.json` [Verified]
- 25 of these carry category `channels`. The other 3 are a2a ("A2A v1.0 Agent-to-Agent protocol channel plugin"), reef ("Guarded end-to-end encrypted claw channel", category agent-orchestration) and qa-channel (developer-tools test channel) [Verified]. So **user-facing messaging channels = 25** (24 excluding the X/Twitter bridge if one is strict) [Inference]
- External or catalog channels documented but not in the repo: WeChat (`openclaw-weixin`), QQ Bot (`@tencent-connect/openclaw-qqbot`), WeCom, Tencent Yuanbao (community plugin), Zalo ClawBot (`@zalo-platforms/openclaw-zaloclawbot`) — `docs/channels/wechat.md:2`, `docs/channels/qqbot.md:17-22`, `docs/channels/wecom.md:2`, `docs/channels/yuanbao.md:9-11`, `docs/channels/zaloclawbot.md:9` [Docs]
- WebChat is a Gateway-hosted static UI over the WS API, not a channel plugin — `docs/concepts/architecture.md:49-53` [Docs]; code: `src/gateway/server-methods/chat-webchat-media.ts`, `src/media/webchat-attachment.ts` [Verified]
- Channel safety default: DM-capable channels pair unknown senders by default (`openclaw pairing approve <channel> <code>`) — `README.md:79` [Docs]
- Self-assessed maturity (`qa/maturity-scores.yaml`, quality/completeness out of 100): discord M4 73/87; telegram M3 68/78; whatsapp, slack, imessage-bluebubbles M3 66/78; signal, google-chat, msteams M2 59/66; matrix M2 60/67; community-channels M2 53/54 [Verified]

### Native apps
- `apps/` contains android (942 files; includes `wear`, `wear-shared`), ios (644; includes `WatchApp`, `ShareExtension`, `ActivityWidget`), macos (773), linux (141; Tauri `src-tauri`), macos-mlx-tts, shared (`OpenClawKit`, `OpenClawMLXTTSProtocol`, `OpenClawWatchRTC`, `mermaid`), swabble — directory listing [Verified]
- Linux/Windows desktop: "a Tauri v2 shell for Linux and Windows… discovers nearby Gateways over Bonjour… opens the selected Gateway's Control UI, and stays available in the system tray" — `apps/linux/README.md:3` [Docs]
- Windows: native "Windows Hub" (WinUI) is the recommended app and lives in a separate repo, `openclaw/openclaw-windows-node` — `docs/platforms/windows.md:10-23` [Docs]. Self-score for "Windows App / Node": quality 19, completeness 21 ("Experimental") — `qa/maturity-scores.yaml:2087-2099` [Verified]
- macOS app updates via Sparkle-style appcasts (`appcast.xml`, `appcast-arm64.xml`, `appcast-x86_64.xml` at root; `scripts/make_appcast.sh`) [Verified]
- Self-scores: android M4 80/80, ios M4 80/80, macos-app M3 66/78, linux-app M3 71/76, watchos M1 41/44 — `qa/maturity-scores.yaml` [Verified]

### Web, CLI and TUI
- Control UI (Solid) served by the Gateway, opened with `openclaw dashboard` — `README.md:62-66` [Docs]. Docs describe session dashboards, widgets and `show_widget` — `docs/web/dashboard-architecture.md:594`, `docs/web/dashboards.md` [Docs]
- TUI: `src/tui/` (174 files) built on `@earendil-works/pi-tui` — `package.json:2334` [Verified]
- CLI binary `openclaw` → `openclaw.mjs` — `package.json:34-36` [Verified]

### Voice and Talk mode
- `src/talk/` (90 files: agent-consult, talkback, audio codec/energy, client voice sessions) [Verified]. 19 `talk.*` core RPCs and the `operator.talk` scope — `src/gateway/methods/core-descriptors.ts` [Verified]
- WebRTC Talk control is part of the protocol ("Gateway-controlled WebRTC Talk") — `docs/gateway/protocol.md:30,45` [Docs]
- 16 `voice` plugins: azure-speech, deepgram, elevenlabs, facetime, fish-audio-speech, google-meet, gradium, inworld, microsoft, senseaudio, slack-huddles, talk-voice, teams-meetings, tts-local-cli, voice-call, zoom-meetings [Verified]. Also `src/meeting-bot/` (107 files) and a local MLX TTS macOS helper app — `apps/macos-mlx-tts` [Verified]
- Voice wake (wake word) on nodes: `docs/nodes/voicewake.md`; `voicewake.*` RPCs (3) [Docs/Verified]

### Canvas and widgets
- `extensions/canvas` plugin (category documents-files). Gateway paths `/__openclaw__/canvas/` and `/__openclaw__/a2ui/`. macOS node `canvas.*` commands — `docs/concepts/architecture.md:18-22,44-45` [Verified/Docs]. Protocol schema `canvas.ts` and `canvas-limits.ts` in `packages/gateway-protocol/src` [Verified]

### Agent-to-agent and MCP surfaces
- `openclaw mcp serve` exposes channel conversations over MCP. It also manages outbound MCP server definitions (`list/show/status/doctor/probe/add/…`) — `docs/cli/mcp.md:2-14` [Docs]. 21 `mcp.*` core RPCs [Verified]
- A2A v1.0 channel plugin — `extensions/a2a/openclaw.plugin.json` [Verified]
- App SDK `@openclaw/sdk` (client, event-hub, chat-projection) — `packages/sdk/src/` [Verified]. Self-score app-sdk M2 53/51 — `qa/maturity-scores.yaml` [Verified]

---

## D13 — Extensibility

**In plain words.** Nearly everything beyond the core is a plugin: channels, model providers, memory, voice, web search, harnesses, tools. A plugin is a folder or npm package with an `openclaw.plugin.json` manifest. Code plugins run inside the Gateway (not sandboxed) and get an `api` object with about 60 `register*` methods. "Bundle" plugins can also be imported straight from Claude, Codex or Cursor plugin formats without running their code. Distribution and trust checks go through ClawHub (clawhub.ai), which enforces a SHA-256 integrity and security-verdict gate at install time.

### Plugin inventory (this commit)
- 182 entries in `extensions/`, of which **171 have `openclaw.plugin.json`** — `ls extensions/*/openclaw.plugin.json | wc -l` [Verified]
- By manifest `categories`: models 57 · channels 25 · voice 16 · developer-tools 9 · infrastructure 10 · web 10 · media 7 · security 7 · agent-orchestration 5 · agent-runtimes 5 · memory 5 · documents-files 4 · inbox-collaboration 2 · integrations 2 · productivity 2 · computer-use 1 · context 1 · data-analytics 1 · scheduling 1 · other 1 [Verified]
- Notable members:
  - agent-runtimes: acpx, agentsapi, codex, copilot, muse
  - memory: active-memory, memory-core, memory-lancedb, memory-wiki, voyage
  - integrations: migrate-claude, migrate-hermes
  - security: device-pair, mxc, onepassword, openshell, policy, vault, visitor-access
  - infrastructure: diagnostics-otel, diagnostics-prometheus, crabbox, mcp-relay, code-mode-quickjs, linux-node, bonjour, cloudflare
  - [Verified]
- Taxonomy is a shared contract: `PLUGIN_CATEGORY_SLUGS` (23 active slugs, plus legacy `tools/runtime/gateway`) — `packages/plugin-package-contract/src/categories.ts:1-30` [Verified]
- Memory is an exclusive slot: "only one memory plugin can be active at a time" — `VISION.md:117`. Manifest `kind: "memory"` appears in 2 plugins [Verified]

### Manifest (`openclaw.plugin.json`)
- Required in all 171: `id`, `categories`, `activation`, `configSchema` (JSON Schema). Common optional keys: `enabledByDefault` (98), `contracts` (96), `setup` (75), `providers` (63), `providerAuthChoices` (61), `configGroups` (56), `modelCatalog` (52), `uiHints` (51), `doctorContract` (44), `channels` (28), `cliCommands` (24), `modelPricing` (22), `skills` (15), `toolMetadata` (12), `backupResources` (6), `controlUi` (6), `qaRunners` (2), `requiresPlugins` (1) — parsed key counts [Verified]
- Example (Telegram): `categories:["channels"]`, a `doctorContract` with `configRepair` and named `stateMigrations`, `activation.onStartup:false`, `channels:["telegram"]` — `extensions/telegram/openclaw.plugin.json` [Verified]
- The design point is that control-plane questions get answered from metadata without running plugin code: "Setup screen needing plugin metadata → manifest/lightweight artifact, never plugin execution runtime" — `src/plugin-sdk/AGENTS.md:81` [Verified]
- The external npm package contract (`packages/plugin-package-contract`, 96-line `index.ts`): `validateExternalCodePluginPackageJson`, `EXTERNAL_CODE_PLUGIN_REQUIRED_FIELD_PATHS`, `normalizeExternalPluginCompatibility` — `packages/plugin-package-contract/src/index.ts:29-85` [Verified]

### Registration API (code plugins)
- **60 distinct `register*` members** on the plugin API types (`src/plugins/types*.ts`, `src/plugins/api*.ts`) [Verified]. They include registerChannel, registerProvider, registerTool, registerAgentHarness, registerContextEngine, registerCompactionProvider, registerMemoryCapability (and Memory corpus/prompt supplements), registerEmbeddingProvider, registerWebSearchProvider/WebFetchProvider, registerSpeech/RealtimeVoice/RealtimeTranscription providers, Image/Video/Music generation, registerMediaUnderstandingProvider, registerGatewayMethod, registerHttpRoute, registerGatewayAccessPolicy, registerHook, registerService, registerCli/CliBackend/Command, registerConfigMigration, registerMigrationProvider, registerSecurityAuditCollector, registerTrustedToolPolicy, registerAgentToolResultMiddleware, registerSessionAction/Catalog/Extension/SchedulerJob, registerStorageProvider, registerWorkerProvider, registerNodeHostCommand/NodeInvokePolicy, registerControlUiDescriptor, registerWidgetPresenter, registerCodexAppServerExtensionFactory [Verified]
- Signatures, for example: `registerTool` — `src/plugins/plugin-api.types.ts:239`, `registerHttpRoute` :248, `registerChannel` :257, `registerGatewayMethod` :269, `registerProvider` :324, `registerWebSearchProvider` :365, `registerContextEngine` :377, `registerAgentHarness` :389, `registerMemoryCapability` :504 [Verified]
- SDK surface: root `package.json` exports **357 `openclaw/plugin-sdk/*` subpaths** (of 359 exports). The `@openclaw/plugin-sdk` package itself is `0.0.0-private` with 58 exports [Verified]. "All public plugin SDK contracts are experimental" — `src/plugin-sdk/plugin-entry.ts:1` [Verified]
- SDK rules: narrow subpaths, no broad barrels, acyclic facades, versioned required capabilities, closure-bound host authority. Use the `plugin-sdk-impact` skill to query public ClawHub consumers before deprecations. Deprecated synchronous SQLite SDK members get removed in the first stable release of Jan 2027 — `src/plugin-sdk/AGENTS.md:22-69` [Verified]
- "Capability ladder" for new features: existing owner → existing plugin contract → narrow generic core/SDK capability → universal core surface — `src/plugin-sdk/AGENTS.md:71-79` [Verified]. Philosophy: "Recurring demand defines interfaces… the right response is a contract, not a queue of merges" — `VISION.md:98-99` [Verified]

### Two plugin styles and foreign bundle formats
- Code plugins vs bundle-style plugins (skills, MCP servers, config). "Prefer bundle-style plugins when they can express the capability" — `VISION.md:101-108` [Verified]
- Bundle formats recognized: `.codex-plugin/plugin.json`, `.claude-plugin/plugin.json`, `.cursor-plugin/plugin.json`, and generic `plugin.json` against the `https://agent-plugins.org/schemas/1.0.0/plugin.schema.json` schema — `src/plugins/bundle-manifest.ts:27-32` [Verified]. It also maps Cursor `.cursor/commands|agents|rules|hooks.json` and `.mcp.json` — `src/plugins/bundle-manifest.ts:162-178` [Verified]
- "Compatible bundles are normalized into registry records without importing runtime code." — `docs/plugins/architecture.md:117` [Docs]
- Native plugins: "run **in-process** with the Gateway. They are not sandboxed… same process-level trust boundary as core code." — `docs/plugins/architecture.md:962` [Docs]. SECURITY treats "a malicious plugin after a trusted operator installs" it as out of scope — `SECURITY.md:55,98` [Verified]

### Skills, ClawHub, registry
- 49 bundled skills in `skills/` (1password, apple-notes, coding-agent, github, notion, obsidian, tmux, weather, xurl…) [Verified]. New skills should go to ClawHub first — `VISION.md:122-124` [Verified]
- ClawHub client code: `src/infra/clawhub-*.ts` (client, packages, plugin-catalog, plugin-manifest, plugin-release, plugin-skills, promotions, skill-security, install-trust, integrity), `src/claws/clawhub-*`, and CLI confirmations `src/cli/clawhub-install-confirmation.ts` and `src/cli/non-clawhub-install-acknowledgement.ts` [Verified]
- Install trust gate: "Shared ClawHub exact-release trust gate for plugin, skill, and Claw installs" — `src/infra/clawhub-install-trust.ts:1`. It blocks on `blockedFromDownload`, scan status `malicious`, blocking moderation states, or a non-safe verdict — `src/infra/clawhub-install-trust.ts:96-136`. SHA-256 integrity normalization — `src/infra/clawhub-integrity.ts:2-24` [Verified]
- Installing from outside ClawHub needs an explicit acknowledgement — `src/cli/non-clawhub-install-acknowledgement.ts` exists [Verified; flow not traced]
- Self-score: plugins M3 72/79; clawhub M2 58/62 — `qa/maturity-scores.yaml` [Verified]
- Migration plugins that import other agents' setups: `migrate-claude`, `migrate-hermes` — manifests; docs `docs/install/migrating-claude.md`, `migrating-hermes.md` [Verified]

---

## D14 — Observability, evals, testing, QA

**In plain words.** The test investment is enormous: about 20,600 TypeScript test files holding ~116k `it/test` cases, more test code (6.3M LOC) than product code. On top of unit tests there is a QA Lab plugin that runs about 600 YAML scenarios against a real Gateway with a fake "qa-channel", plus Docker e2e lanes, live-provider tests and a 41-job CI workflow. Runtime observability is structured logs (tslog) plus optional OpenTelemetry and Prometheus exporter plugins and a large diagnostics subsystem (support bundles, stall flight recorder, heap profiles). They also publish a self-graded maturity scorecard, and it's candid: overall quality is graded "Alpha" (65/100).

### Test inventory
- `*.test.ts(x)` files: **20,596**. Regex-matched `it(`/`test(` calls: **~116,130**. `.skip(`: 3 (lexical count) [Verified]
- `*.e2e.test.ts`: 1,135 · `*.live.test.ts`: 148 · `*.integration*.test.ts`: 276 · `scripts/e2e/` files: 363 [Verified]
- Test files per area: src 11,613 · extensions 4,325 · ui 2,700 · test 1,520 · packages 484 [Verified]. Swift/Kotlin test-path files under apps: 957 [Verified]
- npm test lanes: 205 `test*` scripts, including 96 `test:docker:*`, 10 `test:live*`, `test:contracts:{channels,plugins}`, `test:coverage`, `test:changed` — `package.json` scripts [Verified]
- Policy: "Behavior over test count. Prefer production-path integration proof"; "Test failures = defects"; "Red `main` is an emergency" — `AGENTS.md:87-90` [Verified]. Prompt-cache stability is a guarded test: `src/agents/agent-tools.prefix-stability.test.ts` — `AGENTS.md:75` [Verified]

### QA Lab and scenarios (evals)
- `extensions/qa-lab`: "OpenClaw QA lab plugin with private debugger UI and scenario runner", adds the `qa` CLI, a `qa-lab-search` web-search provider, a `static-ssh` worker provider, and the tools `qa_restart_wait` and `qa_restart_unsafe_probe` — `extensions/qa-lab/openclaw.plugin.json` [Verified]
- `qa/scenarios/`: **601 files** (594 YAML + 7 JSONL replays) across 19 themes: channels 259, runtime 139, ui 35, plugins 28, scheduling 23, media 20, agents 16, models 12, personal 11, memory 10, observability 9, config 8, cli 7, jsonl-replay 7, workspace 5, goals 3, security 3, tools 3, character 2 [Verified]
- Scenario shape: `id`, `surface`, `coverage.primary/secondary` IDs, `objective`, `gatewayConfigPatch`, `successCriteria`, `docsRefs`, `codeRefs`, `execution.kind: flow` — e.g. `qa/scenarios/channels/a2a-allowlist-denied-cross-agent-send.yaml` [Verified]
- Workflow: `qa suite` (executable regression loop), `qa manual` (personality/style check), `qa coverage` (inventory) — `qa/README.md:13-16` [Verified]. Pooled live credentials go through a Convex lease broker — `qa/README.md:10` [Verified]
- `taxonomy.yaml` (11,663 lines) is the maturity-scorecard model. It defines profiles such as `smoke-ci`: "Deterministic PR and merge proof with mock model providers and the Crabline-backed channel driver" with explicit coverage IDs — `taxonomy.yaml:1-25` [Verified]
- `qa/maturity-scores.yaml`: 50 surfaces, 280 category scores. Surface averages quality 65 "Alpha", completeness 71 "Beta" — `qa/maturity-scores.yaml:1-20` [Verified]. Highest: cli 83/90, gateway 80/88, android/ios 80/80. Lowest: windows-app 19/21, watchos/nix/voice-call 41/44 [Verified]
- Docs also mention a "personal agent benchmark pack" and "frontier harness plan" — `docs/concepts/personal-agent-benchmark-pack.md`, `qa/frontier-harness-plan.md` [Docs; not read in full]

### CI
- 118 workflow files in `.github/workflows/` (67,020 YAML lines in total) [Verified]
- `ci.yml` is 6,820 lines with **41 jobs**: preflight, security-fast, build-artifacts, control-ui-performance, checks-ui(-e2e, -e2e-real-gateway), checks-baseline-ratchets, checks-fast-core, qa-smoke-ci-profile, plugin/channel contract shards, node-compat, check-plan/check-shard, lint/test-types shards, check-extension-package-boundary, **postgres-conformance**, check-docs, skills-python, checks-windows, macos-node, macos-swift, ios-build, iOS/Android release e2e and screenshot evidence, android, docker-seed-e2e, pr-failure-report, ci-gate — `.github/workflows/ci.yml:117-6612` [Verified]
- Runners: mostly Blacksmith (`blacksmith-4/8/16vcpu-ubuntu-2404`, `blacksmith-6vcpu-macos-15`) with GitHub-hosted fallbacks, chosen by long expressions. Xcode lane: `xcode-27-xlarge` — `ci.yml` runs-on expressions [Verified]
- Security workflows: codeql (4 variants), dependency-audit, security-review, zizmor config, dependabot — `.github/workflows/`, `.github/zizmor.yml` [Verified]
- Static guards as npm scripts: 41 `check:*` (architecture, import-cycles, max-lines-ratchet, line-cap-ratchet, database-worker-ratchet, env-var-count, protocol-coverage, test-timeout-race-ratchet…) and 48 `lint:*` (plugin/extension boundary rules such as `lint:plugins:no-extension-src-imports` and `lint:extensions:no-src-outside-plugin-sdk`, plus `lint:kysely` and `lint:tmp:sqlite-transaction-boundary`) — `package.json` scripts [Verified]
- Bots: ClawSweeper auto-reviews each PR once (`@clawsweeper re-review` asks again) — `AGENTS.md:95`. Barnacle is another review bot — `CONTRIBUTING.md:90,198` [Verified/Docs]

### Logging and diagnostics
- Logger: `tslog` — `src/logging/logger.ts:6`. Default dir is the POSIX tmp root, legacy file `openclaw.log` — `src/logging/logger.ts:41-42` [Verified]
- Rule: "Logs: write raw; redact only at egress (logs.tail, diagnostics/support bundles, remote/OTel export); never log message/tool content." — `AGENTS.md:61` [Verified]
- `src/logging/` (152 files) has diagnostics modules for CPU/heap profiles, heap snapshots, liveness, stall flight recorder, stuck-session recovery, tool-loop detection, support bundle/export with redaction — directory listing [Verified]
- Exporters as plugins: `diagnostics-otel` ("metrics, traces, and logs", OTLP proto exporters, `@opentelemetry/*` deps) and `diagnostics-prometheus` — `extensions/diagnostics-otel/openclaw.plugin.json`, `package.json` deps; `extensions/diagnostics-prometheus/openclaw.plugin.json` [Verified]. OTel transport accepts only `http/protobuf` — `src/config/schema.help.runtime.ts:417` [Verified]
- RPCs: `logs.tail`, `diagnostics.*` (5), `audit.*` (3, metadata-only audit ledger), `usage.status`/`usage.cost` — `src/gateway/methods/core-descriptors.ts` [Verified]. Audit doctrine: "provenance = opt-in, never authorization" — `AGENTS.md:127` [Verified]
- Smoke scripts `qa:otel:smoke`, `qa:prometheus:smoke`, `qa:observability:smoke` — `package.json` scripts [Verified]

---

## D15 — Deployment and ops

**In plain words.** Most people install with a one-line curl/PowerShell script or `npm i -g openclaw`, then run an onboarding wizard that installs the Gateway as a system service. Docker, docker-compose, Fly.io, Render, Kubernetes manifests and a separate Nix repo are all provided. Updates follow stable / extended-stable / beta / dev channels. Telemetry is off by default except for a daily version check, which you can turn off.

### Install paths
- `curl -fsSL https://openclaw.ai/install.sh | bash` (macOS/Linux/WSL2) and `iwr -useb https://openclaw.ai/install.ps1 | iex` (Windows). The installer provisions Node when needed — `README.md:26-36` [Docs]. Scripts are in the repo: `scripts/install.sh` (4,015 lines, uses `--proto '=https' --tlsv1.2`, may `sudo apt-get` build tools) — `scripts/install.sh:53,839-879`; `scripts/install.ps1` (2,491 lines) [Verified]
- npm: `npm install -g openclaw@latest --allow-scripts=openclaw` (npm 11.16+/12) — `README.md:41`. Lifecycle scripts: `postinstall: node scripts/postinstall-bundled-plugins.mjs`, `prepare: node scripts/prepare-git-hooks.mjs` — `package.json:1993,1998` [Verified]
- Onboarding: `openclaw onboard --install-daemon`, then `openclaw gateway status` and `openclaw dashboard` — `README.md:56-64` [Docs]
- Source: pnpm workspace only ("Plain `npm install` at the repository root is not supported") — `README.md:96` [Docs]
- Install guides in `docs/install/`: ansible, azure, bun, cloudflare, daytona, digitalocean, docker, exe-dev, fly, gcp, hetzner, hostinger, kubernetes, macos-vm, nix, northflank, oracle, podman, railway, raspberry-pi, render, standalone, upstash, plus migrating-claude/hermes [Verified, listing]

### Containers and PaaS
- `Dockerfile` (470 lines): multi-stage build. Base images are digest-pinned `node:24-bookworm`/`-slim` and `oven/bun:1.4.3`. Build args select bundled extensions (`OPENCLAW_EXTENSIONS`) and optional apt/pip packages, browser and Docker CLI. Runs as `USER node`, has a `HEALTHCHECK`, entrypoint `tini -s -- node /app/docker-entrypoint.mjs`, CMD `node openclaw.mjs gateway` — `Dockerfile:11-21,449,467-470` [Verified]
- `docker-compose.yml`: ports 18789 (gateway), 18790 (bridge), 3978 (MS Teams); volumes for `~/.openclaw`, workspace and an auth-profile secret dir — `docker-compose.yml:47-71` [Verified]
- Fly.io: `app = "openclaw"`, process `node dist/index.js gateway --allow-unconfigured --port 3000 --bind lan`, `force_https`, health check `/startupz`, 2 GB shared-cpu-2x, volume at `/data` — `fly.toml:4,18,22,33` [Verified]
- Render: `dockerCommand: node openclaw.mjs gateway --allow-unconfigured`, health `/startupz`, generated `OPENCLAW_GATEWAY_TOKEN`, 1 GB disk — `render.yaml:9-19` [Verified]
- Kubernetes: `scripts/k8s/manifests/{configmap,deployment,kustomization,pvc,service}.yaml`, `scripts/k8s/create-kind.sh`, `deploy.sh` [Verified]. Self-score kubernetes M2 55/61, containers M3 67/71 [Verified]
- Nix: first-party but external `openclaw/nix-openclaw` Home Manager module — `docs/install/nix.md` [Docs]. Self-score nix M1 41/44 [Verified]
- `deploy/` contains `fly.private.toml` and `lightpanda` [Verified]

### Updates
- Channels: `stable`, `extended-stable`, `beta`, `dev`. Extended-stable is package-only and never auto-applies — `src/config/schema.help.core.ts:77-82` [Verified]
- `update.checkOnStart` defaults to true. Setting it false disables Gateway and headless-node update checks, applies, and anonymous pings — `src/config/schema.help.core.ts:79-80` [Verified]
- `update.auto.enabled`: background auto-update for stable/beta package installs, **default false** — `src/config/schema.help.core.ts:81-82` [Verified]. Exception: packaged long-running headless nodes auto-update by default (opt out with false or `OPENCLAW_NO_AUTO_UPDATE=1`) — `src/config/schema.help.runtime.ts:313` [Verified]
- Doctor-based migrations: "When a config change makes existing user config invalid, the same change needs a doctor migration… `openclaw doctor --fix`" — `VISION.md:59-64` [Verified]. "Updates always work" is a stated product rule with an update implementation contract — `AGENTS.md:84` [Verified]
- `package.json` declares `updateAdmissionProtocol: 1` — `package.json:5` [Verified]

### Telemetry defaults (code)
- Endpoint `https://telemetry.openclaw.ai/api/latest-version`, overridable with `OPENCLAW_TELEMETRY_ENDPOINT` — `src/infra/telemetry.ts:25,141` [Verified]
- Feature statistics are sent only if `telemetry.enabled === true`, and never when `DO_NOT_TRACK` is set — `src/infra/telemetry.ts:112-113,190-194,325` [Verified]
- Schema help: "Shares enabled channel and provider names, plugin count, and recent session count with the daily update check. Disabled by default and always disabled when DO_NOT_TRACK=1." — `src/config/zod-schema.telemetry.ts:7-9` [Verified]
- VISION/README: no analytics unless the operator opts in. The wizard offers stats "with no selected by default". The daily update check reports version and platform — `VISION.md:80-84`, `README.md:20` [Verified]
- ClawHub has its own telemetry module: `src/infra/clawhub-telemetry.ts` [Verified exists; not traced]

---

## D16 — Workspace-harness practices in their own repo

**In plain words.** The repo is built to be worked on by AI agents. A dense root `AGENTS.md` (134 lines but 21 KB) sets design rules. 27 more scoped `AGENTS.md` files sit next to the code they govern. 53 maintainer "skills" in `.agents/skills` (also exposed as `.claude/skills`) encode workflows like PR landing, testing policy and releases. Claude Code gets a SessionStart hook. CI ratchets and boundary lints enforce what the prose asks for. AI-written PRs are explicitly welcome with no disclosure required, but they must show evidence and run an `autoreview` skill.

### Instruction files
- Root `AGENTS.md`: 134 lines / 21,242 bytes. Sections: Choose good compromises · Design priorities · Working agreement · Execution discipline · One owner, complete cutover · Runtime and code safeguards · Product and validation · Execution gotchas · Authority and safety · Read when relevant — `AGENTS.md:3-134` [Verified]
- Notable rules:
  - "Design for real failures… No engineering for millisecond races" — `AGENTS.md:7`
  - "One owner per responsibility" — `:19`
  - "Small core, capable plugins" — `:20`
  - "Stable conversation context… only compaction rewrites history" (prompt-cache prefix stability) — `:21`, `:75`
  - "Two identical tooling failures… or 10 min without progress: switch to an authorized supported alternative" — `:42`
  - "Time pressure never waives gates" — `:45`
  - "no agent-attribution trailers" — `:104`
  - "Never switch models to bypass refusals" — `:105`
  - "Untrusted contributor/fork code: only secretless isolation, never locally" — `:106`
  - [Verified]
- Scoped guidance: **28 repo-guidance `AGENTS.md`** (33 tracked, minus 5 product templates under `docs/reference/templates/`). Largest: root 21.2 KB, `ui/` 14.6 KB, `src/gateway/` 9.5 KB, `extensions/telegram/` 8.4 KB, `scripts/` 7.5 KB, `src/plugin-sdk/` 7.3 KB, `apps/ios/` 6.7 KB. Smallest: `src/gateway/server-methods/` 312 B [Verified]
- `CLAUDE.md`: 4 files, all symlinks to the sibling `AGENTS.md` (git mode 120000), e.g. `extensions/ollama/CLAUDE.md -> AGENTS.md` [Verified]. No root `CLAUDE.md` [Verified]
- Root-level rule: "Read nearest scoped `AGENTS.md`… Update instructions at owner; no competing rules here." — `AGENTS.md:14-15` [Verified]
- Product templates for multi-agent roles (coordinator/researcher/reviewer/writer workspace `AGENTS.md`) ship in `docs/reference/templates/roles/*` [Verified]

### Agent skills and tool config
- `.agents/skills/`: **53 skills** (8,623 lines of SKILL.md). Examples: openclaw-pr-maintainer, openclaw-testing, autoreview, pr-simplify, clawsweeper, security-triage, release-openclaw-*, openclaw-docker-e2e-authoring, crabbox, discord-e2e, slack-e2e, telegram-e2e-userbot, plugin-sdk-impact, test-audit, update-team-server, solid, proof-video [Verified]
- `.claude/skills -> ../.agents/skills` symlink. `.claude/settings.json` adds a SessionStart hook running `node scripts/worktree-setup.mjs source` — `.claude/settings.json` [Verified]
- `.github/codex/` and `.github/instructions/` dirs exist (Codex/Copilot config) [Verified, listing only]

### Contribution rules (AI PRs)
- "AI/Vibe-Coded PRs Welcome! … No AI-assistance label or disclosure is required." Checklist: Evidence section, "Confirm you understand what the code does", run `autoreview`, follow the review flow — `CONTRIBUTING.md:215-226` [Verified]
- "For agent-authored or otherwise non-trivial work, create or reuse the issue first" — `CONTRIBUTING.md:38` [Verified]
- Hard cap of **20 open PRs per author**, enforced by auto-close — `CONTRIBUTING.md:44` [Verified]. PRs over ~5,000 changed lines are reviewed only in exceptional cases — `VISION.md:53` [Verified]
- No refactor-only PRs unless requested. No PRs that only fix CI already red on main (auto-closed) — `CONTRIBUTING.md:109-111` [Verified]
- PR template sections: What Problem This Solves / User Impact / Why This Change Was Made / Evidence — `.github/pull_request_template.md:21-49` [Verified]
- Visual changes need inspected before/after screenshots — `AGENTS.md:93`, `CONTRIBUTING.md:115` [Verified]
- Security-sensitive changes (auth, credentials, sandboxing, exec permissions) need a maintainer `/allow-security-sensitive-change` comment. Dependency changes need `/allow-dependencies-change`. "GitHub App bot identities do not qualify." SecOps code-owner approval is needed for security policy/CodeQL/guard files — `CONTRIBUTING.md:118-156` [Verified]
- CHANGELOG is release-owned and generated, so PRs must not edit it — `CONTRIBUTING.md:92`, `AGENTS.md:131` [Verified]
- After a CI failure: `pnpm test:ci-failed <pr>` classifies failures. "Do not rerun CI just to chase green" — `CONTRIBUTING.md:94` [Verified]
- Pre-commit hook (`git-hooks/pre-commit`) formats code and can block private literals via `hooks.blockedLiteralsFile` — `CONTRIBUTING.md:161-194` [Verified]
- Security reports: "Given the volume of AI-generated scanner findings, we need reports that researchers have checked and understand." — `CONTRIBUTING.md:285` [Verified]

### CI guards that back the prose
- Ratchets: `check:max-lines-ratchet`, `check:line-cap-ratchet`, `check:database-worker-ratchet` (stops new main-thread DB access, matching `AGENTS.md:68`), `check:database-dialect-ratchet`, `check:test-timeout-race-ratchet`, `check:env-var-count`; plus `checks-baseline-ratchets` CI job — `package.json` scripts, `ci.yml:2763` [Verified]
- Boundary lints enforce "plugins use documented `openclaw/plugin-sdk/*` contracts… never core internals" (`AGENTS.md:63`): `lint:plugins:no-extension-imports`, `lint:extensions:no-src-outside-plugin-sdk`, `lint:extensions:no-relative-outside-package`, the `check-extension-package-boundary` job — `package.json`, `ci.yml:4466` [Verified]
- `check:protocol-coverage`, `check:architecture`, `check:import-cycles`/`madge`, `check:no-conflict-markers` — `package.json` [Verified]
- `dated-todo-sweep.yml`, `duplicate-after-merge.yml`, `stale.yml`, `labeler.yml` workflows [Verified]

---

## Distinctive ideas (worth stealing)
1. **One daemon, one typed WS protocol for everything** (operators, devices, workers), with roles, scopes, a signed nonce handshake and an N-1 node version window. 526 core methods sit in a single append-only policy table that drives listing, authorization, startup availability and write throttling — `src/gateway/methods/core-descriptors.ts:16-45` [Verified]
2. **Manifest-first plugins**: control-plane questions (setup UI, auth choices, model catalog, pricing, doctor migrations, backup resources) are answered from `openclaw.plugin.json` without loading plugin code — `src/plugin-sdk/AGENTS.md:39,81` [Verified]
3. **Imports foreign plugin ecosystems as data**: Claude, Codex and Cursor bundle formats plus the agent-plugins.org schema — `src/plugins/bundle-manifest.ts:27-32` [Verified]
4. **Harness-agnostic agent runtime**: Codex app-server, ACP (acpx), Copilot and pi are swappable "agent-runtimes" plugins via `registerAgentHarness` — manifests, `plugin-api.types.ts:389` [Verified]
5. **Prompt-prefix stability as a tested invariant** (cache economics): `agent-tools.prefix-stability.test.ts` — `AGENTS.md:21,75` [Verified]
6. **Doctor-owned migrations instead of runtime compat shims** — `VISION.md:59-64`, `AGENTS.md:65` [Verified]
7. **Scenario-YAML QA with coverage IDs + a self-published maturity scorecard** (taxonomy → scores). This is honest product telemetry for a roadmap — `qa/`, `taxonomy.yaml` [Verified]
8. **"Core tax" doctrine**: every core tool/prompt line costs every request, so the bar for core is high and the bar for plugins is low — `VISION.md:93-96` [Verified]
9. **Devices as tool hosts** (camera, screen, location, canvas, computer-use, node-exec) through paired nodes, including a Rust headless node host — `docs/concepts/architecture.md:39-45`, `crates/openclaw-node-host` [Verified]
10. **Migration-in plugins** (`migrate-claude`, `migrate-hermes`) that lower switching cost from competitors [Verified]

## Weaknesses and risks
- **Size and complexity.** ~57k files, ~4.9M non-test TS LOC, 526 core RPCs, 60 registration hooks, 357 SDK subpaths, 602 npm scripts, 118 workflows (ci.yml alone is 6,820 lines with runner expressions over 1,000 characters). AGENTS.md itself warns "machinery nobody can follow fails in ways nobody predicted" — `AGENTS.md:10` [Verified counts; Inference on risk]
- **Attack surface.** Native plugins run in-process and unsandboxed (`docs/plugins/architecture.md:962`). Tools run on the host for the main session by default (`README.md:81`). A shared-secret bearer token on `/tools/invoke` = full operator (`SECURITY.md:103`). 25+ inbound chat channels feed untrusted text to an agent with host tools. The trust model puts multi-tenant and prompt-injection-only chains out of scope (`SECURITY.md:5-7`) [Verified]
- **The SDK is officially "experimental"** even though third parties depend on it (`src/plugin-sdk/plugin-entry.ts:1`). There is an active deprecation schedule, e.g. synchronous SQLite SDK members removed Jan 2027 (`src/plugin-sdk/AGENTS.md:69`) [Verified]
- **Self-graded Alpha quality** (65/100 average). Weak spots: Windows app 19/21, watchOS/Nix/voice-call 41/44, app-sdk 53/51, ClawHub 58/62 — `qa/maturity-scores.yaml` [Verified]
- **Doc drift.** CONTRIBUTING still describes a Lit Control UI while the code is Solid 2 RC — `CONTRIBUTING.md:202` vs `ui/package.json:53` [Verified]
- **Bleeding-edge pins.** Solid 2.0.0-rc.14, baileys 7.0.0-rc14 (WhatsApp via an unofficial protocol library), a pinned Bun fork, Node ≥24.16 only — `ui/package.json:53`, `package.json:2431`, `CONTRIBUTING.md:84-85` [Verified]
- **Release velocity.** About 10 releases a month plus "Red main is an emergency" policies mean a high upgrade burden for operators. headless nodes auto-update by default — `CHANGELOG.md`, `schema.help.runtime.ts:313` [Verified/Inference]
- **Single-owner Gateway.** One process per host owns every DB, so it is a single point of failure. Postgres is "experimental" and only some families support it — `docs/reference/database-schemas.md`, `database-engines.md:70` [Docs]
- **Installer weight.** `curl | bash` script of 4,015 lines that may use sudo/apt. Downloads are over TLS from openclaw.ai — `scripts/install.sh` [Verified]
- **Cannot verify locally**: history, contributor stats and star counts (shallow clone) [Context]

## What a data engineer would care about
- **Storage.** SQLite by default, with a shared state DB (131 tables, schema v28) plus one DB per agent (45 tables, v31). Experimental Postgres 16+ for the families `state, agent, workboard, logbook, team-reports`. Kysely typed queries, synchronous transactions, worker-thread DB access, integrity checks once per load — `package.json:5-20`, `src/state/*.sql`, `AGENTS.md:66-69` [Verified]
- **CI covers Postgres.** A `postgres-conformance` job exists — `ci.yml:4508` [Verified]
- **Typed event stream.** The WS protocol pushes `agent`, `chat`, `presence`, `health`, `cron`, `sessions.changed` and similar events with `seq`/`stateVersion`. JSON Schema is generated from TypeBox, so ingestion contracts can be machine-checked. Events are *not* replayed, so a CDC consumer must resync on gaps — `docs/concepts/architecture.md:31,82,175` [Docs]
- **Observability export.** OTLP (http/protobuf) metrics/traces/logs and a Prometheus exporter, via plugins. Raw logs are redacted at egress — `extensions/diagnostics-*`, `AGENTS.md:61` [Verified]
- **Audit and usage data.** `audit.*` ledger RPCs (metadata-only), `usage.status`/`usage.cost`, and manifest `modelPricing` in 22 provider plugins, which allows cost accounting per model — `core-descriptors.ts`, manifests [Verified]
- **Analytics-style plugins.** `team-reports` ("Daily, weekly, and monthly team activity reports from GitHub and Discord, with model-written summaries"), `logbook` (screen-snapshot work journal), `workboard` — manifests [Verified]
- **Programmatic access.** `@openclaw/sdk` (event hub, chat projection), `@openclaw/gateway-protocol` validators, a Rust `openclaw-gateway-client` crate, `openclaw mcp serve`, and the `POST /tools/invoke` HTTP API — `packages/sdk/src`, `crates/`, `docs/cli/mcp.md`, `SECURITY.md:103` [Verified/Docs]
- **Gap.** There are no data-warehouse, SQL-engine or dbt-specific plugins in the 171 manifests. The only `data-analytics` category member is `team-reports` [Verified]. Integrations would need MCP servers, skills or custom tools [Inference]
