# Hermes Agent v0.22.2 — profile

> Subject: **NousResearch/hermes-agent**, tag `v0.22.2` (commit `d923a5eb48`, tagged 2026-10-15 01:52 -0400, "Release 0.22.2 from release/0.22.1 with the published v0.22.1 as a parent").
> The brief's report template says "Claude Code"; the repo profiled here is **Hermes Agent**. The heading names the actual subject so the comparison stays apples-to-apples.
> Method: a read-only `git archive v0.22.2` export to scratch (`…/cache/scratch/hsrc`). The live checkout (v0.22.0 `e06df9c2`) was not modified. Every citation is `path:line@v0.22.2` from that export. Counts come from `git ls-tree`/`find`/`grep`, or from importing modules with the installed venv against the exported tree.
> Tags: **[Verified]** = read in source at the cited line · **[Docs]** = website/docs or AGENTS.md claim only · **[Inference]** = my reading, not stated.
> Not done: nothing was built, run or benchmarked. Test counts are lexical, not executed. The local clone is **shallow** (`git rev-list --count v0.22.2` = 784, earliest commit 2026-10-13), so project age, total commits and contributor counts **cannot be derived locally**.

---

## D1 — Identity, license, governance, stack, size

**In plain words.** Hermes is Nous Research's MIT-licensed personal AI agent. It is mostly Python, with TypeScript front-ends (an Ink TUI, an Electron desktop app and a web dashboard). The repo is very large, at 18.6K tracked files. Tests make up a third of the files, and the Electron app is the largest single code area.

- Name `hermes-agent`; `pyproject.toml` version is a placeholder `0.0.0` (the release version comes from tags). `requires-python = ">=3.11,<3.15"`. `pyproject.toml:6-17@v0.22.2` [Verified]
- License MIT, "Copyright (c) 2025 Nous Research". `LICENSE:1-3@v0.22.2`, `pyproject.toml:17-18@v0.22.2` [Verified]
- Self-description: "a personal AI agent that runs the same agent core across a CLI, a messaging gateway (Telegram, Discord, Slack, ~20 platforms), a TUI, and an Electron desktop app … learns across sessions (memory + skills), delegates to subagents, runs scheduled jobs". `AGENTS.md:12-16@v0.22.2` [Verified]
- Governance and contribution rubric. A "triage sweeper" may close PRs only on `implemented_on_main`, `cannot_reproduce` or `incoherent`; taste-based closes are a maintainer decision. `AGENTS.md:30-33@v0.22.2` [Verified]
- Explicitly rejected even when well built: hooks with no consumer, new `HERMES_*` env vars for non-secret config, a new core tool when terminal/file/skill suffices, telemetry without opt-in, plugins touching core files, and third-party products in core. `AGENTS.md:41-46@v0.22.2` [Verified]
- "Footprint ladder" for new capability: extend existing → CLI command + skill → service-gated tool → plugin → MCP server in the catalog → new core tool. `AGENTS.md:56-59@v0.22.2` [Verified]
- Security reports go private (GHSA or security@nousresearch.com); `SECURITY.md` is the scope authority. `AGENTS.md:52-54@v0.22.2`, `SECURITY.md:7,226-249@v0.22.2` [Verified]
- Contributor attribution lives in `contributors/`: 2,000 tracked files (README, `reattributed.tsv`, and an `emails/` dir holding the rest). Git-history contributor counts are not available because the clone is shallow; `git shortlog` shows 103 authors only within the shallow window. [Verified/limited]
- Stack:
  - Python core.
  - Node/TypeScript: `ui-tui` (Ink), `apps/desktop` (Electron + Vite + Vitest + Playwright), `web/` dashboard SPA.
  - Docusaurus `website/`.
  - Rust: there is a `rust-tests.yml` workflow and a `native/` dir with 5 files.
  - Nix flake. `AGENTS.md:75-96@v0.22.2`; `apps/desktop/` listing; `.github/workflows/rust-tests.yml` [Verified]
- **Size at v0.22.2** (`git ls-tree -r` = **18,605 tracked files**; extensions: 8,503 `.py`, 3,228 `.ts`, 1,298 `.tsx`, 1,640 `.md`, 822 `.yaml`) [Verified]:

| Area | Tracked files | .py/.ts/.tsx lines |
|---|---:|---:|
| tests/ | 6,095 | 1,340,921 |
| apps/ (desktop + shared + bootstrap-installer) | 4,079 | 833,329 (desktop TS alone 818,772 across 3,662 files, incl. 1,564 test files and i18n) |
| hermes_cli/ | 688 | 276,956 |
| agent/ | 358 | 138,470 |
| tools/ | 357 | 125,326 |
| ui-tui/ | 544 | 105,162 |
| gateway/ | 188 | 96,463 |
| plugins/ | 355 | 75,789 |
| web/ | 242 | 61,157 |
| tui_gateway/ | 110 | 44,069 |
| scripts/ | 227 | 39,466 |
| optional-skills/ | 857 | 35,585 |
| evals/ | 221 | 21,918 |
| cron/ | 41 | 18,705 |
| skills/ | 331 | 11,590 |
| acp_adapter/ | 14 | 4,501 |
| plugin-catalog/ | 615 (614 YAML entries) | — |
| website/ | 828 | — |
| root *.py | 52 files | 33,621 |

- Biggest single files: `apps/desktop/electron/main.ts` = 19,750 lines; `agent/context_compressor.py` = 5,852 lines. The latter is over the 2,000-line ratchet cap and grandfathered, so it "may only go down". `AGENTS.md:112-115@v0.22.2` [Verified/Inference]

## D2 — Architecture & process model

**In plain words.** One Python "AIAgent" engine sits behind every surface: CLI, TUI, desktop, messaging gateway, IDE (ACP), cron, batch and an API server. Each former "god file" is now a thin facade plus many `<stem>_<topic>.py` sibling modules. A single gateway process can serve many isolated "profiles".

- Entry points into the same `AIAgent`: CLI (`cli.py`), Gateway (`gateway/run.py`), ACP (`acp_adapter/`), Batch Runner, API Server, Python library. `website/docs/developer-guide/architecture.md:13-49@v0.22.2` [Docs]
- `run_agent.py` (1,555 lines) is the AIAgent facade; the loop is in `agent/conversation_loop.py` (1,859 lines) plus about 40 `agent/turn_*.py` phase modules (iteration_prep, api_call, api_error, overflow, truncation, recovery, stop_gates, tool_round, finalizer…). `website/docs/developer-guide/agent-loop.md:9@v0.22.2`; `agent/` listing [Verified]
- "Facade + siblings" rule: siblings late-import the facade; patch where production reads; behaviour goes in a sibling, never in a facade. `AGENTS.md:101-117@v0.22.2` [Verified]
- Tool registration chain: `tools/registry.py` (no deps) ← `tools/*.py` call `registry.register()` at import ← `model_tools.py` triggers discovery ← run_agent/cli/batch. `architecture.md:268-280@v0.22.2` [Docs]
- TUI process model: Node (Ink) ⇄ newline-delimited JSON-RPC over stdio ⇄ Python `tui_gateway`. The same backend serves the Desktop app and the dashboard `/chat`, so it has three consumers. "TypeScript owns the screen. Python owns sessions, tools, model calls." `tui_gateway/AGENTS.md:1-30@v0.22.2` [Verified]
- Multi-profile in one process:
  - "One process serves many profiles". Code that runs outside a turn must bind the profile scope explicitly, otherwise it causes a "silent default-profile leak". `AGENTS.md:128-132@v0.22.2` [Verified]
  - Cron runs as ONE ticker iterating `profiles_to_serve()` under `_profile_cron_scope(home)`, never N threads. `cron/AGENTS.md:29-40@v0.22.2` [Verified]
- Gateway in-memory agent cache: LRU `max_size: 128`, `idle_ttl_secs: 3600`, RSS-based shedding `memory_high_mb: "auto"`. `hermes_cli/config_defaults.py:95-106@v0.22.2` [Verified]
- `max_live_sessions: 16` is a soft LRU cap on TUI/desktop/dashboard sessions; `max_concurrent_sessions: None` (unbounded). `config_defaults.py:53-58@v0.22.2` [Verified]
- Concurrency model: a synchronous loop; the HTTP call runs in a background thread watched by an interrupt event (`_interruptible_api_call`). `agent-loop.md:106-124@v0.22.2` [Docs]
- Optional plugin out-of-process isolation: `plugins.isolation: "in_process"` (default) | `host` (one plugin-host process per profile). `config_defaults.py:1731-1740@v0.22.2` [Verified]

## D3 — Agent loop

**In plain words.** Each user turn runs a `while` loop: build request → call model (with retries) → run the tool calls or finish with text. By default there is **no turn cap** (unlimited iterations). Safety comes from guardrails, a liveness watchdog, optional wall-clock budgets and fallback providers. Independent tool calls run in parallel on up to 8 threads.

