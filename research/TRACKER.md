# Research tracker

Live status of every research subagent. Each analyst reads one repo's source code (a shallow clone at a pinned
commit), follows [RUBRIC.md](RUBRIC.md), and writes one cited report. This file is updated when an analyst starts
and when its report lands.

Status values: `running` · `done` · `failed` · `reviewing` (I'm spot-checking the report's citations).

## Wave 1: agents (started 2026-10-18 08:44 CDT, all 7 done by 09:01)

| # | Repo @ commit | Scope | Analyst | Status | Report |
|---|---|---|---|---|---|
| 1 | openclaw/openclaw @ 15ae101a | A: core runtime (loop, providers, harness plugins, context) | sa-0-c124ff6c | done | [agents/openclaw/A-runtime.md](agents/openclaw/A-runtime.md) |
| 2 | openclaw/openclaw @ 15ae101a | B: tools, sandbox, security | sa-1-2e93c9b8 | done | [agents/openclaw/B-tools-security.md](agents/openclaw/B-tools-security.md) |
| 3 | openclaw/openclaw @ 15ae101a | C: memory, skills, orchestration | sa-2-b845864a | done | [agents/openclaw/C-memory-orchestration.md](agents/openclaw/C-memory-orchestration.md) |
| 4 | openclaw/openclaw @ 15ae101a | D: platform, surfaces, ops, repo practices | sa-3-159c659d | done | [agents/openclaw/D-platform.md](agents/openclaw/D-platform.md) |
| 5 | odysseus-dev/odysseus @ 9712a54 | A: agent core | sa-4-865705c5 | done | [agents/odysseus/A-agent-core.md](agents/odysseus/A-agent-core.md) |
| 6 | odysseus-dev/odysseus @ 9712a54 | B: platform | sa-5-c4cfec3a | done | [agents/odysseus/B-platform.md](agents/odysseus/B-platform.md) |
| 7 | NousResearch/hermes-agent @ v0.22.2 (7b77e29) | full 18-point profile | sa-6-8c96451d | done | [agents/hermes/REPORT.md](agents/hermes/REPORT.md) |

## Wave 2: building blocks (started 2026-10-18 ~09:00 CDT; all 9 done by 09:35)

