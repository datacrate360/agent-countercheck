# claude-mem — research report

| Field | Value |
|---|---|
| Repo | https://github.com/thedotmack/claude-mem (shallow clone at `~/.hermes/profiles/datacrate360/cache/scratch/agents/claude-mem`) |
| Commit | `b85cd12` (2026-10-17 12:32 -0700, "fix(install): restart the worker after a non-interactive update (#4860)") |
| Version | 14.0.1 (`package.json:3`) |
| License | Apache-2.0 (`LICENSE`, `NOTICE`, `package.json:19`) — Copyright 2026 Alex Newman |
| Size | 2,515 tracked files; `src/` is about 149k lines of TS; 925 `*.test.*` files under `tests/` |
| Files read (main ones) | `README.md`, `CLAUDE.md`, `package.json`, `plugin/hooks/hooks.json`, `plugin/.mcp.json`, `plugin/modes/code.json`, `src/cli/handlers/{observation,session-init,context,file-context,summarize,session-end}.ts`, `src/cli/spool-hook-event.ts`, `src/cli/adapters/{index,claude-code,raw}.ts`, `src/shared/{hook-spool,SettingsDefaultsManager,cmem-gateway}.ts`, `src/services/worker/{hook-spool-drain,SessionManager,SessionMessageBuffer,ClaudeProvider,provider-dispatch}.ts`, `src/services/worker/http/shared.ts`, `src/services/worker/http/routes/SessionRoutes.ts`, `src/services/worker/agents/ResponseProcessor.ts`, `src/services/worker/validation/PrivacyCheckValidator.ts`, `src/services/worker/search/{SearchOrchestrator,strategies/*}.ts`, `src/sdk/{prompts,parser,hardened-options}.ts`, `src/services/sqlite/{SessionStore,SessionSearch,connection}.ts`, `src/services/sync/{ChromaSync,ChromaMcpManager,CanonicalContent}.ts`, `src/services/context/{ContextBudget,ContextBuilder,ObservationCompiler,types}.ts`, `src/servers/{mcp-server,mcp-tool-visibility}.ts`, `src/utils/tag-stripping.ts`, `src/services/telemetry/{consent,common}.ts`, `src/npx-cli/commands/install.ts`, `src/npx-cli/cmem-memory-credentials.ts`, `openclaw/{openclaw.plugin.json,src/index.ts}`, `.github/workflows/*.yml`, `evals/read-gate/README.md`, `docs/native-prompt-identity.md`, `SECURITY.md`, `docker-compose.yml`, `transcript-watch.example.json` |

## Executive summary

- claude-mem is a **"second agent watches the first one" memory system**. Hooks capture every tool call. A background worker sends each call to a *separate, tool-less observer LLM session* (Claude Haiku 4.5 by default, via the Claude Agent SDK). That session writes XML `<observation>` records (type, title, facts, narrative, concepts, files). They are stored in SQLite with FTS5, embedded into Chroma, and a compact timeline is injected back at the next SessionStart.
- "Compression" here means **LLM summarisation of each tool call**, not a compression algorithm. The cost is about one observer LLM call per tool use, plus one per turn for the summary. By default this runs on the user's **Claude subscription** (`CLAUDE_MEM_CLAUDE_AUTH_METHOD=subscription`).
- The pipeline is well engineered for durability. Hook events are written atomically to a disk spool, then drained into the worker (exactly once, through a `hook_spool_consumed` table). After that hand-off the queue is **in RAM only**. A worker crash relies on "transcript replay" for recovery.
- Injection is a **character-budgeted timeline**. It is capped at 10,000 characters, which is Claude Code's limit on hook stdout. Items are dropped in a fixed order until the block fits. It is not semantic per-prompt retrieval: that mode exists but is off by default.
- "Hybrid search" in the README is **not score fusion**. When Chroma is on and a query is given, Chroma answers. SQLite FTS5 only fills categories that come back empty, or answers when Chroma is disabled.
- The MCP server exposes **about 27 tools** (`mem_search`, `search`, `timeline`, `get_observations`, `save_memory`, `smart_outline/unfold/search`, corpus tools, work-state, TeamBrain…). The README describes "4 tools".
- Adapters: Claude Code (hooks.json), Codex, Cursor, Kimi, Windsurf, Antigravity, a generic `raw` adapter, a declarative transcript-JSONL watcher, and native plugins for OpenClaw, OpenCode, Pi and DeepSeek Harness. **No Hermes adapter exists in this repo.** Hermes appears only as a UI label, test fixtures and a doc line about a "Hermes provider".
- Privacy: `<private>…</private>` and similar tags are stripped at one choke point. Secret redaction is **opt-in** (`CLAUDE_MEM_REDACT_ENABLED=false`). Anonymous PostHog telemetry is **opt-out** (on by default).
- Upsell: the interactive installer **pre-selects "CMEM Pro"**. Sign-in writes cloud-sync credentials, so observations, summaries **and user prompts** sync to `sync.cmem.ai`. The observer then runs on the cmem gateway. Headless or CI installs default to local Claude.
- Recommendation for omni-agent: **port the idea and call over HTTP/MCP during the trial; do not embed it.** Apache-2.0 is compatible with MIT if NOTICE is kept, but the codebase is huge, fast-moving and full of vendor-specific paths.

## Core pipeline diagram

```
 Host agent (Claude Code / Codex / OpenClaw / raw)
   │ SessionStart ─► `claude-mem hook claude-code context` ─► GET /api/context/inject ─► additionalContext (≤10k chars)
   │ UserPromptSubmit ─► session-init ─► POST /api/sessions/init  (prompt tag-stripped; empty ⇒ "private" turn)
   │ PreToolUse(Read|Bash|Grep|Glob) ─► file-context: prior observations for file; may DENY whole-file Read ≥32KB
   │ PostToolUse(*) ─► observation handler ─► atomic JSON file in ~/.claude-mem/state/hook-spool/
   │ Stop ─► summarize (last assistant msg from transcript) ─► spool
   ▼
 Worker (Bun, Express on 127.0.0.1:37700+uid%100)
   HookSpoolDrainer (fs.watch + nudge + 30s sweep) ─► ingestObservation()
     filters: skip-tools, bash regex, subagent, excluded project, privacy row
     stripMemoryTags(tool_input/response) ─► tool_uses (raw receipt, SQLite)
     SessionManager.queueObservation ─► in-RAM SessionMessageBuffer
   ▼
 Observer generator (ClaudeProvider: @anthropic-ai/claude-agent-sdk query(), tools:[], resume=memory_session_id)
   init prompt (mode JSON: system_identity/observer_role/…) + <observed_from_primary_session> per tool use
   ◄─ <observation><type/><title/><subtitle/><facts/><narrative/><concepts/><files_read/><files_modified/></observation>
      or <skip_summary reason=…/>  |  <summary><request/><investigated/><learned/><completed/><next_steps/><notes/></summary>
   ▼
 ResponseProcessor ─► SessionStore.storeObservations (SQLite observations / session_summaries + FTS5 triggers)
                   ─► ChromaSync (chroma-mcp via uvx, collection cm__<project>, one doc per narrative/fact)
                   ─► CloudSync.notify() (only if sync token + hub URL set) ─► SSE broadcast to viewer
   ▼
 Retrieval: MCP server (stdio, plugin/.mcp.json) ─► worker HTTP /api/mem-search, /api/search, /api/timeline, …
```

