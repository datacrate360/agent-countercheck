# Research tracker

Live status of every research subagent. Each analyst reads one repo's source code (a shallow clone at a pinned
commit), follows [RUBRIC.md](RUBRIC.md), and writes one cited report. This file is updated when an analyst starts
and when its report lands.

Status values: `running` · `done` · `failed` · `reviewing` (I'm spot-checking the report's citations).

## Wave 1: agents (started 2026-10-18 08:44 CDT)

| # | Repo @ commit | Scope | Analyst | Status | Report |
|---|---|---|---|---|---|
| 1 | openclaw/openclaw @ 15ae101a | A: core runtime (loop, providers, harness plugins, context) | sa-0-c124ff6c | running | `agents/openclaw/A-runtime.md` |
| 2 | openclaw/openclaw @ 15ae101a | B: tools, sandbox, security | sa-1-2e93c9b8 | running | `agents/openclaw/B-tools-security.md` |
| 3 | openclaw/openclaw @ 15ae101a | C: memory, skills, orchestration | sa-2-b845864a | done | [agents/openclaw/C-memory-orchestration.md](agents/openclaw/C-memory-orchestration.md) |
| 4 | openclaw/openclaw @ 15ae101a | D: platform, surfaces, ops, repo practices | sa-3-159c659d | done | [agents/openclaw/D-platform.md](agents/openclaw/D-platform.md) |
| 5 | odysseus-dev/odysseus @ 9712a54 | A: agent core | sa-4-865705c5 | done | [agents/odysseus/A-agent-core.md](agents/odysseus/A-agent-core.md) |
| 6 | odysseus-dev/odysseus @ 9712a54 | B: platform | sa-5-c4cfec3a | done | [agents/odysseus/B-platform.md](agents/odysseus/B-platform.md) |
| 7 | NousResearch/hermes-agent @ v0.22.2 (7b77e29) | full 18-point profile | sa-6-8c96451d | done | [agents/hermes/REPORT.md](agents/hermes/REPORT.md) |

## Wave 2: building blocks (started 2026-10-18 ~09:00 CDT)

| # | Repo @ commit | License | Focus | Analyst | Status | Report |
|---|---|---|---|---|---|---|
| 8 | thedotmack/claude-mem @ b85cd12 | Apache-2.0 | capture → compress → inject memory pipeline | sa-0-f122848f | running | `tools/claude-mem/REPORT.md` |
| 9 | Graphify-Labs/graphify @ 86c57be (v8) | Apache-2.0 | code/doc knowledge graph, the "80% fewer tokens" claim | sa-1-b3ea6542 | running | `tools/graphify/REPORT.md` |
| 10 | 666ghj/MiroFish @ 7657031 | **AGPL-3.0** | swarm simulation / forecasting | sa-2-240f8849 | running | `tools/mirofish/REPORT.md` |
| 11 | thesysdev/openui @ 8d9c4c3 | MIT | generative UI language and streaming renderer | sa-3-ebb4aa06 | running | `tools/openui/REPORT.md` |
| 12 | paperclipai/paperclip @ c9669a6 | MIT | AI-agent company and work management (closest to Agile) | sa-4-24f1fb5f | running | `tools/paperclip/REPORT.md` |
| 13 | usestrix/strix @ a0f8ea2 | Apache-2.0 | AI penetration-testing agent | sa-5-027b1408 | running | `tools/strix/REPORT.md` |
| 14 | morluto/rea @ 23f984b | MIT | reverse-engineering MCP | sa-6-3d4dd82e | running | `tools/rea/REPORT.md` |
| 15 | microsoft/markitdown @ 4cc9fa1 + guillaumemeyer/watermarks-remover @ 980c0b4 | MIT + MIT | document ingestion; AI-watermark detection/removal (+ diff vs the local `ai-watermark-remover` copy) | sa-7-be93a8de | running | `tools/markitdown/REPORT.md`, `tools/watermarks-remover/REPORT.md` |

## Wave 3: synthesis (after every report is in)

| # | Deliverable | Status |
|---|---|---|
| 16 | `synthesis/agents-compared.md`: Hermes vs OpenClaw vs Odysseus, feature by feature (who has what, shared principles) | pending |
| 17 | `synthesis/capability-map.md`: every capability across the 12 repos → one omni-agent module, with build/borrow/skip for each | pending |
| 18 | `synthesis/architecture.md`: proposed omni-agent architecture for the Agile lifecycle, interface contracts, license plan, build order | pending |
| 19 | Rendered HTML report in the tutorial format (plain-words boxes + SVG diagrams) | pending |

## Notes

- Clones are shallow and read-only. Nothing was installed or run, and no scans were executed.
- License watch: MiroFish and Odysseus are **AGPL-3.0**. Copying their code into an MIT platform isn't allowed without
  re-licensing; see each report's T13 for the options.
- The watermark-removal tool gets a legal/ethics review before anything goes into the platform.