| # | Repo @ commit | License | Focus | Analyst | Status | Report |
|---|---|---|---|---|---|---|
| 8 | thedotmack/claude-mem @ b85cd12 | Apache-2.0 | capture → compress → inject memory pipeline | sa-0-f122848f | done | [tools/claude-mem/REPORT.md](tools/claude-mem/REPORT.md) |
| 9 | Graphify-Labs/graphify @ 86c57be (v8) | Apache-2.0 | code/doc knowledge graph, the "80% fewer tokens" claim | sa-1-b3ea6542 | done | [tools/graphify/REPORT.md](tools/graphify/REPORT.md) |
| 10 | 666ghj/MiroFish @ 7657031 | **AGPL-3.0** | swarm simulation / forecasting | sa-2-240f8849 | done | [tools/mirofish/REPORT.md](tools/mirofish/REPORT.md) |
| 11 | thesysdev/openui @ 8d9c4c3 | MIT | generative UI language and streaming renderer | sa-3-ebb4aa06 | done | [tools/openui/REPORT.md](tools/openui/REPORT.md) |
| 12 | paperclipai/paperclip @ c9669a6 | MIT | AI-agent company and work management (closest to Agile) | sa-4-24f1fb5f | done | [tools/paperclip/REPORT.md](tools/paperclip/REPORT.md) |
| 13 | usestrix/strix @ a0f8ea2 | Apache-2.0 | AI penetration-testing agent | sa-5-027b1408 → sa-0-b949a489 → coordinator | done, **reduced depth**: both analysts were stopped by the provider's content filter while reading the pentest knowledge packs; the coordinator wrote an integration profile from docs, config and layout only (22 citations, each matched to its text) | [tools/strix/REPORT.md](tools/strix/REPORT.md) |
| 14 | morluto/rea @ 23f984b | MIT | reverse-engineering MCP | sa-6-3d4dd82e | done | [tools/rea/REPORT.md](tools/rea/REPORT.md) |
| 15 | microsoft/markitdown @ 4cc9fa1 + guillaumemeyer/watermarks-remover @ 980c0b4 | MIT + MIT | document ingestion; AI-watermark detection/removal (+ diff vs the local `ai-watermark-remover` copy) | sa-7-be93a8de | done (both reports written; the agent's final message was cut off by a content filter) | [tools/markitdown/REPORT.md](tools/markitdown/REPORT.md), [tools/watermarks-remover/REPORT.md](tools/watermarks-remover/REPORT.md) |

## Wave 3: synthesis (after every report is in)

| # | Deliverable | Status |
|---|---|---|
| 16 | `synthesis/agents-compared.md`: Hermes vs OpenClaw vs Odysseus, feature by feature (who has what, shared principles) | pending |
| 17 | `synthesis/capability-map.md`: every capability across the 12 repos → one omni-agent module, with build/borrow/skip for each | pending |
| 18 | `synthesis/architecture.md`: proposed omni-agent architecture for the Agile lifecycle, interface contracts, license plan, build order | pending |
| 19 | Rendered HTML report in the tutorial format (plain-words boxes + SVG diagrams) | pending |
| 20 | `synthesis/tutorial-audit-hermes.md`: every Hermes claim in agent-harness-tutorial vs source (sa-0-0548e8ad) | done: ~150 claims, 0 broken anchors, 3 wrong / 1 outdated / 19 imprecise / 14 omissions; **applied to the tutorial** |

## Citation check (wave 1)

`python3 research/scripts/check_citations.py <report> <clone> [--ref <tag>]` checks every `path:line` citation: the file
exists and the line is inside it. Short names resolve when exactly one tracked file matches.

| Report | Citations | Resolve to a real file and line | Left over |
|---|---|---|---|
| openclaw/A-runtime | 194 | 193 | 1 ambiguous short name (`run-loop.ts`, 2 files; full path given nearby) |
| openclaw/B-tools-security | 273 | 263 | 10 ambiguous short names (`config.ts`, 57 files; full path in the same bullet) |
| openclaw/C-memory-orchestration | 175 | 174 | 1 ambiguous short name |
| openclaw/D-platform | 195 | 195 | 0 |
| odysseus/A-agent-core | 264 | 264 | 0 |
| odysseus/B-platform | 260 | 260 | 0 |
| hermes/REPORT (@v0.22.2) | 247 | 221 | 26 ambiguous doc names (`agent-loop.md`/`architecture.md` also exist as zh-Hans translations) |

Hand-checked headline values: OpenClaw exec defaults `full`/`off` (`src/infra/exec-approvals-config.ts:94-95`), dreaming
promotion `score>=0.75, recalls>=3, uniqueQueries>=3` (`extensions/memory-core/src/dreaming-command.ts:22`), fallback
blocked after delivery/side effect (`src/agents/embedded-agent-runner/run-entry.ts:226-237`); Odysseus
`MAX_AGENT_ROUNDS = 20` (`src/agent_tools/__init__.py:86`); Hermes curator 14/30 days and `max_turns: None`. All match.

## Citation check (wave 2)

The checker now also tells *ambiguous short names* (several files share the name; the line fits at least one of them)
apart from real failures.

| Report | Citations | Exact | Ambiguous short name | Failed |
|---|---|---|---|---|
| tools/claude-mem | 146 | 146 | 0 | 0 |
| tools/graphify | 184 | 173 | 11 | 0 |
| tools/mirofish | 114 | 114 | 0 | 0 |
| tools/openui | 137 | 125 | 12 | 0 |
| tools/paperclip | 226 | 154 | 72 | 0 |
| tools/rea | 397 | 381 | 16 | 0 |
| tools/markitdown | 170 | 156 | 13 | 1 → fixed (`tests.yml:12-108` → `12-102`) |
| tools/watermarks-remover | 102 | 89 | 13 | 0 |

Hand-checked: Paperclip registers exactly 16 adapters (`server/src/adapters/registry.ts:903-922`); MiroFish rejects a
self-hosted Zep URL (`backend/app/config.py:71-72`); OpenUI's chat calls `window.open(url, "_blank")` with no URL check
(`react-ui/.../OpenUIChat/utils/actionMessage.ts:28`); claude-mem worker port `37700 + uid % 100`
(`src/shared/SettingsDefaultsManager.ts:414`); graphify tries Leiden, falls back to Louvain (`graphify/cluster.py:106-112`).

### Limit of the citation checker

`check_citations.py` proves that a cited file exists and that the line number is inside it. It does **not** prove that
the line says what the report claims. That is why each wave also has hand-checked headline values. While writing the
Strix profile, 13 `AGENTS.md` line numbers passed the checker but pointed at the wrong lines; they were corrected by
matching the quoted text.

## Corrections the research produced

- Hermes curator: skills go stale after **14** days and are archived after **30** (`hermes_cli/config_defaults.py:1502-1503`),
  not 30/90. Fixed in the agent-harness-tutorial (§6).
- Hermes default turn limit is **unlimited** (`agent.max_turns: None`, `config_defaults.py:78`); "500" in some docs is out of
  date. Fixed in the tutorial (§5).
- Hermes `delegation.max_iterations` is 250 in code and the user docs; only `agent-loop.md` and a docstring say 50.

## Notes

- Clones are shallow and read-only. Nothing was installed or run, and no scans were executed.
- License watch: MiroFish and Odysseus are **AGPL-3.0**. Copying their code into an MIT platform isn't allowed without
  re-licensing; see each report's T13 for the options.
- The watermark-removal tool gets a legal/ethics review before anything goes into the platform.
