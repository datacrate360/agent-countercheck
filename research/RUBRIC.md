# Research rubric

Every report answers the same questions so reports can be compared and merged. Each factual line ends with a
citation (`path:line` at the pinned commit, or a docs URL) and one tag:

- **[Verified]**: read in the source code
- **[Docs]**: from the project's documentation only
- **[Inference]**: the analyst's reasoning, not a fact from the code

Every section starts with **"In plain words"**: 2 to 4 sentences anyone can follow.

## A. Agents (Hermes, OpenClaw, Odysseus): 18 dimensions

| ID | Dimension |
|---|---|
| D1 | Identity: purpose, license, governance, stack, size |
| D2 | Architecture and process model |
| D3 | Agent loop: turns, stop conditions, budgets, streaming, interrupts, parallel tools, retries |
| D4 | Models and providers: routing, fallbacks, local models, tool-call parsing, prompt caching |
| D5 | Prompt and context assembly, compaction |
| D6 | Tools catalog, MCP, ACP |
| D7 | Execution environment and sandboxing |
| D8 | Safety and security: approvals, injection defenses, secrets, auth |
| D9 | Memory and state |
| D10 | Skills and self-improvement |
| D11 | Multi-agent work and orchestration: subagents, boards, cron, goals, loops |
| D12 | Surfaces: CLI, TUI, desktop, mobile, chat channels, voice |
| D13 | Extensibility: plugins, SDK, hooks |
| D14 | Observability, evals, tests, CI |
| D15 | Deployment and operations, telemetry |
| D16 | Practices the project uses on its own repo (AGENTS.md etc.) |
| D17 | Distinctive ideas, weaknesses, risks |
| D18 | Relevance to a data engineer |

## B. Building-block tools (claude-mem, graphify, MiroFish, OpenUI, Paperclip, Strix, MarkItDown, REA, watermarks-remover): 14 dimensions

| ID | Dimension |
|---|---|
| T1 | Identity: purpose, license, stack, size, pinned commit, maturity, company or cloud upsell |
| T2 | **Core mechanism**: how it works at the code level, step by step, with the key algorithms and data flow |
| T3 | Architecture and process model |
| T4 | Data model: storage, file formats, schemas |
| T5 | Interfaces: CLI, Python/TS API, MCP tools, HTTP, hooks, skills (exact names) |
| T6 | How it plugs into agents: which agents and which mechanism |
| T7 | LLM usage: where models are called, providers, cost drivers, whether it works offline |
| T8 | Security, privacy, safety, legal and ethical points |
| T9 | Quality: tests, benchmarks (re-check the README's claims), CI |
| T10 | Operations: install, dependencies, footprint |
| T11 | What is actually new vs. marketing (check every headline claim) |
| T12 | Weaknesses, limits, risks |
| T13 | **How it fits into omni-agent**: its role in the Agile lifecycle, how to use it (as a library, over MCP, by porting the idea, or skip it), license compatibility with omni-agent's MIT license, effort, and a proposed interface contract |
| T14 | Open questions |

## C. The Agile lifecycle that omni-agent is meant to cover

Discover (research, reverse engineering, forecasting) → Plan (backlog, sprints, estimates) → Build (coding agents) →
Verify (tests, security testing) → Review and release → Operate (incidents) → Learn (memory, knowledge graph).
Cross-cutting: document ingestion, generated UI, provenance, team and agent communication.