- Main loop condition: `while (s.api_call_count < agent.max_iterations and agent.iteration_budget.remaining > 0) or agent._budget_grace_call`. Phases per iteration: `begin_iteration`, `prepare_iteration`, `assemble_api_request`, `run_preflight_gate`, `announce_api_call`, `_run_api_retry_loop`, `apply_retry_restarts`, `normalize_model_response`, then `run_tool_round` if tool_calls else `finish_text_response`; then `finalize_turn`. `agent/conversation_loop.py:1648-1701@v0.22.2` [Verified]
- Retry loop: `while s.retry_count < s.max_retries`, with `nous_rate_limit_guard` → `build_api_request` → `perform_api_call` → `check_api_response`. `InterruptedError` → `handle_api_interrupt`; other exceptions → `handle_api_error`. `conversation_loop.py:1497-1526@v0.22.2` [Verified]
- **Turn cap default = unlimited.**
  - `agent.max_turns: None` with the comment "caps caused silent mid-task truncation". `config_defaults.py:76-78@v0.22.2` [Verified]
  - `resolve_turn_limit` maps None/0/negatives/"unlimited" to `TURN_LIMIT_UNLIMITED = sys.maxsize`. `hermes_cli/config.py:1828,1835-1860@v0.22.2` [Verified]
  - The `AIAgent(max_iterations=sys.maxsize)` default is commented "unlimited … by default". `run_agent.py:241@v0.22.2` [Verified]
  - **Docs are stale.** `agent-loop.md:184` says "Default: 500 iterations"; `agent/iteration_budget.py:3-5` docstring says "default 500 … subagent default 50". [Verified discrepancy]
- `IterationBudget` is a thread-safe consume/refund counter. `execute_code` iterations are **refunded** so programmatic tool calling doesn't eat budget. `agent/iteration_budget.py:24-49@v0.22.2` [Verified]
- Other stop and budget conditions (all [Verified]):
  - Optional `budget_warning_ratio` (one-time model-visible warning; default None). `config_defaults.py:79-81@v0.22.2`
  - `run_budget_seconds` wall clock (default None; injects a one-time wrap-up notice at 80%). `config_defaults.py:82-85@v0.22.2`, `conversation_loop.py:124-153@v0.22.2`
  - `turn_liveness: {timeout_s: 600.0, poll_s: 15.0}` force-interrupts a turn with no observable progress. `config_defaults.py:289-294@v0.22.2`
  - Gateway inactivity `gateway_timeout: 1800`s (idle only), warning at 900s, "still working" every 180s, session stall notifier 300s. `config_defaults.py:86-88,222-241@v0.22.2`
  - `restart_count` backstop so redirect/fallback restarts can't refund the budget forever. `conversation_loop.py:1416-1420@v0.22.2`