---

## T1 — Identity

In plain words: claude-mem is a TypeScript plugin plus a local background service. It gives coding agents memory that lasts across sessions. One author maintains it and releases often. A hosted paid tier (CMEM Pro / TeamBrain on cmem.ai) is built into the installer.

- Purpose: "Memory compression system for Claude Code - persist context across sessions". `package.json:4` [Verified]
- License Apache-2.0, author Alex Newman, NOTICE file present. `package.json:18-19`, `NOTICE:1-6` [Verified]
- Stack: TypeScript on Bun (worker runtime and test runner) and Node ≥20.12. Express HTTP server. SQLite through Bun. Chroma through Python `chroma-mcp` launched with `uvx`. Claude Agent SDK. PostHog. Optional Postgres/Valkey "server runtime". `package.json:63-65`, `src/services/server/Server.ts:211`, `src/services/sync/ChromaMcpManager.ts:131-136`, `docker-compose.yml:55-72` [Verified]
- Size: 2,515 files; `src/` about 148,877 lines; 925 test files. `git ls-files`, `wc -l` [Verified]
- Committed build bundles: `plugin/scripts/worker-service.cjs` is about 4.0 MB and `mcp-server.cjs` about 0.8 MB. Plugin installs run these committed bundles directly. `plugin/scripts/*.cjs`, `.github/workflows/ci.yml:32-36` [Verified]
- Maturity: v14, more than 4,800 issues/PRs referenced (#4860 in the HEAD commit). Code comments cite issue numbers everywhere. This is a mature but high-churn codebase. `git log -1` [Verified]
- Company/upsell: CMEM Pro observer gateway, cloud sync hub `https://sync.cmem.ai`, TeamBrain. `src/shared/SettingsDefaultsManager.ts:33`, `README.md` "TeamBrain" bullet [Verified]

## T2 — Core mechanism (capture → compress → store → retrieve → inject)

In plain words: every tool call your agent makes is written to a small file on disk. A background worker picks it up. The worker asks a second, tool-less Claude session: "what durable fact did the main agent just learn or change?" The answer comes back as structured XML and is saved in SQLite and a vector index. At the start of the next session, a short timeline of the most recent observations is put into the prompt.

**1. Capture (hooks)**
- Claude Code hook registration: `Setup`, `SessionStart` (3 entries: `claude-mem start` async, `hook claude-code context`, `ensure-launcher.cjs`), `UserPromptSubmit` → `session-init`, `PostToolUse` and `PostToolUseFailure` (matcher `*`, async) → `observation`, `PreToolUse` (Read/Bash/Grep/Glob/memory_search) → `file-context`, `Stop` → `summarize` (async), `SessionEnd` → `session-end`. `plugin/hooks/hooks.json:4,17,61,77,95,113,130,147` [Verified]
- Entry point: `claude-mem hook <platform> <event>`. `src/services/worker-service.ts:1761-1780` → `hookCommand()`. `src/cli/hook-command.ts` [Verified]
- The PostToolUse handler drops memory-retrieval tools (to avoid recursion), excluded projects and subagent observations (when configured). It then spools the event. `src/cli/handlers/observation.ts:45-81,129` [Verified]
- Spool = one atomic JSON file per event in `<DATA_DIR>/state/hook-spool/<kind>-<key>.json`. `src/shared/hook-spool.ts:164,296,345` [Verified]
- An *observation* at capture time is `{contentSessionId, platformSource, toolName, toolInput, toolResponse, cwd, agentId, agentType, toolUseId}`. `src/cli/handlers/observation.ts:20-33` [Verified]

**2. Ingest (worker)**
- `HookSpoolDrainer` runs on fs.watch, on a hook "nudge" and on a 30 s safety sweep. Each entry goes to the same `ingestObservation()` that the HTTP route uses. A `hook_spool_consumed` table makes the hand-off exactly-once. `src/services/worker/hook-spool-drain.ts:8,18-42,66-77,90-123`, `src/services/sqlite/SessionStore.ts:2088` [Verified]
- Filters in `ingestObservation`, in order: recursion guard, excluded project, `CLAUDE_MEM_SKIP_TOOLS` (default `ListMcpResourcesTool,SlashCommand,Skill,TodoWrite,AskUserQuestion`), `CLAUDE_MEM_SKIP_BASH_PATTERNS` regex, subagent skip, `session-memory` file paths, privacy check. `src/services/worker/http/shared.ts:148-264`, `src/shared/SettingsDefaultsManager.ts:433` [Verified]
- Privacy check: if the `user_prompts` row for this prompt number exists but is empty after tag stripping, the whole turn is "private" and skipped. If the row is missing, ingest goes ahead (race tolerance). `src/services/worker/validation/PrivacyCheckValidator.ts:21-50` [Verified]
- Tool input and output are tag-stripped (and redacted, if redaction is enabled). They are written to `tool_uses` as a raw "receipt" and pushed to an in-RAM per-session buffer. `src/services/worker/http/shared.ts:270-319`, `src/services/worker/SessionManager.ts:472-500` [Verified]
- The buffer is deliberately not durable: "If the worker dies, the buffer is gone and recovery is a transcript replay". `src/services/worker/SessionMessageBuffer.ts:30-50` [Verified]

**3. Compress (observer LLM)**
- `ClaudeProvider` runs one long-lived `query({prompt: messageGenerator, options})` from `@anthropic-ai/claude-agent-sdk` per session. It uses `resume: memorySessionId`, so the observer keeps conversational memory of earlier tool calls in the same session. `src/services/worker/ClaudeProvider.ts:34,448-476` [Verified]
- The observer is hardened as "belt, suspenders, braces": `tools: []`, `disallowedTools`, `permissionMode: 'dontAsk'`, and an isolated env/credentials file `~/.claude-mem/.env`. `src/sdk/hardened-options.ts:8-60`, `src/services/worker/ClaudeProvider.ts:456` [Verified]
- Init prompt = mode-specific instructions (`system_identity`, `observer_role`, `spatial_awareness`, `recording_focus`, `skip_guidance`, XML skeleton) + optional prior context + `<observed_from_primary_session><user_request>`. The static prefix is placed first on purpose, so provider prompt caches hit. `src/sdk/prompts.ts:76-112` [Verified]
- Mode file `plugin/modes/code.json` defines observation types `bugfix, feature, refactor, change, discovery, decision, security_alert, security_note, sensitive` and concepts `how-it-works, why-it-exists, what-changed, problem-solution, gotcha, pattern, trade-off`. There are about 30 language variants (`code--zh`, …). `plugin/modes/code.json` [Verified]
- Key prompt text: "You are a Claude-Mem, a specialized observer… Record what was LEARNED/BUILT/FIXED/DEPLOYED/CONFIGURED… SILENT BY DESIGN… NO CONTACT". `plugin/modes/code.json` (prompts.system_identity/observer_role) [Verified]
- Per-tool prompt: `<observed_from_primary_session><what_happened>TOOL</what_happened><occurred_at/><working_directory/><parameters/><outcome/>`. Each field is truncated to 16,000 chars by default (window-aware), and image payloads are stripped. The model must reply with `<observation>` blocks or `<skip_summary reason="noise"/>`. `src/sdk/prompts.ts:163,394-443` [Verified]
- Oversized fields can first be condensed by an extra LLM call (`optimizeObservationFields`). `src/services/worker/ClaudeProvider.ts:1230-1238`, `src/services/worker/field-optimizer.ts` [Verified]
- Turn summary: on Stop, `buildSummaryPrompt` sends the last assistant message and asks for `<summary><request/><investigated/><learned/><completed/><next_steps/><notes/></summary>`. `src/sdk/prompts.ts:450-482`, `src/cli/handlers/summarize.ts:8` [Verified]
- Parsing is regex-based XML extraction. `src/sdk/parser.ts:62,69,121` [Verified]

**4. Store**
- `processAgentResponse` → drops untitled observations → `sessionStore.storeObservations(...)` → confirms the claimed batch *before* any side effects (no duplicates on replay) → `ChromaSync.syncObservation` → `CloudSync.notify()` → SSE broadcast. `src/services/worker/agents/ResponseProcessor.ts:700-740,1060,1084-1086` [Verified]

**5. Retrieve and inject**
- SessionStart: the `context` handler calls `GET /api/context/inject?projects=…&cwd=…`. The result goes back as `hookSpecificOutput.additionalContext`, with memory instructions and an optional TeamBrain brief added. `src/cli/handlers/context.ts:2,190,257-338` [Verified]
- The builder selects the newest N observations (`CLAUDE_MEM_CONTEXT_OBSERVATIONS=50`) and the last 10 session summaries. Optional ACT-R reinforcement ranking is off by default (`REINFORCE_ALPHA=0`). `src/services/context/ObservationCompiler.ts:77-110,83-87`, `src/shared/SettingsDefaultsManager.ts:412,500` [Verified]
- `fitContextToBudget` re-renders until the block is ≤10,000 characters. It drops content in this order: full narratives → last summary → last message → halve sessions → halve observations. `src/services/context/ContextBudget.ts:9,47-64,72` [Verified]
- Optional per-prompt semantic injection (`CLAUDE_MEM_SEMANTIC_INJECT=false` by default) adds the top-5 Chroma matches on UserPromptSubmit. `src/cli/handlers/session-init.ts:331-342`, `src/shared/SettingsDefaultsManager.ts:533` [Verified]
- PreToolUse "File Read Gate": for a file ≥1.5 KB that has prior observations, the file's observation timeline is added as context. For ≥32 KB, a whole-file `Read` is **denied** and the agent is pointed to `smart_outline`/`smart_unfold`/targeted reads. `src/cli/handlers/file-context.ts:23,33,163-173` [Verified]

## T3 — Architecture and process model

In plain words: short-lived hook processes start on every agent event and exit quickly. One long-running local worker does all the real work. It holds the database, talks to the LLM, runs Chroma as a child process and serves a web viewer. A separate stdio MCP server process lets the agent search memory, and it calls the worker over HTTP.

- Processes:
  1. Hook CLI (`claude-mem` launcher → `worker-service.cjs hook …`). `src/launcher/claude-mem-launcher.ts`, `src/services/worker-service.ts:1761` [Verified]
  2. Worker daemon (Bun + Express) on `127.0.0.1`, port `37700 + uid % 100`. `src/shared/SettingsDefaultsManager.ts:414-415`, `src/services/worker-service.ts:657`, `src/services/server/Server.ts:211,274` [Verified]
  3. Observer subprocesses (Claude Code CLI spawned by the Agent SDK). At most 2 at a time. `src/shared/SettingsDefaultsManager.ts:517` [Verified]
  4. `chroma-mcp` Python child launched through `uvx` (`--client-type persistent --data-dir …`) with pinned deps (`chromadb==1.5.9`, `pydantic<2.14`). `src/services/sync/ChromaMcpManager.ts:131-136,654-693` [Verified]
  5. MCP stdio server (`plugin/scripts/mcp-server.cjs`). `plugin/.mcp.json`, `src/servers/mcp-server.ts` [Verified]
  6. Optional transcript watcher (`transcript-watcher.cjs`). `plugin/scripts/`, `src/services/transcripts/watcher.ts` [Verified]
- Worker CLI verbs: `start|stop|restart|status|hook|generate|clean|transcript|adopt|project|cleanup|upload-spool|server-*|cursor|memory|kimi|antigravity-cli|--daemon`. `src/services/worker-service.ts:1490-1888` [Verified]
- Supervisor: process registry, orphan-Chroma sweep, graceful shutdown, optional idle exit (`CLAUDE_MEM_IDLE_EXIT_SEC=0` = off). `src/supervisor/*.ts`, `src/shared/SettingsDefaultsManager.ts:432` [Verified]
- Alternative "server runtime" (`CLAUDE_MEM_RUNTIME=server`): Postgres + Valkey/Redis + BullMQ-style generation jobs, better-auth API keys. Hooks POST to `/v1/events` instead of the spool. The default is `worker`. `src/cli/handlers/observation.ts:83-127`, `src/storage/postgres/schema.ts:188-333`, `src/shared/SettingsDefaultsManager.ts:638`, `docker-compose.yml:55-141` [Verified]
- Remote-worker mode: hooks can upload the spool to a remote worker over `/api/spool/ingest` with a bearer `CLAUDE_MEM_WORKER_INGEST_TOKEN`. `src/cli/spool-hook-event.ts:23-75` [Verified]

## T4 — Data model

In plain words: everything lives in one SQLite file, `~/.claude-mem/claude-mem.db`. The core tables are sessions, user prompts, observations and session summaries, plus full-text indexes. Vectors live in a separate Chroma directory with one collection per project. An observation is a structured record, not a free-text blob.

- Data dir `~/.claude-mem` (`CLAUDE_MEM_DATA_DIR`). DB `~/.claude-mem/claude-mem.db`, Chroma `~/.claude-mem/chroma/`, settings `~/.claude-mem/settings.json`. `src/shared/SettingsDefaultsManager.ts:484`, `CLAUDE.md` "File Locations" [Verified]
- SQLite pragmas: WAL, `synchronous=NORMAL`, `foreign_keys=ON`, incremental auto-vacuum. Schema versioning through `PRAGMA user_version` and the `schema_versions` table. `src/services/sqlite/connection.ts:124-134` [Verified]
- `sdk_sessions(id, content_session_id, memory_session_id UNIQUE, project, platform_source, user_prompt, started_at[_epoch], completed_at[_epoch], status)`. Later columns: `worker_port, prompt_counter, custom_title, observed_model, observed_billing, project_key_source, cwd`. `src/services/sqlite/SessionStore.ts:1221-1233,1287-2296` [Verified]
- `observations(id, memory_session_id FK, project, text, type, created_at[_epoch])`. Later columns: `prompt_number, title, subtitle, facts, narrative, concepts, files_read, files_modified, discovery_tokens, content_hash, generated_by_model, relevance_count, reinforcement_dates, last_reinforced, merged_into_project, agent_type, agent_id, metadata, occurrence_count, title_norm_key`. `src/services/sqlite/SessionStore.ts:1241-1250,1307,1519-1525,1678,1944,2004-2007,2151-2154,2217,2456-2459,2599,403-409` [Verified]
- `session_summaries(memory_session_id UNIQUE, project, request, investigated, learned, completed, next_steps, files_read, files_edited, notes, created_at…)`. `src/services/sqlite/SessionStore.ts:1257-1272` [Verified]
- `user_prompts(session_db_id, content_session_id, prompt_number, prompt_text, created_at…)` plus `native_prompt_id`/`native_prompt_hash` for idempotent host turn IDs. `src/services/sqlite/SessionStore.ts:1645-1662`, `docs/native-prompt-identity.md:1-24` [Verified]
- FTS5: `observations_fts(title, subtitle, narrative, text, facts, concepts)` and `session_summaries_fts(request, investigated, learned, completed, next_steps, notes)`, external-content tables kept in sync by triggers. Prompts have no FTS; they are searched with LIKE. `src/services/sqlite/SessionSearch.ts:240-270`, `src/services/sqlite/SessionStore.ts:1638-1640` [Verified]
- Other tables: `tool_uses` (raw I/O receipt, v51), `pending_messages` (legacy, CHECK type observation/summarize), `hook_spool_consumed`, `sync_outbox`/`sync_state`/`sync_dead_letter`/`sync_pull_quarantine`/`sync_content_outbox`, `token_df`/`dedup_meta`/`observation_dedup_candidates`, `work_state_entries`, `advisor_calls`, `telegram_wrapups`, `file_context_injections`, `work_view_reviews`. `src/services/sqlite/tool-uses.ts:147`, `src/services/sqlite/SessionStore.ts:70-94,414-432,788-981,1706,2088,2119,2335`, `src/services/sqlite/work-state.ts:31` [Verified]
- Chroma: collection `cm__<sanitised project>`. Each observation becomes several documents: `obs_<id>_narrative`, `obs_<id>_fact_<n>` (and a title doc when nothing else exists), with metadata `doc_type`, `field_type`. Default embedding is local `all-MiniLM-L6-v2`; API embedders (openai, cohere, jina, voyageai…) are optional. `src/services/sync/ChromaSync.ts:340-346,535-600`, `src/shared/SettingsDefaultsManager.ts:552-558` [Verified]
- Cloud sync payload kinds: `observation | summary | prompt | work_state | mutation`, body ≤256 KB. `src/services/sync/CanonicalContent.ts` (ContentKind) [Verified]

## T5 — Interfaces

In plain words: you talk to it in four ways. A CLI (`npx claude-mem …`) installs and diagnoses it. Hook commands are run by the host agent. A local HTTP API on the worker does everything. An MCP server gives agents search tools. Skills under `plugin/skills` wrap common workflows.

- Install CLI: `npx claude-mem install [--ide claude-code|cursor|grok-bot|kimi|opencode|pi|dsh|omp|windsurf|openclaw|t3code|codex-cli|antigravity|copilot-cli|goose|roo-code|warp] [--provider …]`, plus `doctor`, `uninstall`, `prune`, `runtime`, `server`, `teambrain`, `telemetry`. `src/npx-cli/commands/install.ts:306-563`, `src/npx-cli/commands/*.ts` [Verified]
- Hook CLI: `claude-mem hook <platform> <event>`. Platforms: `claude|claude-code|codex|cursor|kimi|windsurf|antigravity[-cli]|raw` (unknown → raw). Handler events: `context, session-init, observation, file-context, file-edit, summarize, session-end, user-message, post-compact, memory-search`. `src/cli/adapters/index.ts:10-22`, `src/cli/handlers/` [Verified]
- Worker HTTP (selection): `/api/context/{inject,preview,recent,semantic}`, `/api/sessions/{init,observations,summarize}`, `/api/search`, `/api/search/{observations,by-file}`, `/api/mem-search`, `/api/timeline[/by-query]`, `/api/observation/:id`, `/api/observations[/batch|/by-file]`, `/api/memory/{save,ingest}`, `/api/spool/ingest`, `/api/settings`, `/api/stats`, `/api/sync/status`, `/api/chroma/status`, `/api/work-state`, `/api/teambrain/*`, `/api/corpus/*`, `/api/admin/{restart,shutdown,doctor}` (localhost-only), `/api/health`. `src/services/worker/http/routes/*.ts`, `src/services/server/Server.ts:518-553` [Verified]
- MCP tools (worker runtime): `mem_search` (guided 3-step progressive disclosure with continuation token `ms_…`), `save_memory`, `important_workflow`, `search`, `timeline`, `get_observations`, `get_tool_uses`, `work_state_write`, `work_state_read`, `team_brief`, `team_search`, `session_start_context`, `smart_search`, `smart_unfold`, `smart_outline`, `build_corpus`, `list_corpora`, `prime_corpus`, `query_corpus`, `rebuild_corpus`, `reprime_corpus`. Server-runtime-only tools: `observation_add`, `observation_record_event`, `observation_search`, `observation_context`, `observation_generation_status`, `memory_add/search/context`. `src/servers/mcp-server.ts:513-1136`, `src/servers/mcp-tool-visibility.ts:4-28` [Verified]
- A separate "recall" MCP server for the server runtime exposes `search`, `context`, `recent`. `src/server/mcp/recall-mcp-server.ts:41-69` [Verified]
- Read-only MCP tools are annotated `readOnlyHint` so they work in plan mode. `src/servers/mcp-server.ts:505-509` [Verified]
- Skills shipped: `mem-search`, `smart-explore`, `make-plan`, `do`, `handoff`, `standup`, `timeline-report`, `weekly-digests`, `learn-codebase`, `knowledge-agent`, `cloud-sync`, `mode-creator`, and about 15 others. `plugin/skills/` [Verified]
- Web viewer and "My Work" UI served by the worker (SSE stream). `src/ui/viewer`, `src/ui/work-view`, `README.md` "Local My Work" [Verified]

## T6 — Agent integration (which agents, how; Hermes and OpenClaw specifically)

In plain words: for Claude Code it is a marketplace plugin with hooks and an MCP server. For other CLIs it either translates their hook JSON with an adapter or tails their transcript files. For OpenClaw it ships a native gateway plugin that calls the same worker HTTP API. There is no Hermes integration in this repo.

- Claude Code: hooks in `plugin/hooks/hooks.json` plus MCP in `plugin/.mcp.json`. Context comes back through `hookSpecificOutput.additionalContext`. `src/cli/adapters/claude-code.ts:40-90` [Verified]
- Codex, Cursor, Kimi, Windsurf, Antigravity: their own normalisers in `src/cli/adapters/*.ts`. `src/cli/adapters/index.ts:2-8` [Verified]
- Generic `raw` adapter: accepts `{sessionId|session_id, cwd, prompt, toolName|tool_name, toolInput, toolResponse, toolUseId, transcriptPath}` and passes output through unchanged. This is the drop-in for any host that can run a shell command per event. `src/cli/adapters/raw.ts` [Verified]
- Transcript watcher: declarative JSONL → event mapping (`match` path/equals/exists, `action` session_context/session_init/…). Used for Codex (`~/.codex/sessions`) and hosts without hooks (Grok Bot). `transcript-watch.example.json`, `src/services/transcripts/{watcher,processor}.ts`, `README.md` Quick Start [Verified]
- **OpenClaw**: native plugin `kind: "memory"` (`openclaw/openclaw.plugin.json`). It hooks `session_start`, `after_compaction`, `before_agent_start` → `POST /api/sessions/init`; `before_prompt_build` → `GET /api/context/inject` → returns `{appendSystemContext}`; `after_tool_call` → fire-and-forget `POST /api/sessions/observations` (tool response capped at **1,000 chars**); `agent_end` → `/api/sessions/summarize`. It registers commands `claude_mem_feed`, `claude-mem-search`, `claude-mem-recent`, `claude-mem-timeline`, `claude_mem_status`, and has an optional live "observation feed" to Telegram/Discord/Slack/Signal/WhatsApp/LINE. `openclaw/src/index.ts:757-905,977-1137`, `openclaw/openclaw.plugin.json` [Verified]
- OpenClaw default `workerPort: 37777` in the plugin schema, while the worker default is `37700 + uid%100`. The installer must reconcile the two, otherwise capture silently fails. `openclaw/openclaw.plugin.json` (workerPort), `src/shared/SettingsDefaultsManager.ts:414` [Inference — see T14]
- **Hermes**: `git grep -i hermes` finds only a UI label (`'hermes-cli': 'Hermes'`), an icon manifest, test fixtures with `platformSource: 'hermes'`, a roadmap line "Net-new hosts (Hermes #2825 …)", and the sentence "The Hermes provider requires the echoed identity…". There is no `install --ide hermes` case and the README has no Hermes mention at this commit. The local Hermes install has no claude-mem memory provider either (`~/.hermes/hermes-agent/plugins/memory/` = byterover, holographic, retaindb). `src/services/work-view/agent-view.mjs:6`, `plans/23-host-integration-contracts.md:46`, `docs/native-prompt-identity.md:36-37`, `tests/worker/http/routes/native-prompt-init.test.ts:51`, `src/npx-cli/commands/install.ts:306-563` [Verified]
- Practical Hermes path today: register `plugin/scripts/mcp-server.cjs` as an MCP server (retrieval), and either call `POST /api/sessions/{init,observations,summarize}` from a Hermes plugin/hook, or run `claude-mem hook raw <event>`. [Inference]

## T7 — LLM usage, cost, offline

In plain words: every non-skipped tool call costs one LLM request to a cheap model, and each turn adds a summary request. By default this is charged to your Claude subscription's usage limits. You can switch to Gemini, OpenRouter, any OpenAI-compatible endpoint (including local Ollama or LM Studio), Codex, or the paid CMEM gateway. Embeddings run locally by default.

- Default observer: provider `claude`, model `claude-haiku-4-5-20251001`, auth `subscription` (logged-in Claude SDK, not an API key). `src/shared/SettingsDefaultsManager.ts:411,442,450` [Verified]
- Tier routing is on: simple observations go to `haiku`, with `$TIER:fast=haiku` and `$TIER:smart=sonnet`. `src/shared/SettingsDefaultsManager.ts:536-540`, `src/services/worker/model-aliases.ts:26` [Verified]
- Other providers: Gemini (`gemini-flash-latest`, key pools), OpenRouter (default `cohere/north-mini-code:free`), OpenAI-compatible presets (nvidia-nim, deepseek, groq, together, vllm, **ollama, lmstudio**), Codex app-server. `src/shared/SettingsDefaultsManager.ts:451-476`, `src/services/worker/{GeminiProvider,OpenRouterProvider,OpenAICompatProvider,CodexProvider}.ts` [Verified]
- Fully offline is possible in principle: OpenAI-compatible → local Ollama, plus Chroma's default local MiniLM embeddings, plus telemetry off. This was not exercised. [Inference]
- Cost drivers: one call per tool use (`CLAUDE_MEM_CLAUDE_OBSERVATION_BATCH_SIZE=1`; Codex batches 8). The observer conversation is resumed, so input context grows until it is retired at 400k chars. Output is capped at 4,096 tokens per request. Oversized fields can trigger a second condensation call. `src/shared/SettingsDefaultsManager.ts:448-449,518-521,573` [Verified]
- Quota guard: the subscription observer stops at 95% of the 5-hour window and 93% of the 7-day window, then falls back to `CLAUDE_MEM_QUOTA_FALLBACK_PROVIDER=claude`. `src/shared/SettingsDefaultsManager.ts:477-483` [Verified]
- `discovery_tokens` per observation and a "savings %" display in injected context are tracked as token-economics. Tokens are estimated at 4 chars/token. `src/services/context/TokenCalculator.ts:6-37`, `src/services/context/types.ts:148`, `src/shared/SettingsDefaultsManager.ts:491-494` [Verified]
- Prompt-caching design: a static mode prefix comes first, so the cmem gateway or OpenRouter caches hit across sessions and users. `src/sdk/prompts.ts:76-85` [Verified]

## T8 — Security, privacy, legal

In plain words: by default all memory stays on your machine. But it records tool inputs and outputs, which can contain secrets, and automatic secret redaction is off unless you turn it on. Anonymous telemetry is on unless you opt out. If you accept the installer's suggested CMEM Pro sign-in, your prompts and observations also sync to the vendor's cloud.

- Privacy tags stripped at one choke point: `<private>`, `<claude-mem-context>`, `<system_instruction>`, `<system-instruction>`, `<persisted-output>`, `<system-reminder>`. A fully-private prompt marks the turn private, and observations and summaries for that turn are skipped. `src/utils/tag-stripping.ts:4-11,155`, `src/services/worker/http/routes/SessionRoutes.ts:865-879`, `src/services/worker/validation/PrivacyCheckValidator.ts:40-47` [Verified]
- Secret redaction (`CLAUDE_MEM_REDACT_ENABLED`) defaults to **false**; generic `password=`/`*_TOKEN=` redaction is a separate opt-in. Logs always use `redactForLog`. `src/shared/SettingsDefaultsManager.ts:524-528`, `src/cli/handlers/observation.ts:49-50` [Verified]
- Telemetry: PostHog `us.i.posthog.com` with a publishable key. **Opt-out**: default enabled unless `DO_NOT_TRACK`/`CLAUDE_MEM_TELEMETRY=0`/config. Exceptions are scrubbed. `src/services/telemetry/consent.ts:43,62`, `src/services/telemetry/common.ts:19-21`, `src/services/telemetry/telemetry.ts:245-330` [Verified]
- Cloud: sync is off while `CLAUDE_MEM_CLOUD_SYNC_HUB_URL`/token are empty. But every successful installer sign-in poll writes those keys (`buildSyncConnectionSettings`, "every ready poll delivers, seat or Pro"). The interactive provider prompt pre-selects `cmem`. Synced kinds include **prompts**. `src/shared/SettingsDefaultsManager.ts:562-564`, `src/services/worker/DatabaseManager.ts:37-57`, `src/npx-cli/commands/install.ts:1213-1224,1714-1751`, `src/services/sync/CanonicalContent.ts` [Verified]
- CMEM Pro observer = an OpenRouter-style client pointed at the cmem gateway, so raw tool I/O (after tag stripping) is sent to the vendor's gateway for summarisation. `src/npx-cli/cmem-memory-credentials.ts:298-311`, `src/shared/cmem-gateway.ts:41-50` [Verified]
- Worker HTTP has no auth for local callers. Protection is the loopback bind, an Origin allow-list, a DNS-rebinding Host check and localhost-only admin routes. Any local process can read all memory over HTTP. `src/services/worker/http/middleware.ts:92-209`, `src/services/server/Server.ts:518-553` [Verified code; exposure = Inference]
- Observer isolation: no tools, `dontAsk` permissions, isolated creds. A prompt injection inside tool output can still poison memory text that is later injected into the main agent (stored prompt-injection vector). `src/sdk/hardened-options.ts:8-60` [Verified mechanism; risk = Inference]
- Odd defaults that reach every install: `CLAUDE_MEM_GROK_BOT_AWARENESS_AGENT_IDS` hard-codes two specific agent UUIDs ("Pilot agents: LFG + Orifice"), and Telegram is "enabled" (it needs a token to do anything). `src/shared/SettingsDefaultsManager.ts:596,604-606` [Verified]
- Legal: Apache-2.0 allows use, modification and redistribution with NOTICE retention and change notices. It includes a patent grant. It is compatible with MIT omni-agent when used as a dependency or port, provided Apache attribution is kept for copied code. `LICENSE`, `NOTICE` [Verified/Inference]
- Product name uses "Claude"; a trademark question for any fork, not for consumption. [Inference]

## T9 — Tests, benchmarks, CI (README claims re-checked)

In plain words: the test suite is large and runs on every PR. The only real benchmark in the repo is a small eval of the file-read gate. The README's "~10x token savings" for 3-layer search has no reproducible benchmark in the repo.

- 925 test files under `tests/` (bun test). CI runs bundle checks, typecheck, build (with bundle-size guardrails), `bun test tests`, plus the openclaw suite. A Windows job runs a subset. `.github/workflows/ci.yml:35-60`, `.github/workflows/windows.yml:48` [Verified]
- Publish workflow runs `npm run smoke:clean-room` before `npm publish`. `.github/workflows/npm-publish.yml:21-24` [Verified]
- `evals/read-gate`: 5 cases × 3 runs with `claude plugin eval`. The README of that eval reports the gate saves ~35% (Sonnet) / ~32% (Opus) on a 49 KB file, and that small-file denial cost Opus +20%, which is why the 32 KB threshold exists. These numbers are self-reported and were not re-run here. `evals/read-gate/README.md`, `src/cli/handlers/file-context.ts:25-33` [Docs]
- "~10x token savings by filtering before fetching details" (`README.md:295`): repeated in `docs/public/architecture-evolution.mdx:293`. The only related figure is the separate `smart-explore-benchmark.mdx` (10x–55.8x for symbol unfold vs whole-file read). There is no retrieval-quality or recall benchmark (e.g. LongMemEval) in the repo. [Verified absence via `git grep`]
- "4 MCP tools" (`README.md` MCP section) vs about 27 actually defined. The README is stale. `src/servers/mcp-server.ts:513-1136` [Verified]
- "Hybrid semantic + keyword search": the code uses Chroma-first with an FTS fallback or supplement, not fused ranking. No RRF or other fusion code exists in `src/`. `src/services/worker/search/SearchOrchestrator.ts:101-143` [Verified]
- README Node badge says ≥20.0.0; `engines` says `>=20.12.0`. Minor drift. `README.md` badges, `package.json:63-65` [Verified]

## T10 — Operations

In plain words: one `npx` command installs it, but under the hood it needs Node, Bun, uv/Python (for Chroma), and the Claude Code CLI for the default observer. It runs a persistent local daemon and keeps a SQLite file and a Chroma directory that grow without limit unless you configure retention.

- Install: `npx claude-mem install` (or `/plugin marketplace add thedotmack/claude-mem`). OpenClaw: `curl -fsSL https://install.cmem.ai/openclaw.sh | bash`. `npm i -g claude-mem` installs only the library. `README.md` Quick Start [Docs]
- Dependencies: Node ≥20.12, Bun ≥1.1.31 (auto-installed), uv (auto-installed) → Python 3.13 for chroma-mcp, Claude Code CLI (`CLAUDE_CODE_PATH` auto-detect). Runtime npm deps are tiny (`better-auth`, `@better-auth/api-key`) because everything is bundled. `package.json:63-65`, `CLAUDE.md` Requirements, `src/shared/SettingsDefaultsManager.ts:486-487` [Verified]
- Footprint: about 7.6 MB of committed JS bundles, plus a Chroma Python env with onnxruntime (download size not measured). `plugin/scripts/*.cjs` sizes [Verified/Inference]
- Retention: `tool_uses` are kept forever by default (`CLAUDE_MEM_TOOL_USES_RETENTION_DAYS=0`). Observations have no TTL. Semantic search only ranks the last 90 days unless dates are given. `src/shared/SettingsDefaultsManager.ts:560,655` [Verified]
- Diagnostics: `npx claude-mem doctor`, `/api/admin/doctor`, `npm run worker:logs`, viewer UI, `npm run bug-report`. `src/npx-cli/commands/doctor.ts`, `src/services/server/Server.ts:553`, `package.json` scripts [Verified]
- Multi-account isolation: UID-derived port. Optional externally managed worker (`CLAUDE_MEM_WORKER_AUTOSTART=false`). `src/shared/SettingsDefaultsManager.ts:414,656` [Verified]

## T11 — What is actually new vs marketing

In plain words: the real innovation is using a second, silent LLM session as a live "court reporter" that turns raw tool traffic into typed, searchable facts, plus careful engineering around hooks, budgets and durability. "Compression", "hybrid search" and "~10x savings" are marketing words for simpler mechanics.

- New and worth copying: an **observer-agent pattern** (a resumed, tool-less LLM session fed by `<observed_from_primary_session>` messages, with explicit skip-dedup instructions) that produces a typed schema (`type/title/subtitle/facts/narrative/concepts/files_*`). `src/sdk/prompts.ts:36-112`, `plugin/modes/code.json` [Verified]
- New: **File Read Gate**, which uses memory to *block* expensive whole-file reads and redirect to tree-sitter outline/unfold. It is backed by an eval. `src/cli/handlers/file-context.ts:23-33,163-173`, `evals/read-gate/README.md` [Verified]
- Solid engineering: crash-safe hook spool with exactly-once hand-off. Char-budget fitter tied to the host's 10k stdout limit. Prompt-cache-friendly prompt order. Quota guard with provider fallback. Native prompt-ID idempotency. `src/shared/hook-spool.ts`, `src/services/context/ContextBudget.ts`, `src/sdk/prompts.ts:76-85`, `docs/native-prompt-identity.md` [Verified]
- Marketing vs reality:
  - "Compresses it with AI" = per-event LLM summarisation. [Verified]
  - "Hybrid search" = Chroma-first with FTS fallback, not fusion. `SearchOrchestrator.ts:101-143` [Verified]
  - "Relevant context injected" = newest-N timeline by default, not relevance-ranked (semantic inject and ACT-R are both opt-in). `ObservationCompiler.ts:83-87`, `SettingsDefaultsManager.ts:500,533` [Verified]
  - "Works with Hermes" = not in this codebase. [Verified absence]
  - "~10x token savings" = unbenchmarked in repo. [Verified absence]

## T12 — Weaknesses, limits, risks

In plain words: it is expensive in LLM calls, huge and complicated, tightly tied to Claude Code's hook format, and its default retrieval is "most recent", not "most relevant". The commercial funnel is woven through the installer and settings.

- Cost and latency: one LLM call per tool use, burning subscription quota (hence the quota guard and the many issue references about provider exhaustion). `src/shared/SettingsDefaultsManager.ts:448,477-483`, `src/cli/handlers/observation.ts:63-66` [Verified]
- Durability gap: after spool hand-off, queued work lives in RAM only. A crash relies on transcript replay, which exists only for hosts with transcripts. `src/services/worker/SessionMessageBuffer.ts:30-50` [Verified]
- Retrieval quality: by default the newest 50 observations are injected regardless of the current task. Chroma failure raises `ChromaUnavailableError` instead of falling back to FTS when a query is present. `src/services/context/ObservationCompiler.ts:77-110`, `src/services/worker/search/SearchOrchestrator.ts:116-124` [Verified]
- Memory poisoning / prompt injection: LLM-written text from untrusted tool output is re-injected as context. There is no provenance score or trust gate beyond tag stripping. [Inference]
- Secrets: raw tool I/O is stored forever in `tool_uses` with redaction off by default. `src/services/worker/http/shared.ts:286-305`, `src/shared/SettingsDefaultsManager.ts:524,655` [Verified]
- Complexity: about 149k LOC, a 4,777-line `SessionStore.ts`, a 2,093-line `worker-service.ts`, more than 300 settings keys, and a Python sidecar with pinned transitive deps (pydantic<2.14 workaround). Hard to fork or maintain. `wc -l`, `src/services/sync/ChromaMcpManager.ts:120-136` [Verified]
- Coupling: the context budget, hook JSON and Agent SDK observer all assume Claude Code semantics. Other hosts get partial features (e.g. OpenClaw truncates tool output to 1,000 chars). `src/services/context/ContextBudget.ts:4-9`, `openclaw/src/index.ts:833-836` [Verified]
- Vendor funnel: CMEM Pro is pre-selected, sign-in enables cloud sync, and personal pilot UUIDs sit in default settings. `src/npx-cli/commands/install.ts:1224,1714-1751`, `src/shared/SettingsDefaultsManager.ts:606` [Verified]
- Docs drift (4 vs ~27 tools, Node version) suggests the README lags the code. [Verified]

## T13 — How to incorporate into omni-agent

In plain words: omni-agent needs a "learn" stage that remembers decisions, bugfixes and gotchas across Agile cycles. claude-mem's *observer pattern* and *observation schema* are exactly that. Its codebase is too large and vendor-shaped to embed. Use it as an external service over MCP/HTTP while evaluating, and in parallel port the core idea (about 1–2k LOC) into omni-agent's own memory module.

**Agile lifecycle role**
- **Build/Verify** (capture): every tool call by build or test agents becomes a typed observation (`bugfix`, `decision`, `discovery`, `security_alert`).
- **Release/Operate**: per-turn summaries (`request/investigated/learned/completed/next_steps`) become release notes and handoff material.
- **Learn → Discover/Plan** (inject): at the start of the next sprint or session, inject a budgeted timeline. Planners use `search → timeline → get_observations` progressive disclosure.
- **Build** (cost control): the File Read Gate idea (memory-informed outline instead of full reads).

**Options evaluated**

| Option | Verdict | Why |
|---|---|---|
| Depend on it (npm lib / embed) | ✗ | 149k LOC, Bun+uv+Chroma sidecar, upsell defaults, Claude-Code-shaped |
| Call via MCP/HTTP as external service | ✓ short-term (pilot) | `mem_search/search/timeline/get_observations/save_memory` over stdio MCP; capture via `POST /api/sessions/{init,observations,summarize}` or `claude-mem hook raw …` |
| Port the idea | ✓ long-term | Observer prompt + XML schema + spool + FTS5 + budget fitter are small, well-specified pieces |
| Skip | ✗ | It directly covers omni-agent's "learn" stage |

- License: Apache-2.0 → MIT project. Calling it as a service needs no license action. Porting *ideas* is free. Copying *code or prompts* (e.g. `plugin/modes/code.json` text) requires keeping the Apache license and NOTICE for those files and marking changes. [Inference]
- Pilot config (if run as a service): `CLAUDE_MEM_TELEMETRY=0`, `--provider claude` (or openai-compatible → local), `CLAUDE_MEM_REDACT_ENABLED=true`, `CLAUDE_MEM_REDACT_GENERIC_ENABLED=true`, leave `CLOUD_SYNC_*` empty, `CLAUDE_MEM_SKIP_SUBAGENT_OBSERVATIONS=true`, set `CLAUDE_MEM_TOOL_USES_RETENTION_DAYS`. Keys from `src/shared/SettingsDefaultsManager.ts:430-655`. [Inference]
- Effort:
  - MCP/HTTP pilot: about 1–2 days (a Hermes/omni plugin posting 3 endpoints, plus MCP registration).
  - Port: about 2–3 weeks for capture spool + observer worker (provider-agnostic) + SQLite/FTS5 + optional vector + budgeted injector + 4 MCP tools + privacy tags + redaction-on by default.
  
  [Inference]

**Proposed interface contract (omni-agent `memory` port)**

```ts
// Capture (fire-and-forget, durable spool on the omni side)
interface MemoryEvent {
  sessionId: string; turnId: string;            // native turn id → idempotency (cf. docs/native-prompt-identity.md)
  project: string; cwd: string; agent: string;   // e.g. "planner", "builder", "verifier"
  kind: 'prompt' | 'tool_use' | 'turn_end' | 'session_end';
  tool?: { name: string; input: unknown; output: unknown; ok: boolean };
  text?: string;                                  // prompt or last assistant message
  privacy?: 'normal' | 'private';                 // private ⇒ never sent to observer
}
// Observer output (claude-mem-compatible schema)
interface Observation {
  id: string; project: string; sessionId: string; turnId: string;
  type: 'bugfix'|'feature'|'refactor'|'change'|'discovery'|'decision'|'security_alert'|'security_note'|'sensitive';
  title: string; subtitle?: string; facts: string[]; narrative: string;
  concepts: string[]; filesRead: string[]; filesModified: string[];
  model: string; createdAt: string; sourceToolUseIds: string[];   // provenance for trust gating
}
interface MemoryPort {
  record(e: MemoryEvent): Promise<void>;
  injectContext(q: { project: string; budgetChars: number; query?: string }): Promise<string>;
  search(q: { query?: string; type?: string[]; project?: string; limit?: number; dateStart?: string }): Promise<Array<{id: string; title: string; type: string; createdAt: string}>>;
  timeline(anchorId: string, before?: number, after?: number): Promise<Observation[]>;
  get(ids: string[]): Promise<Observation[]>;
  save(note: { project: string; title: string; text: string; type?: Observation['type'] }): Promise<string>;
}
```
- Adapter for the pilot: `record` → `POST /api/sessions/init|observations|summarize`; `injectContext` → `GET /api/context/inject?projects=…`; `search/timeline/get/save` → MCP `search`/`timeline`/`get_observations`/`save_memory`. Endpoints are listed in T5. [Inference]

## T14 — Open questions

In plain words: these are the points the code did not settle, or that need a live run to confirm.

1. Where is the "Hermes provider" referenced in `docs/native-prompt-identity.md:36` — in the hermes-agent repo, a cmem private repo, or unreleased? It is not present in `~/.hermes/hermes-agent/plugins/memory/`.
2. Does Hermes expose shell hooks per tool call that could run `claude-mem hook raw observation`, or must a Python memory-provider plugin call the HTTP API? (Check the Hermes docs; not verified here.)
3. OpenClaw plugin default `workerPort: 37777` vs worker default `37700+uid%100`. Does `openclaw/install.sh` always write the real port? (`openclaw/openclaw.plugin.json`, `src/shared/SettingsDefaultsManager.ts:414`)
4. The README mentions `CLAUDE_MEM_ONLINE_OPTIN=false` to skip sign-in, but `git grep ONLINE_OPTIN -- src` finds no match. Where is it honoured (`install.sh` or bundled code only)?
5. Real per-session observer token spend on a typical coding session is not measured in the repo. It needs an instrumented run (`observed_billing`, `discovery_tokens` columns exist).
6. Retrieval quality: no recall or precision benchmark. Would injecting newest-N beat a semantic top-k for omni-agent's planner? Needs an eval.
7. Does the cmem gateway retain tool I/O sent for summarisation? Hosted terms were not reviewed (cmem.ai ToS out of scope for source analysis).
8. Exactly what the anonymous PostHog events contain beyond `skill_invoked` and exceptions. A full audit of `captureEvent` call sites was not done.
9. Is the server runtime (Postgres/Valkey) production-ready, or beta? Settings keys still carry `*_BETA_*` fallbacks (`src/shared/SettingsDefaultsManager.ts:640-647`).