- Retries and recovery:
  - `api_max_retries: 3` (the OpenAI SDK's own `max_retries=2` sits underneath).
  - `auto_recovery_cycles: 5`: after retries and the fallback chain are spent on a transient outage, it waits jittered 15/30/60/60/60s (Retry-After up to 120s).
  - `empty_response_guard`: 3 retries, dropping to 1 when one empty attempt costs ≥ $0.25.
  - `config_defaults.py:130-153@v0.22.2` [Verified]
- Anti-stall behaviour (all [Verified]):
  - `intent_ack_continuation: "auto"`: nudges "continue now, execute the tools", max 2 per turn, when the model narrates without calling a tool. `config_defaults.py:171-176@v0.22.2`
  - `stall_guards: True`: the identical-call loop breaker fires on 3+ identical calls with identical results. `config_defaults.py:177-181@v0.22.2`
  - `verify_on_stop: False` by default ("nudges proved more noise than signal"), with `max_verify_nudges: 3`. `config_defaults.py:210-221@v0.22.2`
- Tool loop guardrails (`agent/tool_guardrails.py`, a pure decision controller):
  - Warn after exact_failure 2 / same_tool_failure 3 / idempotent_no_progress 2.
  - Hard stop after 5 / 8 / 5. Hard stop is **off for interactive** surfaces and **on for gateway/cron** (`non_interactive_hard_stop_enabled: True`).
  - Per-turn caps: `max_web_searches: 50`, `max_subagents: 50`.
  - `config_defaults.py:556-575@v0.22.2`; `agent/tool_guardrails.py:1-25@v0.22.2` [Verified]
- **Parallel tool calls** (all [Verified]):
  - `_MAX_TOOL_WORKERS = 8` threads per batch; image_generate is limited to 4. `agent/tool_executor.py:132-133,242-249@v0.22.2`
  - Batches run on `DaemonThreadPoolExecutor`. `tool_executor.py:1495-1498@v0.22.2`
  - Never parallel: `clarify, manage_connections, manage_catalog, setup_beat, set_theme`.
  - Explicit parallel-safe read-only set: `read_file, search_files, session_search, skill_view, skills_list, vision_analyze, web_extract, web_search, image_generate, connectors__execute`.
  - Path-scoped reader/writer sets, plus a destructive-terminal regex (`rm, cp, mv, sed -i, dd, git reset/clean/checkout…`) that serializes. `agent/tool_dispatch_helpers.py:30-62@v0.22.2`
  - Results are reinserted in the original call order. `agent-loop.md:128-135@v0.22.2` [Docs]
  - System prompt includes `parallel_tool_call_guidance: True` (~70 cached tokens). `config_defaults.py:185-188@v0.22.2` [Verified]
- Agent-level intercepted tools (`todo`, `memory`, `session_search`, `delegate_task`) bypass the registry dispatch. `agent-loop.md:150-161@v0.22.2` [Docs]
- Streaming:
  - `display.streaming: False` CLI default; per-platform overrides Telegram `True`, Discord `False`.
  - `stream_delta_callback`, `tool_gen_callback` and `reasoning_callback` surfaces.
  - Plugin hooks `on_stream_start/delta/end`.
  - `config_defaults.py:855,867,960-961@v0.22.2`; `agent-loop.md:163-176@v0.22.2`; `hermes_cli/plugins.py:109-193@v0.22.2` [Verified/Docs]
- Interrupts: the API thread is abandoned, the partial response is discarded, and the user message wins. On failed turns a Hermes-authored assistant "boundary" row is appended so the next user turn doesn't merge with the failed one (`_close_durable_failed_turn`). `agent-loop.md:121-124@v0.22.2`; `conversation_loop.py:1813-1856@v0.22.2` [Docs/Verified]
- Scripted "prelude" tool calls can run before the first model call (`play_prelude`); there is also a `codex_app_server` mode that hands the whole turn to a Codex subprocess. `conversation_loop.py:1640-1647@v0.22.2` [Verified]

## D4 — Providers, models, tool-call parsing, prompt caching

**In plain words.** Hermes talks to dozens of providers through five wire protocols (OpenAI chat, OpenAI Responses/Codex, Anthropic Messages, Bedrock Converse, Codex app-server). It keeps one internal OpenAI-style message format. Anthropic-style prompt caching is a first-class design invariant.

- Valid API modes: `{"chat_completions","codex_responses","anthropic_messages","bedrock_converse","codex_app_server"}`. `hermes_cli/runtime_provider.py:96@v0.22.2` [Verified]
- Pluggable transports: `agent/transports/` has anthropic, bedrock, chat_completions, codex, codex_app_server, hermes_tools_mcp_server, with `register_transport(api_mode, cls)`. `agent/transports/__init__.py:21-51@v0.22.2` [Verified]
- `PROVIDER_REGISTRY` has **80 keys** (including aliases): anthropic, openai-codex, openrouter-family routers, bedrock, vertex, gemini, xai, deepseek, kimi, minimax, qwen-oauth, zai, nous, nvidia, lmstudio, ollama-cloud, copilot / copilot-acp, azure-foundry, huggingface, fireworks, … (counted by import). `hermes_cli/auth.py` [Verified]
- **42 model-provider plugin dirs** in `plugins/model-providers/`. `plugins/model-providers/` listing [Verified]
- Docs claim "18+ providers". `architecture.md:211@v0.22.2` [Docs, understated]
- Fallback and auth (all [Verified]):
  - `fallback_providers: []` is an ordered chain, with credential refresh on 401/403. `config_defaults.py:36@v0.22.2`; `agent-loop.md:188-198@v0.22.2` [Docs]
  - Credential pools with strategies (`credential_pool_strategies: {}`). `config_defaults.py:41@v0.22.2`
  - `auth.adopt_external_logins: True` borrows the Codex CLI (`~/.codex/auth.json`) and **Claude Code (`~/.claude/.credentials.json`)** logins when Hermes has none. `config_defaults.py:1754-1760@v0.22.2`
- Auxiliary models: an aux client handles side tasks (vision, compression, goal judge, smart approval, curator, title generation), each with its own fallback chain under `auxiliary.*`. `agent-loop.md:198@v0.22.2`; `agent/auxiliary_*.py` (≈20 modules) [Docs/Verified]
- Tool-call parsing:
  - Native structured tool_calls per API mode.
  - Argument canonicalization/repair (`_canonicalize_api_tool_calls`). `conversation_loop.py:997-1050@v0.22.2`
  - Invalid-tool-name error content. `conversation_loop.py:1052@v0.22.2`
  - Truncated tool-call retries (`truncated_tool_call_retries`). `conversation_loop.py:1422@v0.22.2`
  - Hermes-format `<tool_call>` XML prompting is used for ACP/OpenAI bridges and trajectories. `agent/agent_runtime_helpers.py:105-138@v0.22.2`
  - Reasoning replay: `reasoning_echo` pad for DeepSeek / Kimi / MiMo. `config_defaults.py:283-288@v0.22.2`
  - All [Verified].
- **Prompt caching:**
  - Default layout is 4 `cache_control` breakpoints (static system prefix + last 3 messages) sharing one TTL (5m or 1h). `agent/prompt_caching.py:3-5,102-104@v0.22.2`
  - Default `prompt_caching.cache_ttl: "5m"`; `auto` picks the tier by who paces the session. `config_defaults.py:702@v0.22.2`; `prompt_caching.py:132-156@v0.22.2`
  - Both [Verified].
- Caching as an invariant: "Per-conversation prompt caching is sacred … the ONE exception is context compression." Slash commands that change system-prompt state defer to the next session (`--now` opt-in). `AGENTS.md:20-23@v0.22.2` [Verified]
- Model-specific prompt blocks (all [Verified]):
  - `tool_use_enforcement: "auto"` (gpt/codex). `config_defaults.py:161-164@v0.22.2`
  - `execution_guidance: "auto"` (gpt/codex/grok/deepseek/kimi/qwen/glm/minimax/mimo/mistral). `config_defaults.py:165-170@v0.22.2`
  - `task_completion_guidance: True`. `config_defaults.py:182-184@v0.22.2`
- Model metadata via a `models.dev` registry plus llama.cpp/HTTP probes. `agent/models_dev.py`, `agent/model_metadata_*.py` [Verified file presence]

## D5 — Prompt & context assembly / compaction

**In plain words.** The system prompt is built once per session in three cache tiers (stable → context → volatile) and is only rebuilt when the context is compressed. Compression summarizes the middle of the conversation using an auxiliary model. The configured trigger is 50% of the window, but any model under 512K tokens is floored to 75%, so most models actually compress at 75%.

- Tiers: `stable` (identity / SOUL.md, guidance, env hints, coding brief, platform hints) → `context` (workspace snapshot, caller system_message, context files) → `volatile` (skills index, memory, USER.md, external memory, timestamp). "Built once per session … only context compression triggers a rebuild". `agent/system_prompt.py:1-8,731-746@v0.22.2` [Verified]
- Byte stability: the date line is date-only, with zone + offset, "so the prompt is byte-stable for the day". `system_prompt.py:476-502@v0.22.2` [Verified]
- Context files auto-loaded: SOUL.md, AGENTS.md, CLAUDE.md, .hermes.md, .cursorrules.
  - Per-file cap `context_file_max_chars: None`, which scales with the window (floor 20K, ceiling 500K chars). `config_defaults.py:519-522@v0.22.2` [Verified]
  - Read timeout 5.0s. `config_defaults.py:525@v0.22.2` [Verified]
  - Each file is threat-scanned with `scan_for_threats(scope="context")` before injection. `agent/prompt_builder.py:30,83-100,1688,1708,1843@v0.22.2` [Verified]
- Coding posture `coding_context: "auto"`: on interactive surfaces in a code workspace it adds a coding brief and a live git snapshot; "focus" mode collapses the toolset. `config_defaults.py:199-205@v0.22.2` [Verified]
- Compression defaults (all [Verified]):
  - `compression.enabled: True`, `threshold: 0.50`, `threshold_tokens: None`, `target_ratio: 0.20`, `tail_mode: "lean"` (2.5%-of-window tail, 10K floor / 25K cap), `protect_last_n: 20`, `min_tail_user_messages: 1`, `max_attempts: 3`. `config_defaults.py:577-609@v0.22.2`
  - The **0.75 floor for windows < 512K** is in the code: `_SMALL_CTX_WINDOW_LIMIT = 512_000`, `_SMALL_CTX_THRESHOLD_PERCENT = 0.75`. `agent/context_compressor.py:1148-1151@v0.22.2`
  - The config comment documents it. `config_defaults.py:586-589@v0.22.2`
  - `LEAN_TAIL_FLOOR_TOKENS = 10_000`, `_MAX_TAIL_MESSAGE_FLOOR = 8`. `context_compressor.py:917,1135@v0.22.2`
- Opt-in cheaper compaction modes, off by default because each pass breaks the cache (all [Verified]):
  - `proactive_prune_tokens: 0`: deterministic no-LLM tool-result prune; min result 8,000 chars; min reclaim 4,096 tokens.
  - `micro_compact: False`: a rolling per-turn summary.
  - `config_defaults.py:610-631@v0.22.2`
- Gateway "hygiene" compression (all [Verified]):
  - Force-compress at 5,000 messages.
  - 30s inactivity timeout, 600s ceiling.
  - Holds an arriving turn at most 10s.
  - `config_defaults.py:632-645@v0.22.2`
- Docs: "Gateway auto-compression: … 85%". `agent-loop.md:205@v0.22.2` [Docs]
- What compression does: flushes memory first; keeps tool-call/result pairs together; creates a child session lineage. `agent-loop.md:207-213@v0.22.2` [Docs]
- Pluggable `ContextEngine` ABC. The default `context.engine: "compressor"`; engines are single-select plugins. `agent/context_engine.py:48@v0.22.2`; `config_defaults.py:1300-1301@v0.22.2`; `architecture.md:235@v0.22.2` [Verified/Docs]
- `compression.checkpoint_required: False` fails closed before lossy compaction unless the memory provider confirms a checkpoint. `config_defaults.py:579-581@v0.22.2` [Verified]
- Tool output caps (all [Verified]):
  - `tool_output.max_bytes: 50000` (terminal, head+tail), `max_lines: 2000`, `max_line_length: 2000`. `config_defaults.py:552-555@v0.22.2`
  - `file_read_max_chars: 100_000`. `config_defaults.py:526-528@v0.22.2`
  - Subagent summaries capped at `max_summary_chars: 24000`, with overflow spilled to a cache file. `config_defaults.py:1362-1366@v0.22.2`
- Ephemeral `prefill_messages_file` (few-shot priming, never persisted). `config_defaults.py:1396-1398@v0.22.2` [Verified]

## D6 — Tools catalog, MCP client/server, ACP

**In plain words.** A central registry holds about 50 core tools: web, terminal, files, browser, vision, skills, memory, delegation, cron, kanban, computer use and more. Rarely used tools are hidden behind a "tool search" bridge to save tokens. Hermes is a full MCP client (stdio/HTTP/SSE, OAuth, sampling). It can also act as an MCP server and as an ACP agent for IDEs.

- `_HERMES_CORE_TOOLS` = **51 names**:
  - Web: `web_search, web_extract`
  - Terminal: `terminal, process_manage`
  - Files: `read_file, write_file, patch, search_files`
  - Media: `vision_analyze, image_generate`
  - Skills: `skills_list, skill_view, skill_manage`
  - Browser: `browser_navigate, browser_snapshot, browser_click, browser_type, browser_scroll, browser_back, browser_press, browser_get_images, browser_vision, browser_console, browser_cdp, browser_dialog, browser_vault, browser_exec`
  - Assistant: `text_to_speech, todo_list, memory, session_search, clarify, execute_code, delegate_task, cronjob_manage`
  - Kanban (13): `kanban_show, kanban_list, kanban_complete, kanban_block, kanban_request_review, kanban_request_changes, kanban_heartbeat, kanban_comment, kanban_create, kanban_link, kanban_unblock, kanban_attach, kanban_attach_url, kanban_attachments`
  - Other: `computer_use, manage_connections`
  - `toolsets.py:11-50@v0.22.2` [Verified]
- Other registered tools (AST scan of `registry.register(name=…)` in tools/):
  - Video: `video_analyze, video_generate`
  - Search: `x_search`
  - Desktop UI: `desktop_preview, drive_preview, annotate_preview, gui_tour, show_tip, focus_pane, set_theme, read_terminal, close_terminal, read_window_below, desktop_project`
  - Setup/catalog: `manage_catalog, setup_beat`
  - Messaging/docs: `react_to_message, feishu_doc_read`
  - [Verified]
- **59 toolsets** (`TOOLSETS`), including per-platform presets (`hermes-cli`, `hermes-telegram`, `hermes-acp`, `hermes-api-server`, `hermes-cron`, …). Counted by import. `toolsets.py` [Verified]
- Docs say "70+ tools, ~28 toolsets". `architecture.md:217@v0.22.2` [Docs]
- Dispatch: `model_tools.handle_function_call(function_name, function_args, task_id, tool_call_id, session_id, turn_id, api_request_id, user_task, enabled_tools…)`. `model_tools.py:873-876@v0.22.2` [Verified]
- `check_fn` service gating: answers reachability/opt-in only, never per-session surface. `AGENTS.md:56-58@v0.22.2` [Verified]
- **Tool Search (deferred tools):**
  - `tools.tool_search.enabled: "auto"`; listing budget `threshold_pct: 5`% of context.
  - Tiered manifests: tier 0 = everything eager; tier 1 = bridge + name/description manifest; tier 2 = bridge + per-server summary for ~3,300-tool APIs.
  - Default deferred: `computer_use, session_search, image_generate, todo_list, process_manage, cronjob_manage, browser_vault`, and the desktop GUI tools.
  - `config_defaults.py:1986-2022@v0.22.2` [Verified]
- `execute_code` = "Programmatic Tool Calling":
  - The LLM writes Python that calls Hermes tools over RPC; only stdout returns.
  - Locally it runs a persistent per-conversation kernel over a Unix socket; remote backends get a remote kernel or ship the script per call.
  - `tools/code_execution_tool.py:1-11@v0.22.2` [Verified]
- **MCP client** (all [Verified]):
  - `tools/mcp_tool.py` facade + 25 `mcp_*` siblings (OAuth incl. device flow, sampling, schema cache, liveness, death supervisor, Node ABI…).
  - Transports: stdio, Streamable HTTP, SSE.
  - One background asyncio loop, with each server as a long-lived Task. `tools/mcp_tool.py:1-9@v0.22.2`
  - Discovery: `mcp_discovery_timeout: 1.5`s (one-shot 15.0s), `discovery_concurrency: 4`.
  - `auto_reload_on_config_change: True`; the docs warn it invalidates the prompt cache.
  - `config_defaults.py:529-551@v0.22.2`
- **MCP catalog:** `optional-mcps/` has **65** vetted server definitions (airtable … wordpress-com, incl. linear, notion, sentry, stripe, supabase, vercel). `optional-mcps/` listing [Verified]
- **MCP server:**
  - `hermes mcp serve` is a stdio server exposing messaging conversations (list, read history, send, poll events, manage approvals). It "Matches OpenClaw's 9-tool channel bridge surface plus … channels_list". `mcp_serve.py:1-8@v0.22.2` [Verified]
  - `agent/transports/hermes_tools_mcp_server.py` exposes Hermes tools to the Codex app-server runtime. [Verified file]
- **ACP:**
  - `acp_adapter/` (14 files: server, session, permissions, edit_approval, events, tools, model_catalog…) runs Hermes as an ACP agent over stdio for VS Code / Zed / JetBrains.
  - Renders chat, tool activity, file diffs, terminal commands and approval prompts.
  - `acp_adapter/server.py:1@v0.22.2`; `website/docs/user-guide/features/acp.md:9-20@v0.22.2` [Verified/Docs]
  - There is also an ACP *client* (`agent/copilot_acp_client.py`, provider `copilot-acp`). [Verified file]

## D7 — Execution environments / sandboxing

**In plain words.** By default, shell commands run directly on your machine (`backend: local`) with no sandbox. Six other backends exist: Docker, SSH, Singularity, Modal, Daytona and Vercel Sandbox. The project's own security policy says only OS-level isolation counts as a real boundary.

- `terminal.backend: "local"` default; `timeout: 180`s; `cwd: "."`. `config_defaults.py:297-314@v0.22.2` [Verified]
- Backends in `tools/environments/`: `local, docker, ssh, singularity, modal (+ managed_modal), daytona, vercel_sandbox` = **7**, plus `docker_egress`, `file_sync`, `remote_common`. `tools/environments/` listing [Verified]
- Docs agree: "7 backends". `architecture.md:217@v0.22.2` [Docs]
- `BaseEnvironment`: "spawn-per-call … every command spawns a fresh `bash -c`". The session snapshot (env vars, functions, aliases) is captured once and re-sourced each call. CWD persists via stdout markers (remote) or a temp file (local). `tools/environments/base.py:1-8@v0.22.2` [Verified]
- Docker hardening (all [Verified]):
  - `--cap-drop ALL` + add back `DAC_OVERRIDE, CHOWN, FOWNER`.
  - tmpfs `/tmp` 512m nosuid, `/var/tmp` 256m noexec.
  - `--security-opt no-new-privileges`, dropped in `snap_compat` mode.
  - PID limit only when the pids cgroup exists.
  - `tools/environments/docker.py:300-313,379-384@v0.22.2`
- Container defaults: `container_cpu: 1`, `container_memory: 5120` MB, `container_disk: 51200` MB, `container_persistent: True`.
  - Default image `DEFAULT_SANDBOX_IMAGE`: nikolaik/python-nodejs (Python 3.13 / Node 26) plus a display stack, so the browser and computer_use run inside the sandbox.
  - `config_defaults.py:348-366@v0.22.2` [Verified]
- `degraded_mode: "warn"` for remote connection failures; `env_passthrough: []` (skill-declared required env vars pass automatically); `home_mode: "auto"`. `config_defaults.py:300-335@v0.22.2` [Verified]
- Remote sync-back cap 2 GiB. `config_defaults.py:329-331@v0.22.2` [Verified]
- Credential scoping: provider keys and gateway tokens are stripped from shell / MCP / cron-script / code-exec children; "reduces casual exfiltration. It is not containment." `SECURITY.md:121-131@v0.22.2` [Verified]
- Egress proxy: the sandbox sees opaque tokens and an "iron-proxy" swaps in real credentials at egress (`hermes egress setup`). `config_defaults.py:2592-2594@v0.22.2`; `website/docs/developer-guide/egress-internals.md` [Verified/Docs]
- Whole-process wrapping posture: Hermes' own Docker image/Compose, or **NVIDIA OpenShell** (declarative FS / L7-network / syscall / inference policy). `SECURITY.md:90-114@v0.22.2` [Verified]
- Terminal environments are also pluggable (`terminal-environment-plugin.md`, `agent/terminal_env_registry.py`). [Docs/Verified file]

## D8 — Safety & security

**In plain words.** Dangerous shell commands go through an approval gate:
- **12 "hardline" patterns** are always blocked.
- **110 "dangerous" patterns** need approval.
- In the default "smart" mode, an auxiliary LLM guardian decides first.

Unattended surfaces (cron, one-shot, unattended platforms) deny by default. The gateway denies unknown users unless they are allow-listed or paired with a one-time code. The project is candid that all of these in-process checks are heuristics, not containment.

- Trust model: "**The only security boundary against an adversarial LLM is the operating system.** Nothing inside the agent process constitutes containment — not the approval gate, not output redaction, not any pattern scanner, not any tool allowlist." `SECURITY.md:58-65@v0.22.2` [Verified]
- Approval config (all [Verified]):
  - `approvals.mode: "smart"`, `timeout: 300`s, `cron_mode: "deny"`, `single_query_mode: "deny"`, `unattended_mode: "deny"`.
  - `smart_policy: ""` appends extra guardian rules.
  - `denial_breaker_threshold: 3` (after 3 consecutive guardian DENYs, escalate to a hard stop).
  - `deny: []` user fnmatch globs **block even under --yolo / mode=off**.
  - `mcp_reload_confirm: True`, `destructive_slash_confirm: True`.
  - `config_defaults.py:1665-1701@v0.22.2`
- Modes are `manual | smart | off`; the code falls back to "manual" if the config is absent; a hosted-room policy can override. `tools/approval_context.py:245-253@v0.22.2` [Verified]
- Persistent allowlist `command_allowlist: []` (filled via "always" approvals). `config_defaults.py:1702-1703@v0.22.2` [Verified]
- Pattern counts by import (all [Verified]):
  - `HARDLINE_PATTERNS` = **12**: rm -rf of root, system dirs or home; mkfs; dd to a block device; redirect to a block device; fork bomb; kill -1; shutdown / reboot / halt / poweroff; init 0/6; systemctl poweroff; telinit. `tools/approval_detection.py:107-145@v0.22.2`
  - `DANGEROUS_PATTERNS` = **110**. `approval_detection.py:228-485@v0.22.2`
  - Patterns are command-position anchored or quote-masked so prose like `git commit -m "rm -rf /"` doesn't trip them, while `sh -c` / `eval` payloads are still scanned raw. `approval_detection.py:100-131@v0.22.2`
- YOLO hardening: `HERMES_YOLO_MODE` is **frozen at import**, "reading os.environ per call would let any skill … bypass every approval check (prompt-injection escalation path)". `tools/approval.py:44-46@v0.22.2` [Verified]
- Smart-approval guardian injection defenses: shell comments are stripped before assessment, the command is wrapped in XML-style delimiters, and the system message tells the guard to ignore directives in `<command>`. "Inspired by OpenAI Codex's Smart Approvals". `tools/approval_smart.py:1-9@v0.22.2` [Verified]
- Subagents never prompt: `subagent_auto_approve: False` = auto-deny. `config_defaults.py:1387-1390@v0.22.2` [Verified]
- Prompt-injection / threat scanning (all [Verified]):
  - `tools/threat_patterns.py` has **36 patterns**, scopes `all / context / strict`, plus invisible-Unicode detection (zero-width etc.). `threat_patterns.py:108-148@v0.22.2`
  - Applied to context files (prompt_builder), memory writes (`scope="strict"`, blocks the write), and skills.
  - `tools/memory_tool_store.py:16,28-31,226-234@v0.22.2`
- Protected instruction files: writes to AGENTS.md / CLAUDE.md / SOUL.md / .cursorrules / project `.hermes` config "always need human approval, even under yolo". `config_defaults.py:1784-1788@v0.22.2` [Verified]
- Other guard modules: `file_tools_write_guards.py`, `self_repo_guard.py`, `skills_guard.py`, `plugin_guard.py`, `osv_check.py` (OSV vuln check), `terminal_tool_guards.py`. `tools/` listing [Verified]
- SSRF protection (all [Verified]):
  - `tools/url_safety.py` blocks private IPs; cloud metadata endpoints are **always** blocked.
  - DNS-rebinding is closed by dialing the validated IP; redirects are re-validated. `tools/url_safety.py:1-11@v0.22.2`
  - `security.allow_private_urls: False`. `config_defaults.py:1770@v0.22.2`
- Secret redaction:
  - `security.redact_secrets: True`. `config_defaults.py:1775@v0.22.2`
  - `agent/redact.py` (36 `re.compile`s): tokens under 18 chars are fully masked; longer ones keep the first 6 and last 4 chars. `agent/redact.py:1-5@v0.22.2`
  - Failed-turn error metadata is force-redacted before persistence. `conversation_loop.py:1791-1810@v0.22.2`
  - All [Verified].
- **Credential vault:**
  - `agent/vault_store.py` is a profile-scoped, **model-blind** store (Fernet-encrypted payload, 0600 key/file under `<HERMES_HOME>/vault/`). The model sees only opaque handles; secrets are resolved server-side by the browser fill path. Kinds: `login`, `payment`, `address`. Design ported from Merit-Systems/OpenInstinct (MIT). `agent/vault_store.py:1-16@v0.22.2` [Verified]
  - Backends: local, Bitwarden, 1Password (`agent/vault_backends/`); secret sources: Bitwarden, 1Password, command (`agent/secret_sources/`). [Verified]
- **Gateway auth:**
  - Per-platform `<PLATFORM>_ALLOWED_USERS` / `_ALLOW_ALL_USERS`, `GATEWAY_ALLOW_ALL_USERS`, otherwise "default deny". `gateway/authz_mixin.py:32-36,614-646@v0.22.2` [Verified]
  - **DM pairing:** 8-char codes from a 32-char unambiguous alphabet via `secrets`; TTL 3600s; 1 request per user per 600s; max 3 pending per platform; lockout after 5 failures for 3600s; 0600 files; salted-hash compare; codes never logged. `gateway/pairing.py:1-8,29-36,474,490@v0.22.2` [Verified]
- Shell hooks require first-run consent (`~/.hermes/shell-hooks-allowlist.json`); `hooks_auto_accept: False`. `config_defaults.py:1742-1750@v0.22.2` [Verified]
- Skills (all [Verified]):
  - `skills.inline_shell: False`; community hub installs never auto-execute.
  - `guard_agent_created: False` (rationale: "the agent can run the same code via terminal() ungated").
  - Hub installs are always scanned; NVIDIA SkillEvaluator is an optional advisory.
  - `config_defaults.py:1466-1478@v0.22.2`
- Supply chain:
  - `security.acked_advisories` startup banner; `allow_lazy_installs: True`. `config_defaults.py:1790-1797@v0.22.2` [Verified]
  - CI workflows: `osv-scanner.yml`, `supply-chain-audit.yml`, `lockfile-diff.yml`, `uv-lockfile-check.yml`. `.github/workflows/` [Verified]
  - Dependencies carry upper bounds; git URLs and Actions are pinned to a SHA. `AGENTS.md:134-136@v0.22.2` [Verified]
- MoA privacy filter for advisor outputs: `moa.privacy_filter: ""` (off) | display | full. `config_defaults.py:1426-1433@v0.22.2` [Verified]

## D9 — Memory & state

**In plain words.** Memory has two layers:
- **Small curated files** (MEMORY.md ≈2.2K chars, USER.md ≈1.4K chars) are always injected into the prompt.
- **A SQLite database with full-text search** holds every session; the agent can search it on demand.

Optional external memory providers (Honcho, mem0, etc.) can be plugged in, one at a time.

- Built-in memory (all [Verified]):
  - `memory.memory_char_limit: 2200` ("~800 tokens at 2.75 chars/token"), `user_char_limit: 1375` (~500 tokens), `nudge_interval: 10`, `write_approval: False`, `provider: ""`. `config_defaults.py:1311-1326@v0.22.2`
  - Store defaults match: `MemoryStore.__init__(memory_char_limit=2200, user_char_limit=1375)`; entries are joined by `"\n§\n"`. `tools/memory_tool_store.py:25,186@v0.22.2`
- `memory` tool actions `add | patch | remove`, targets `memory | user`; the threat scan blocks injected entries. `tools/memory_tool.py:404-409@v0.22.2`; `memory_tool_store.py:226-234@v0.22.2` [Verified]
- External memory providers are single-select:
  - Bundled in-tree: `plugins/memory/byterover`, `plugins/memory/retaindb` only.
  - Catalog-installed: honcho, hindsight, supermemory, mem0, openviking, **holographic**.
  - `config_defaults.py:1323-1325@v0.22.2`; `plugins/memory/` listing [Verified]
  - The brief listed holographic as in-tree; at v0.22.2 it is a **catalog** plugin. [Verified correction]
- Cron sessions run with `skip_memory=True`. `cron/AGENTS.md:87@v0.22.2` [Verified]
- **SessionDB** (`hermes_state.py` facade + ~37 `hermes_state_*.py` siblings) [Verified]:
  - `SCHEMA_VERSION = 31`. `hermes_state_common.py:283@v0.22.2`
  - Tables: `schema_version, system_prompts, sessions, messages, session_model_usage, state_meta, gateway_routing, gateway_hygiene_state, conversation_generations, gateway_heartbeats, compression_locks, session_turn_leases, async_delegations`. `hermes_state_common.py:373-582@v0.22.2`
  - FTS5 virtual tables `messages_fts` plus a **trigram** `messages_fts_trigram` (substring/CJK search). `hermes_state_common.py:785,905@v0.22.2`
  - FTS5-availability probe with graceful degradation. `hermes_state_schema.py:212-220,692-695@v0.22.2`
  - `database.journal_mode: "wal"` (switch to `"delete"` on NFS/SMB/virtiofs). `config_defaults.py:43-50@v0.22.2`
- `session_search` tool: one shape with modes inferred from args (DISCOVERY via FTS5 deduped by lineage, SCROLL, READ, BROWSE). "No LLM calls — every shape returns actual DB messages." `tools/session_search_tool.py:1-8@v0.22.2` [Verified]
- Session lineage: compression creates a child session; resume via `/resume` or `hermes chat --resume`; per-terminal `-c` breadcrumbs (`session.terminal_continue: True`). `agent-loop.md:213-220@v0.22.2`; `config_defaults.py:59-64@v0.22.2` [Docs/Verified]
- State-DB recovery / repair tooling: `hermes_state_repair.py`, `hermes_state_rewind.py`, `hermes_state_wal.py`, and the `state-db-recovery.md` doc. [Verified files]
- **Checkpoints / rollback** (all [Verified]):
  - A shadow-git store snapshots the working dir once per turn before `write_file` / `patch` / destructive terminal; it is "NOT a tool — the LLM never sees it". `tools/checkpoint_manager.py:1-11@v0.22.2`
  - Defaults: `checkpoints.enabled: False` (opt-in `--checkpoints`), `max_snapshots: 20`, `max_total_size_mb: 500`, `max_file_size_mb: 10`, `retention_days: 7`. `/rollback` restores. `config_defaults.py:496-518@v0.22.2`
- Profiles: `hermes -p <name>` gives each profile its own HERMES_HOME, config, memory, sessions and gateway PID ("profiles are islands on purpose"). `architecture.md:266@v0.22.2`; `AGENTS.md:48-50@v0.22.2`; `hermes_cli/profiles.py` (2,548 lines) [Docs/Verified]
- Persona: `SOUL.md` is the identity file (the in-repo default begins "You are Hermes Agent, built by Nous Research. Be direct…"). It is loaded into the stable tier and threat-scanned. `SOUL.md:1@v0.22.2`; `prompt_builder.py:1688@v0.22.2` [Verified]

## D10 — Skills & self-improvement

**In plain words.** Skills are SKILL.md folders the agent can list, load and **write itself** via `skill_manage`. After turns, a background "review" fork decides whether to save memory or skills. A periodic "curator" marks unused agent-made skills stale after 14 days and archives them after 30 days (never deletes them). It can optionally merge overlapping skills using an LLM.

- Counts: **58 bundled skills** (`skills/**/SKILL.md`) and **153 optional skills** (`optional-skills/**/SKILL.md`, shipped but not active). [Verified]
- `skill_manage` actions: `create, edit, patch, delete, write_file, remove_file`, each with guards (`_guarded_write`, `_locate_for_write`, background-review read-before-write and delete guards). `tools/skill_manager_tool.py:391,477,498-619@v0.22.2` [Verified]
- Skill config (all [Verified]):
  - `external_dirs` (read-only shared dirs, e.g. `~/.agents/skills`), `create_dir`.
  - Project skills from `<root>/.hermes/skills` and `.agents/skills` load only for `trusted_project_dirs`.
  - `auto_load: []`, `template_vars: True`, `write_approval: False`.
  - **Audit ledger** `.curator_ledger.jsonl` with before/after hashes and rollback (`ledger: True`, 5 MiB cap).
  - `config_defaults.py:1446-1492@v0.22.2`
- **Background review (nudges):**
  - After a turn, a daemon thread may fork the agent on a snapshot of the conversation and ask "should any skill/memory be saved or updated?". The fork inherits the cached system prompt, so it hits the same prefix cache, and runs under a tool whitelist; the main prompt cache is untouched. `agent/background_review.py:1-6@v0.22.2` [Verified]
  - Nudge cadence: `memory.nudge_interval: 10`, `skills.creation_nudge_interval` default 10. `agent/agent_init.py:1302,1326,1381-1383@v0.22.2` [Verified]
  - A skill-nudge review "never gets the memory tool". `background_review.py:1087@v0.22.2` [Verified]
- **Curator:**
  - Config: `curator.enabled: True`, `interval_hours: 168` (7 days), `min_idle_hours: 2`, **`stale_after_days: 14`, `archive_after_days: 30`**, `consolidate: False` (LLM umbrella pass off), `prune_builtins: False`, `archive_ttl_days: 0` (never purge), `backup: {enabled: True, keep: 2}`. `config_defaults.py:1494-1522@v0.22.2` [Verified]
  - **This corrects the brief's "30d / 90d".** The docs match the code (14 / 30). `website/docs/user-guide/features/curator.md:38@v0.22.2` [Verified]
  - Invariants: inactivity-triggered (no cron daemon); only curator-managed skills; never delete, only archive; pinned skills bypass. `agent/curator.py:1-7@v0.22.2` [Verified]
  - Cron-referenced skills are skipped; never-used skills get a grace floor. `curator.md:39-40@v0.22.2` [Docs]
- **Skills Hub** sources (all `SkillSource` subclasses): `OptionalSkillSource`, `HermesIndexSource`, `GitHubSource`, `ClawHubSource` (OpenClaw's hub), `SkillsShSource`, `WellKnownSkillSource`, `UrlSource`, `LobeHubSource`, `BrowseShSource`. `tools/skills_hub_*.py` (e.g. `skills_hub_models.py:127`, `skills_hub_clawhub.py:61`) [Verified]
- agentskills.io hub-installed skills are never curated. `curator.md:13@v0.22.2` [Docs]
- Learning extras: `agent/learning_graph.py`, `agent/learn_prompt.py`, `agent/insights.py` (files present; behaviour not read). [Verified presence / Inference on purpose]

## D11 — Multi-agent & orchestration

**In plain words.** The agent can spawn subagents with `delegate_task`:
- Each child gets its own budget, up to 10 run in parallel, and nesting is optional.
- Longer-lived multi-agent work goes on a SQLite **Kanban board** whose dispatcher spawns worker profiles.

Also available: cron jobs, `/goal` (keep going until an auxiliary-model judge says done), `/loop` (re-run a prompt on a cadence), heartbeats, Mixture-of-Agents and git worktrees.

- **delegate_task** (all [Verified]):
  - Child AIAgents get a fresh conversation, their own task_id, and the parent's toolsets minus blocked tools. Single and batch (parallel) modes; "The parent only ever sees the delegation call and the summary result". `tools/delegate_tool.py:1-11@v0.22.2`
  - Blocked in children: `delegate_task` (no recursion), `clarify`, `memory`, `send_message`, `cronjob_manage`, … `tools/delegate_tool_toolsets.py:14-20@v0.22.2`
  - Defaults: **`max_iterations: 250`** per child (own budget), `max_concurrent_children: 10`, **`max_spawn_depth: 1`** (flat; 2 = orchestrator→leaf), `orchestrator_enabled: True`, `child_timeout_seconds: 0` (inactivity-based), `oneshot_max_children: 2`, `inherit_mcp_toolsets: True`, `independent_completions: False`. A cheaper model/provider can be set via `delegation.model/provider`. `config_defaults.py:1327-1395@v0.22.2`
- **250 vs 50 verdict:**
  - Code = 250, and the user docs agree. `website/docs/user-guide/features/delegation.md:344,636@v0.22.2`
  - Stale docs say 50: `agent-loop.md:185@v0.22.2` and `agent/iteration_budget.py:4`. [Verified discrepancy]
- Stuck-child detection: heartbeat staleness at 450s idle / 1200s inside a tool → `status: "timeout"`. `delegation.md:358@v0.22.2` [Docs]
- Background `delegate_task` is process-local; work that must survive restarts should be a cron job or `terminal(background=True, notify_on_complete=True)`. `cron/AGENTS.md:94-95@v0.22.2` [Verified]
- **Kanban** (all [Verified]):
  - A durable SQLite board; CLI `hermes kanban` facade + 14 siblings.
  - The dispatcher loop (default 60s) reclaims stale claims, promotes ready tasks, atomically claims and spawns assigned profiles; it runs inside the gateway by default (`kanban.dispatch_in_gateway: true`).
  - The board is the hard isolation boundary (`HERMES_KANBAN_BOARD` pinned); the tenant is a soft namespace.
  - Auto-block after `kanban.failure_limit` = 2 failed attempts; exit code 78 trips it immediately.
  - Worker liveness = `(pid, start_time)` fingerprint.
  - `cron/AGENTS.md:97-149@v0.22.2`
  - Extras: `kanban_swarm.py`, `kanban_decompose.py`, `kanban_pr_acceptance.py`. [Verified files]
- **Cron** (all [Verified]):
  - First-class *agent* jobs. Schedules: duration, "every …" phrase, 5-field cron, ISO one-shot.
  - Per-job skills, model/provider, pre-run `script` (or `no_agent`), `context_from` chaining, `workdir`, multi-platform delivery.
  - Inactivity watchdog 600s (`HERMES_CRON_TIMEOUT`); script timeout 3600s.
  - Catch-up = half the period clamped 120s–2h; at-most-once via `next_run_at` advanced before dispatch.
  - Per-home tick lock.
  - `cron/AGENTS.md:6-40@v0.22.2`
  - Approvals in cron default to deny. `config_defaults.py:1681@v0.22.2`
- **/goal**: "the Ralph loop for Hermes". After each turn an aux-model judge decides whether the goal is met; the continuation is a normal user message (cache-safe). Judge failures **fail OPEN**; `goals.max_turns: 20` is the backstop; judge timeout 30s, max_tokens 4096. `hermes_cli/goals.py:1-6,33-37@v0.22.2`; `config_defaults.py:1399-1406@v0.22.2` [Verified]
- **/loop**: "Claude Code parity". Stops on `LOOP_COMPLETE`, `--times N`, an `--until` judge, or `loops.max_ticks: 100`. `min_interval_seconds: 30`; self-paced 60–900s with exponential backoff while replies stop changing. State lives in SessionDB `state_meta`. `hermes_cli/loops.py:1-7@v0.22.2`; `config_defaults.py:1407-1415@v0.22.2` [Verified]
- **/heartbeat**: a session-scoped, in-process recurring re-entry prompt with `MIN_INTERVAL_SECONDS = 60`. It fires only into an idle session with an empty queue; the prompt says "reply briefly that nothing has changed … do not invent work". `hermes_cli/heartbeat.py:1-25@v0.22.2` [Verified]
- **MoA** (`/moa`) (all [Verified]):
  - Reference models + an aggregator produce private guidance before each main-model iteration; the normal loop keeps tool calling. `agent/moa_loop.py:1-6@v0.22.2`
  - Default preset: references `openai-codex/gpt-5.5` and `openrouter/deepseek/deepseek-v4-pro`; aggregator `openrouter/anthropic/claude-opus-4.8`. `config_defaults.py:1416-1445@v0.22.2`
- **Worktrees**: `hermes --worktree/-w` flag and a `hermes worktree` subcommand. `hermes_cli/_parser.py:187,282@v0.22.2`; `hermes_cli/subcommands/worktree.py:1-7@v0.22.2`; `website/docs/user-guide/git-worktrees.md` [Verified/Docs]
- Bot Mode / hosted rooms (agent-to-agent messaging protocol; `bot_mode_protocol: True`) and an `a2a` platform plugin exist. `config_defaults.py:193-194@v0.22.2`; `plugins/platforms/a2a` [Verified presence]

## D12 — Surfaces

**In plain words.** You can use Hermes from:
- a classic CLI and an Ink TUI;
- an Electron desktop app and a web dashboard;
- IDEs (via ACP) and an OpenAI-compatible API server;
- about 30 messaging platforms through a single gateway, plus voice.

- CLI: `cli.py` HermesCLI + `hermes_cli/cli_*_mixin.py`. The slash `COMMAND_REGISTRY` has **102 commands** (counted by import). `hermes_cli/commands.py` [Verified]
- TUI: `ui-tui/` (Ink) + `tui_gateway/` JSON-RPC (`hermes --tui` / `HERMES_TUI=1`); it "fully replaces the classic prompt_toolkit CLI". `tui_gateway/AGENTS.md:3-5@v0.22.2` [Verified]
- Desktop: `apps/desktop/` Electron (Bot Mode, slash palette, Bot Screen, multi-connection), with an auto-update feed. `AGENTS.md:161@v0.22.2`; `apps/desktop/` listing [Verified]
- Web dashboard: `web/`, which "embeds the real TUI". `AGENTS.md:160@v0.22.2` [Verified]
- Messaging gateway:
  - **Built-in adapters (gateway/platforms/)**: api_server (OpenAI-compatible `/v1`), webhook, msgraph_webhook, signal, weixin, bluebubbles (iMessage), qqbot, whatsapp_cloud, yuanbao. `gateway/platforms/` listing [Verified]
  - **Bundled platform plugins (plugins/platforms/, 21)**: a2a, buzz, dingtalk, discord, email, feishu, google_chat, irc, line, matrix, mattermost, ntfy, photon, raft, simplex, slack, sms, teams, telegram, wecom, whatsapp. `plugins/platforms/` listing [Verified]
  - Platform enum: local, telegram, discord, whatsapp, whatsapp_cloud, slack, signal, mattermost, matrix, email, sms, dingtalk, api_server, webhook, msgraph_webhook, feishu, wecom, wecom_callback, weixin, bluebubbles, qqbot, yuanbao, relay (+ irc). `gateway/config.py:217-250@v0.22.2` [Verified]
  - Home Assistant is a catalog plugin. `architecture.md:127@v0.22.2` [Docs]
  - Total ≈ 30 distinct messaging adapters [Inference from the two listings]. Docs say "25+". `architecture.md:229@v0.22.2` [Docs]
- Voice:
  - `text_to_speech` core tool; streaming TTS (`streaming-tts.md`); TTS/transcription provider registries (`agent/tts_registry.py`, `agent/transcription_registry.py`).
  - `voice_turn_route` (auxiliary.voice_chat); Discord voice doctor script.
  - `toolsets.py:22@v0.22.2`; `conversation_loop.py:1615-1618@v0.22.2` [Verified]
- Other surfaces: `hermes mcp serve` (D6), ACP (D6), `batch_runner.py` (D14), the Python library (`agent.chat()` / `run_conversation()`). `agent-loop.md:24-39@v0.22.2` [Docs]
- i18n: `locales/` (35 files) and `agent/i18n*.py`; README in en/es/ur-pk/zh-CN. [Verified]

## D13 — Extensibility

**In plain words.** The project deliberately grows through **plugins and skills**, not core. Plugins can add tools, hooks, CLI commands, platforms, model providers, memory, context engines, image/video/web-search/browser backends and more. A 614-entry plugin catalog and a 65-entry MCP catalog sit in the repo.

- "It is extended primarily through **plugins and skills**, not by growing the core." `AGENTS.md:15-16@v0.22.2` [Verified]
- Plugin discovery: `~/.hermes/plugins/` (user), `.hermes/plugins/` (project), pip entry points; `register(ctx)` API. `architecture.md:233-235@v0.22.2`; `hermes_cli/plugins.py:7@v0.22.2` [Docs/Verified]
- **44 plugin hook names** (`VALID_HOOKS`, counted by import). They include:
  - Tool: `pre_tool_call`, `post_tool_call`, `transform_terminal_output`, `transform_tool_result`.
  - LLM and stream: `transform_llm_output`, `pre_llm_call`, `post_llm_call`, `on_stream_start/delta/end`, `pre_api_request`, `post_api_request`, `api_request_error`, `pre/post_auxiliary_call`, `transform_api_error_classification`.
  - Session, skills and subagents: `on_session_start/end/finalize/reset`, `on_skill_lifecycle`, `subagent_start/stop`.
  - Gateway and loop: `pre_gateway_dispatch`, `post_gateway_admission`, `agent_loop_stopped`, `pre_verify`.
  - Approval and human input: `pre_approval_request`, `post_approval_response`, `on_human_input_request/resolved`.
  - Kanban: `kanban_task_*`, `on_kanban_worker_*`.
  - `hermes_cli/plugins.py:109-193@v0.22.2` [Verified]
- Plugin runtime limits: `hook_callback_timeout: 30`s, `load_timeout_seconds: 10`, `clone_timeout_seconds: 300`, `auto_update_check_hours: 24`, `auto_apply: False`, `isolation: in_process | host` (+ launcher argv for sandboxing). `config_defaults.py:1711-1741@v0.22.2` [Verified]
- Shell-script hooks keyed by event (`hooks: {}`) with consent. `config_defaults.py:1742-1750@v0.22.2` [Verified]
- Plugin kinds with developer guides: model-provider, memory-provider, context-engine, browser-provider, image-gen, video-gen, web-search, secret-source, terminal-environment, middleware, observer-hooks, desktop plugin SDK, relay connector. `website/docs/developer-guide/*.md` listing [Docs]
- In-tree plugin dirs: browser, computer_use, context_engine, cron_providers, dashboard_auth, disk-cleanup, google_meet, hermes-achievements, image_gen, kanban, memory, model-providers, platforms, security-guidance, teams_pipeline, video_gen, web. `plugins/` listing [Verified]
- **Plugin catalog** `plugin-catalog/`: **614** YAML entries with admission rules in its README (e.g. `claude-code-theme`, `cloudflare-sandbox`, `brave-search`, `account-pool`). `plugin-catalog/` listing; `AGENTS.md:158@v0.22.2` [Verified]
- MCP catalog `optional-mcps/` (65); Skills Hub (D10); quick commands (`quick_commands: {}`, exec-only, bypass the loop); personalities; skins. `config_defaults.py:1704-1705,1751-1753@v0.22.2` [Verified]
- Import from other agents (`user-guide/import-from-other-agents.md`); profile distributions. [Docs]

## D14 — Observability, evals, tests, CI

**In plain words.** The test suite is huge: about 5.7K Python test files with roughly 46K test functions, plus about 1.8K JS/TS test files. The test runner isolates every file. `evals/` is a set of probe/benchmark harnesses rather than a single benchmark score. CI runs about 55 GitHub workflows. Monitoring export is off by default and content-free.

- Python tests: **5,737** `test_*.py` files under `tests/`; **46,257** `def test_` occurrences (lexical, not executed). JS/TS tests: **1,832** `*.test.ts(x)` files across tests-js / apps / ui-tui / web. [Verified counts]
- Docs claim "~25,000 tests across ~1,250 files". `architecture.md:136@v0.22.2` [Docs, stale]
- Runner: `scripts/run_tests.sh` gives **per-file process isolation** (no xdist), `TZ=UTC`, `LANG=C.UTF-8`, `PYTHONHASHSEED=0`, and blanked env vars. `scripts/run_tests.sh:1-14@v0.22.2` [Verified]
- Test policy (all [Verified]):
  - Banned: change-detector tests and "never read source code in tests"; tests never write to `~/.hermes/`.
  - OS-specific behaviour is tested on that OS via `@pytest.mark.platforms(...)`, never by faking `sys.platform`.
  - A `wine2e` lane for live Windows process topology.
  - `tests/AGENTS.md:5-9,62,92-100,126,139@v0.22.2`; `AGENTS.md:143-146@v0.22.2`
- `evals/` (221 files): probe directories and scripts:
  - Compaction, native_compaction, prompt_footprint, token_accounting, tool_search, core_tool_deferral, provider_fallback, provider_wire, reasoning_replay, plugin_isolation, gateway*, liveness, codebase_navigability, toolperf_abeval, desktop_bug_campaign, session_search_schema, kanban probes, `goal_command_parity.py`, `heartbeat_idle_wire.py`, `vault_fill_live_e2e.py`, `fanout_resource_bench.py`, …
  - They are regression and A/B harnesses, not a public leaderboard. `evals/` listing [Verified presence / Inference on role]
- CI: **55 workflows**, including:
  - Core: `tests.yml`, `tests-os.yml`, `js-tests.yml`, `rust-tests.yml`, `lint.yml`, `nix.yml`, `docker.yml`.
  - Live/remote: `live-providers.yml`, `remote-backends.yml`.
  - Desktop and install E2E: `e2e-desktop*.yml`, `install-e2e*.yml` (macOS / Windows), `termux-verify.yml`.
  - Prompt and skills: `prompt-surface-diff.yml`, `skills-index*.yml`.
  - Repo hygiene: `blob-size-check.yml`, `case-collision-check.yml`, `history-check.yml`, `profile-artifact-check.yml`.
  - Security: `osv-scanner.yml`, `supply-chain-audit.yml`.
  - Release: `canary-release.yml`, `stable-release.yml`.
  - `.github/workflows/` listing [Verified]
- Observability (all [Verified]):
  - `monitoring.gateway_health_export.enabled: False`; OTLP export `export.otlp.enabled: False`. "Content-free by construction: no prompts, messages, tool args/results … Nothing is collected or sent until enabled with an endpoint." `config_defaults.py:2077-2098@v0.22.2`
  - Context/compression telemetry modules are local (`agent/context_compressor_telemetry.py`, `conversation_compression_telemetry.py`).
  - `hermes logs`, `hermes doctor`, `/debug share`. `config_defaults.py:242-247@v0.22.2`
  - Optional MoA JSONL traces (`save_traces: False`). `config_defaults.py:1422-1425@v0.22.2`
- Training-data tooling (all [Verified]):
  - `batch_runner.py`: a multiprocessing pool over JSONL prompts, `--resume` checkpoints, ShareGPT-style trajectories, aggregated tool stats. `batch_runner.py:1-6@v0.22.2`
  - `trajectory_compressor.py`: protects the head and the last N turns, summarizes the middle, and never splits `<tool_call>`/`<tool_response>`. `trajectory_compressor.py:1-11@v0.22.2`
  - `mini_swe_runner.py` is present (not read).

## D15 — Deployment & ops

**In plain words.** Install options:
- a one-line curl/PowerShell script;
- Docker (s6-supervised);
- Nix (flake with NixOS and Home-Manager modules);
- a desktop bundle.

Hermes manages its own Python environments through an internal package manager (`pm/`). `hermes update` handles upgrades. Telemetry is opt-in only.

- Install: `curl -fsSL https://hermes-agent.nousresearch.com/install.sh | bash`; Windows: `iex (irm https://hermes-agent.nousresearch.com/install.ps1)`. `README.md:40,50@v0.22.2` [Verified]
- Also `setup-hermes.sh/.ps1` and `scripts/install.{sh,ps1,cmd}`; `apps/bootstrap-installer`. [Verified]
- Docker:
  - Multi-stage `Dockerfile` with SHA-pinned base `debian:13.4@sha256:e2d08da6…`. `Dockerfile:7@v0.22.2` [Verified]
  - `docker-compose.yml` / `docker-compose.windows.yml`; `docker/` has an s6-rc.d supervision tree, tini shim, entrypoint dispatch, profile-reconcile cont-init and a sandbox-desktop Dockerfile. `docker/` listing [Verified]
- Nix: `flake.nix` + `nix/` (nixosModules, homeManagerModules, packages, devShell, sandbox, desktop, tui, web, checks). `nix/` listing [Verified]
- Package management:
  - `pm/` (own AGENTS.md) owns pinned environments; "never mutate a Hermes environment with raw pip/uv"; `hermes pm lock` → commit `uv.lock`. `AGENTS.md:134-136@v0.22.2` [Verified]
  - `security.allow_lazy_installs: True` lazily installs backend extras. `config_defaults.py:1794-1797@v0.22.2` [Verified]
- Updates and releases:
  - `hermes update` (`hermes_cli/post_update.py`, `_update_takeover.py`, old-updater compatibility).
  - Canary tags are cut daily (e.g. `v0.22.1+canary.20261015T070051Z` in local tags); stable release workflow.
  - Desktop auto-update feed (`apps/desktop/update-feed.cjs`); macOS bundle updates doc.
  - [Verified files/tags]
- Gateway ops: `restart_drain_timeout: 0`, `cron_drain_timeout: 30`, `restart_after_turn_timeout: 1800`, `build_wait_timeout: 600`, `runtime.nofile_soft_limit: 4096`, startup warm-up gate 20s, auto-continue note after crash (freshness 3600s). `config_defaults.py:51-52,107-129,252-267@v0.22.2` [Verified]
- Platforms: macOS, Linux, Windows native and WSL, Termux (CI lane). `website/docs/user-guide/windows-native.md`; `.github/workflows/termux-verify.yml` [Docs/Verified]
- Telemetry: outbound telemetry needs an opt-in gate (a contribution rule); monitoring is off by default. `AGENTS.md:44@v0.22.2`; `config_defaults.py:2077-2086@v0.22.2` [Verified]
- `security.allow_data_training_tiers_noninteractive: False` guards against unattended use of vendor tiers that train on prompts. `config_defaults.py:1776-1778@v0.22.2` [Verified]

## D16 — Repo harness practices

**In plain words.** The repo is tuned for AI coding agents working on it:
- a size-capped root AGENTS.md hub plus 14 per-area AGENTS.md files;
- a one-command local mirror of CI lint (`scripts/check`);
- a "code-health ratchet" that stops functions and files from growing past fixed limits.

- AGENTS.md hub: **15 AGENTS.md files total** = root + 14 area files (agent, apps/desktop, apps/desktop/src, cron, gateway, hermes_cli, hermes_platform, plugins, pm, skills, tests, tools, tui_gateway, web). `git ls-tree` [Verified]
- The brief's "15 per-area" = root + 14. [Verified correction]
- Size caps: `scripts/check` caps the root AGENTS.md at **12k chars** and every root-to-area chain at **30k**, "so it loads whole on 128k+ models". The root file is 11,952 bytes at v0.22.2. `AGENTS.md:5-6@v0.22.2` [Verified]
- `python scripts/check` runs every blocking CI lint check locally with CI's pinned tools (lint, ruff, code-health ratchet, case-collision, lazy-deps, profile-archive, blob-size). Supports `--staged`, `--commit REV`, `--install-hook pre-commit|pre-push`. `scripts/check:1-16@v0.22.2` [Verified]
- Code-health ratchet (`scripts/code_health/config.py`, `ENFORCEMENT = "blocking"`):
  - New functions: cyclomatic complexity ≤ 20, ≤ 300 lines, nesting ≤ 6.
  - Files ≤ 2,000 lines.
  - Units already over the limit "may only go down".
  - Waiver syntax: `# health: allow <RULE> -- <why>` (`# noqa` doesn't waive).
  - `AGENTS.md:66-69,112-115@v0.22.2`; `scripts/code_health/config.py:18@v0.22.2` [Verified]
- Custom invariant linters with HX codes: HX001 (never hardcode `~/.hermes`), HX002/4/5/12 (profile scope), HX003 (never infer process identity from argv). Scripts include `check_no_tmp_literals.py`, `check_profile_scope_patterns.py`, `check_subprocess_stdin.py`, `check_config_yaml_writers.py`, `check-windows-footguns.py`, `check_bash_shebangs.py`. `AGENTS.md:123-132@v0.22.2`; `scripts/` listing [Verified]
- Process rules: rebase before merge (a stale squash silently reverts fixes); 1–2 invariant tests per fix "proven red on the base"; "salvage by cherry-pick so authorship survives"; moving a symbol means fixing its docs in the same PR; no re-export shims. `AGENTS.md:35-39,116-117,141-142@v0.22.2` [Verified]
- `lefthook.yml` and `justfile` exist; `prompt-surface-diff.yml` CI catches system-prompt changes (protecting the cache invariant). [Verified presence / Inference on purpose]
- Workflow rules (PR / issue / review / salvage) live in a `hermes-agent-dev` skill, not in AGENTS.md. `AGENTS.md:168-169@v0.22.2` [Verified]

---

## Distinctive ideas (worth stealing)

1. **Prompt-cache-first design as an invariant.** Byte-stable three-tier system prompt; slash commands that touch the prompt defer to the next session; background forks reuse the parent's cached prefix; CI diffs the prompt surface. (`AGENTS.md:20-23`, `system_prompt.py:1-8`, `background_review.py:1-6`)
2. **Facade + sibling modules with a blocking complexity ratchet** and size-capped AGENTS.md chains (12k / 30k) designed for agent contributors. (`AGENTS.md:5-6,101-117`)
3. **Self-improving skills loop:**
   - post-turn review fork → `skill_manage` writes;
   - an audit ledger with hash rollback;
   - a curator that archives but never deletes;
   - a Skills Hub that federates 9 source types, including OpenClaw's ClawHub.
4. **Tool Search with tiered deferral.** It keeps the model-facing tool array small even with thousands of MCP tools. (`config_defaults.py:1986-2022`)
5. **Programmatic Tool Calling (`execute_code`).** A persistent kernel calls tools over RPC, and its iterations are refunded from the budget. (`code_execution_tool.py:1-11`, `iteration_budget.py:24-49`)
6. **Layered approvals:**
   - 12 hardline floors are never bypassable;
   - 110 dangerous patterns go to an LLM guardian, which has a denial circuit breaker;
   - user deny-globs still hold under YOLO, which is itself frozen at import;
   - protected instruction files always need approval.
7. **Model-blind credential vault.** The model sees only opaque handles; values are filled server-side.
8. **One process, many profiles**, with explicit profile-scope binding and a single cron ticker across profiles.
9. **Loop primitives family:** `/goal` (fail-open judge + 20-turn budget), `/loop` (`LOOP_COMPLETE`, backoff), `/heartbeat` (idle-only), cron, and a Kanban dispatcher with fingerprinted worker liveness.
10. **Training-data lineage.** Batch runner + trajectory compressor + ShareGPT trajectories (Nous's RL/data heritage).

## Weaknesses / risks

- **Default posture is permissive** (all [Verified]):
  - `terminal.backend: local` (no sandbox), plus `agent.max_turns: None` (unlimited) and `delegation.max_iterations: 250`.
  - Loop guardrail hard stops are off for interactive sessions.
  - SECURITY.md itself says the local backend with untrusted inputs is "outside the supported security posture". `SECURITY.md:116-119`
- **Docs drift on load-bearing defaults:**
  - `agent-loop.md` (500 turns / 50 subagent iterations) vs code (unlimited / 250); the `iteration_budget.py` docstring is stale.
  - `architecture.md` claims ~25K tests / 1,250 files vs 46K / 5.7K; "18+ providers" vs 80 registry keys.
  - The task brief itself carried wrong curator numbers (30 / 90 vs 14 / 30).
- **Enormous surface and complexity:** 18.6K files; `context_compressor.py` is 5,852 lines; Electron `main.ts` is 19,750 lines; 3,182-line config defaults with hundreds of knobs. High cognitive load, and the regression risk is offset by a very large test suite. [Verified sizes / Inference on risk]
- **Smart approvals depend on an LLM judging attacker-influenced strings.** The project concedes these are heuristics. `guard_agent_created: False` means agent-written skills are not scanned by default. (`config_defaults.py:1470-1473`)
- **`auth.adopt_external_logins: True`** borrows Codex / Claude Code OAuth tokens by default. Single-use rotating refresh tokens can log the other tool out (acknowledged in the comment). (`config_defaults.py:1755-1760`)
- **Plugins run in-process by default** (`isolation: in_process`), so a plugin crash or compromise equals agent-process compromise. (`config_defaults.py:1731-1736`)
- **Fail-open goal judge**: it relies on the 20-turn budget to bound spend. (`goals.py:1-6`)
- Background delegation is process-local and is lost on restart. (`cron/AGENTS.md:94-95`)

## Open questions (not resolvable from this read)

- Real-world task success, cost and latency vs OpenClaw / Odysseus: no benchmark results ship in `evals/` that I examined. Needs runs.
- Full-history metrics (age, commits, contributors, release cadence before Oct 2026): blocked by the shallow clone. Needs `git fetch --unshallow` or GitHub API (not done; repo is the user's live install).
- Exact behaviour of `codex_app_server` mode and `copilot-acp`, and how tool approvals map there (files present, not traced).
- What `agent/learning_graph.py`, `insights.py` and `verification_evidence.py` feed into prompts at runtime (not read).
- How many of the 46K lexical tests run in the default CI lane vs OS-specific / live lanes (workflows not dissected).
- Exact tool counts after toolset resolution per platform, and how many kanban/browser tools are registered via loops vs `register(name=…)` literals (the AST scan found 45 literal registrations in `tools/`; the rest are dynamic).
