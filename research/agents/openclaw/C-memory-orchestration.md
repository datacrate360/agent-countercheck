# OpenClaw — C: memory, skills, orchestration

- **Repo:** `openclaw` shallow clone, commit `15ae101a` (Sun 18 Oct 2026 13:39 UTC), TypeScript monorepo.
- **Scope:** D9 Memory & state · D10 Skills & self-improvement · D11 Multi-agent & orchestration · distinctive ideas.
- **Method:** static reading only (rg / sed / file reads). Nothing was built, installed or run. About 75 files were read in targeted ranges: ~35 source/schema files and ~40 in-repo docs pages (`docs/` is the source of docs.openclaw.ai).
- **Path convention:** paths are relative to the repo root. `docs/...` paths are the in-repo copies of the public docs. Tags: **[Verified]** = read in code or schema · **[Docs]** = only stated in the in-repo docs · **[Inference]** = my conclusion.
- **Root guidance read:** `VISION.md` (163 lines), `AGENTS.md` (134 lines); README skimmed.

---

## D9 — Memory & state

### In plain words
OpenClaw keeps an agent's long-term memory as plain Markdown files in a "workspace" folder: `MEMORY.md` for facts, `USER.md` for user preferences, and daily notes under `memory/`. A SQLite database per agent holds sessions, full transcripts and a search index (keyword + vector) over those files. Every night a "dreaming" job scores the notes and moves only trusted, often-recalled items into `MEMORY.md`. Every indexed piece of memory carries a provenance label (owner / agent / untrusted / system), and anything labelled untrusted can never be promoted or auto-injected.

### Storage layout (sessions, transcripts, state)
- There are two SQLite tiers. The global control plane is `~/.openclaw/state/openclaw.sqlite`. The per-agent data plane is `~/.openclaw/agents/<agentId>/agent/openclaw-agent.sqlite` and holds "Sessions, transcripts, memory indexes, auth state, conversation state". — `docs/reference/database-schemas/layout.md:12-15` [Docs]; the per-agent filename is used in code at `src/state/agent-deletion-journal.ts:72` [Verified].
- House rule: "State and caches: SQLite, not new JSON/JSONL/sidecar stores." All DB access goes through worker threads; the Gateway main thread never touches the DB. — `AGENTS.md:66-68` [Verified].
- Transcripts are stored as rows, not files. Table `transcript_events(session_id, seq, event_json TEXT | event_zstd BLOB, navigation_json, message_role, …)` stores each event as JSON or zstd-compressed JSON. A CHECK constraint caps each event at 4 MiB (`4194304`). — `src/state/openclaw-agent-schema.sql:437-482` [Verified].
- Other session tables: `session_nodes` (session metadata JSON, line 16), `session_windows` (line 140), `session_transcript_archives` / `session_transcript_cold_archives` (cold tier is compressed JSONL files indexed by DB rows, lines 486/511), and `trajectory_runtime_events` (line 535). — `src/state/openclaw-agent-schema.sql` [Verified]; cold archive semantics in `docs/reference/database-schemas/layout.md:193-199` [Docs].
- Transcript full-text search: FTS5 virtual table `session_transcript_fts(text, session_id, message_id, role, timestamp)` with tokenizer `unicode61 remove_diacritics 2`, plus a `session_transcript_fts_rows` ownership map. — `src/state/openclaw-agent-schema.sql:809-822` [Verified].
- PostgreSQL is an experimental alternative backend for agent DB and memory search; SQLite stays the default. — `docs/concepts/memory.md:152-170` [Docs].
- Legacy `sessions.json` + JSONL transcripts are import-only (a Doctor migration). The legacy `~/.openclaw/agents/<id>/sessions/` directory holds only migration inputs and archives. — `docs/concepts/session.md:362-381`, `docs/concepts/agent-workspace.md:117-120` [Docs].
- Session keying:
  - `session.dmScope` defaults to `main`, meaning all DMs share one session. The other options are `per-peer`, `per-channel-peer` (recommended) and `per-account-channel-peer`.
  - Default reset mode is `none`; `daily` (atHour default 4) and `idle` are opt-in.
  - Source: `docs/concepts/session.md:97-104, 206-215` [Docs].
- Incognito sessions keep the entry, transcript and compaction state in process memory only. They expire after 24 h or a Gateway restart, and no memory flush runs. — `docs/concepts/session.md:165` [Docs].
- Session goal receipts (`session_goal_operations`, line 425), progress cards (line 380), heartbeat outcomes (line 390) and dashboard boards (`board_tabs`, `board_widgets`, lines 336/348) all live in the same per-agent DB. — `src/state/openclaw-agent-schema.sql` [Verified].

### Workspace files read at session start
- Canonical bootstrap set, in prompt order: `AGENTS.md`, `SOUL.md`, `IDENTITY.md`, `USER.md`, `BOOTSTRAP.md`, `MEMORY.md`. Files generated on setup: `AGENTS.md`, `SOUL.md`, `IDENTITY.md`, `USER.md`. — `src/agents/workspace-bootstrap-policy.ts:10-36` [Verified].
- **`TOOLS.md` is retired** as a bootstrap file. The constant still exists (`workspace-bootstrap-policy.ts:12`), but a Doctor migration folds it into an `AGENTS.md` "Tools" section. — `src/commands/doctor-tools-md-migration.ts:1,25` [Verified]; `docs/concepts/agent-workspace.md:85-87` [Docs].
- **`HEARTBEAT.md` is retired.** Doctor imports it into DB-stored "monitor scratch" and deletes the file; "The runtime never reads `HEARTBEAT.md`". — `src/commands/doctor-heartbeat-scratch-migration.ts:1,30` [Verified]; `docs/gateway/heartbeat.md:513-515` [Docs].
  - A leftover: the `skipOptionalBootstrapFiles` enum still lists `"HEARTBEAT.md"`. — `src/config/zod-schema.agent-defaults-base.ts:69-71` [Verified].
- `DREAMS.md` is a human-review diary and is never injected. — `docs/concepts/memory-architecture.md:61` [Docs].
- `BOOT.md` is an optional startup checklist, used only when the `boot-md` hook is enabled. — `docs/concepts/agent-workspace.md:88-90` [Docs].
- Budgets:
  - Per file: `DEFAULT_BOOTSTRAP_MAX_CHARS = 20_000`.
  - Total: `DEFAULT_BOOTSTRAP_TOTAL_MAX_CHARS = 60_000`.
  - `USER.md` has a fixed cap, `USER_BOOTSTRAP_MAX_CHARS = 4_000`.
  - Truncation keeps 75% head and 25% tail. `AGENTS.md` gets a "policy digest" truncation that keeps lines with must / never / security wording.
  - Source: `src/agents/embedded-agent-helpers/bootstrap.ts:86-106` [Verified].
- Privacy filter: subagent, cron, group and channel sessions do not get root `MEMORY.md`.
  - Subagents get only `AGENTS.md` (`SUBAGENT_BOOTSTRAP_ALLOWLIST`).
  - Cron gets `AGENTS.md`, `SOUL.md`, `IDENTITY.md` and `USER.md`.
  - Source: `src/agents/workspace.ts:960-1015` [Verified].
- Default workspace is `~/.openclaw/workspace`, or `~/.openclaw-<profile>/workspace` under a named profile; `OPENCLAW_WORKSPACE_DIR` overrides. Agents in a multi-agent roster use `<state-dir>/workspace-<agentId>`. Auto `git init` runs on new workspaces when git is available. — `docs/concepts/agent-workspace.md:21-28, 152` [Docs].

### Long-term memory backends
- Memory is a **single-select plugin slot**. The default slot value is `memory: "memory-core"`; the context-engine slot defaults to `"legacy"`. — `src/plugins/slots.ts:17-20` [Verified]; VISION: "only one memory plugin can be active at a time" `VISION.md:117-118` [Verified].
- **memory-core** (bundled, default):
  - Tools: `intent`, `memory_get`, `memory_search` (`extensions/memory-core/openclaw.plugin.json:32`) [Verified].
  - Index tables:
    - `memory_index_sources`
    - `memory_index_chunks` (embedding stored as BLOB)
    - `memory_index_chunk_recall_metadata` (importance 1–10, triggers, project_key)
    - `memory_index_chunk_provenance` (`origin_class ∈ owner|agent|untrusted|system`, `session_kind ∈ interactive|cron|heartbeat|subagent|unknown`, `observed_at`, `supersedes_key`)
    - `memory_entry_origins`, `memory_session_tombstones`, `memory_embedding_cache`
    - Source: `src/state/openclaw-agent-schema.sql:643-720` [Verified].
  - FTS and vectors: memory FTS5 (`unicode61` or trigram) and a sqlite-vec `vec0` table are created at runtime, not in the static schema. — `src/state/postgres-schema.generated/portability-report.md:7` [Verified]; vec loading and timeouts in `extensions/memory-core/src/memory/manager-sync-base.ts:547` [Verified].
- **Retired backend:** QMD was removed and builtin is now "the only memory engine". — `docs/concepts/memory-builtin.md:319-328` [Docs].
- **memory-lancedb** (bundled extension; docs call it the "official external" plugin):
  - Tools: `memory_store`, `memory_recall`, `memory_forget`.
  - Defaults: `autoRecall` on (`!== false`), `autoCapture` off (`=== true`).
  - Source: `extensions/memory-lancedb/openclaw.plugin.json`, `extensions/memory-lancedb/config.ts:194-195` [Verified].
- **memory-wiki** (bundled, sits beside the active memory plugin; does not replace it): it compiles a "wiki vault" with claims, evidence and contradiction tracking. Tools: `wiki_status`, `wiki_search`, `wiki_get`, `wiki_apply`, `wiki_lint`. — `extensions/memory-wiki/openclaw.plugin.json` [Verified]; `docs/concepts/memory.md:237-250` [Docs].
- **Honcho** is an external plugin (`@honcho-ai/openclaw-honcho`) with user modelling and parent/child agent observation. — `docs/concepts/memory-honcho.md:50` [Docs]. It is not in `extensions/` [Verified by `ls`].
- **Import:** memory can be imported from Codex (`~/.codex/memories`), Claude Code (`~/.claude/projects/*/memory`) and Hermes (`MEMORY.md`, `USER.md`) into `memory/imports/<src>/`. Imported files are indexed but never merged into `MEMORY.md`. — `docs/concepts/memory.md:69-98` [Docs]. Related extensions exist: `extensions/migrate-claude`, `extensions/migrate-hermes` [Verified by `ls`].

### Retrieval
- `memory_search` defaults are hard-coded in the resolved config:
  - `maxResults 6`, `minScore 0.35`
  - hybrid `{ vectorWeight 0.7, textWeight 0.3, candidateMultiplier 4 }`
  - `mmr { enabled: true, lambda: 0.7 }`
  - `temporalDecay { enabled: true, halfLifeDays: 30 }`
  - chunking `{ tokens: 400, overlap: 80 }`
  - embedding cache max `50_000` entries
  - default embedding provider `"openai"`; FTS tokenizer `unicode61`; vector on
  - Source: `src/agents/memory-search.ts:63-71, 113-128, 196-213` [Verified].
- Note: memory-core's own fallback constants say MMR and decay are *off* (`mmr.ts:14-17`, `temporal-decay.ts:7-10`). The host-resolved config turns both on, so the effective default is on. [Verified + Inference]
- `provider: "none"` gives keyword-only (FTS-only) search. — `src/agents/memory-search.ts:79-83` [Verified].
- Supported providers listed in docs: OpenAI, Gemini, Voyage, Mistral, Bedrock, DeepInfra, local GGUF, Ollama, LM Studio, GitHub Copilot, generic OpenAI-compatible. — `docs/concepts/memory.md:210-215` [Docs].
- Transcript search: session transcripts become a search source only when `rememberAcrossConversations` or `experimental.sessionMemory` is set. — `src/agents/memory-search.ts:96-104`, `src/agents/memory-search-source-policy.ts:21-41` [Verified]. A separate `sessions_search` tool queries the FTS table and `sessions_history` returns bounded excerpts. — `src/agents/tool-catalog.ts:171-172` [Verified].
- **Lane 1, trigger injection** (active-memory plugin):
  - Applies to curated `MEMORY.md` / `USER.md` entries that carry `<!-- trigger: … -->` and `<!-- importance: N -->`.
  - `STRONG_TRIGGER_MATCH_SCORE = 0.65`, `TRIGGER_INJECTION_LIMIT = 3`, `MAX_TRIGGER_CONTEXT_CHARS = 1800`, candidate limit 24.
  - Source: `extensions/active-memory/trigger-recall.ts:19-27` [Verified]; format in `docs/concepts/memory-architecture.md:252-269` [Docs].
- **Lane 2, Active Memory recall sub-agent:**
  - Default `mode "escalate"`: it runs only on recall-intent messages when Lane 1 had no strong hit. Other modes are `always` and `off`.
  - `queryMode "recent"`, `timeoutMs 15_000`, tool allowlist `["memory_search","memory_get"]`.
  - Circuit breaker: 3 timeouts, then 60 s cooldown.
  - Source: `extensions/active-memory/types.ts:4-30`, `extensions/active-memory/config.ts:167,204` [Verified]; behaviour in `docs/concepts/active-memory.md:10-21` [Docs].
- **Project-scoped memory:**
  - Entries get `<!-- project: <normalized origin remote> -->` and the `project_key` column.
  - Each session keeps up to 4 recent repo keys; entries from those repos rank higher.
  - Subagents derive their own key set.
  - Source: `docs/concepts/memory-architecture.md:289-334` [Docs]; `project_key` column at `src/state/openclaw-agent-schema.sql:676` [Verified].

### Write path, consolidation ("dreaming"), provenance
- **Pre-compaction memory flush:**
  - On by default; disabled only when `agents.defaults.compaction.memoryFlush.enabled === false`.
  - Runs a silent turn on a private copy of the conversation.
  - Its `write` call may only append new text to today's daily note.
  - Source: `src/auto-reply/reply/memory-flush-plan.ts:37` [Verified]; `docs/concepts/memory.md:259-300` [Docs].
- **Dreaming defaults:**
  - Enabled by default (`enabled ?? true`), and so are the light, deep, REM and deep-recovery phases.
  - Schedule: `DEFAULT_MEMORY_DREAMING_FREQUENCY = "0 3 * * *"`, run as one managed cron job named "Memory Dreaming Promotion".
  - Source: `src/memory-host-sdk/dreaming.ts:21-24, 381-436` [Verified].
- **Deep promotion gates:**
  - `minScore 0.75`, `minRecallCount 3`, `minUniqueQueries 3`, recency half-life 14 d, max 10 promotions per run, max promoted snippet 160 tokens.
  - The rewrite may lose at most 25% of prior entries (`MAX_PRIOR_ENTRY_LOSS_FRACTION 0.25`).
  - Source: `src/memory-host-sdk/dreaming.ts:33-41` [Verified].
- **Promotion score weights:** frequency 0.24, relevance 0.30, diversity 0.15, recency 0.15, consolidation 0.10, conceptual 0.06. — `extensions/memory-core/src/short-term-promotion.ts:31-36` [Verified].
- **Phase sources:**
  - Light: `daily`, `sessions`, `recall`.
  - Deep: `daily`, `memory`, `sessions`, `logs`, `recall`.
  - REM: `memory`, `daily`, `deep`.
  - Source: `src/memory-host-sdk/dreaming.ts:248-260` [Verified].
  - Only Deep writes `MEMORY.md`. — `docs/concepts/dreaming.md:67-71` [Docs].
- **Session-kind gating:** only `interactive` sessions feed dreaming ingestion; checkpoint files and cron-generated or narrative transcripts are excluded. — `extensions/memory-core/src/session-ingestion.ts:133-146` [Verified].
- **Consolidation write safety** (docs claims, not checked in code):
  - A model rewrite of `MEMORY.md` is guarded by a content-hash check before an atomic rename.
  - On conflict it falls back to append-only.
  - The pre-image is stored and a summary goes to `DREAMS.md`.
  - Untrusted and system candidates are excluded before any prompt is built.
  - Source: `docs/concepts/memory-architecture.md:182-220` [Docs].
- **Taint propagation:** a tool that declares network-sourced output taints the rest of the turn as `untrusted` until the next user message. Gap: tools that do not declare it (for example local file reads) do not taint. — `docs/concepts/memory-architecture.md:126-134` [Docs].
- **Forgetting:** `openclaw memory forget` removes entries derived from selected sessions and writes `memory_session_tombstones`; `memory reset` clears derived index data. — `docs/concepts/memory.md:166-168` [Docs]; tombstone table `src/state/openclaw-agent-schema.sql:699-704` [Verified].

### User model
- `USER.md` holds imperative directives, each with a `<!-- observed: YYYY-MM-DD | status: active|superseded -->` metadata line. A changed preference is superseded in place, never appended as a contradiction. — `docs/concepts/user-model.md:366-408` [Docs].
- It has a separate fixed 4,000-character bootstrap budget. — `src/agents/embedded-agent-helpers/bootstrap.ts:88-90` [Verified].
- On a shared, multi-person Gateway, each person gets a personal file at `users/<profileId>/USER.md`. The `personal_instructions` tool writes only the requesting person's file. — `src/gateway/server-methods/users-personal-file.ts:167`, `src/agents/tools/personal-instructions-tool.ts:31` [Verified]; policy `docs/concepts/user-model.md:16-60` [Docs].
- `IDENTITY.md` holds the agent's name / vibe / emoji. `SOUL.md` holds persona and tone. — `docs/concepts/agent-workspace.md:76-84` [Docs].

### Prospective memory (standing intents)
- `standing_intents` table:
  - Columns: keywords, optional trigger embedding, channel/sender scope.
  - Status: `pending|armed|fired|done|cancelled|expired`.
  - Default cooldown 86400 s, plus an FTS5 index on the keywords.
  - Source: `src/state/openclaw-agent-schema.sql:722-775` [Verified].
- Defaults: cooldown 24 h, max fires 3, expiry 90 days. — `extensions/memory-core/src/standing-intents-model.ts:1-3` [Verified].
- Time-based reminders go to cron instead. — `docs/concepts/memory-architecture.md:372-387` [Docs].

### The "decisions" module (not a memory feature)
- `src/decisions/` (21 files, ~5.3k lines with tests) is a typed **decision-model role**: classification, rubric score and Boolean judgments returned with probabilities. It is exposed as the `decision_evaluate` tool. It is unrelated to remembering "decisions". — `src/decisions/types.ts:1-60`, `src/agents/tool-catalog.ts:96-98` [Verified]; providers ONNX / OpenAI / TypeSafe in `docs/concepts/decision-models.md:12-76` [Docs].
- Automatic use (conversational tool filtering) needs the `experimental.decisionAssistance` opt-in. — `src/config/zod-schema.agent-defaults-base.ts:79-80` [Verified].

---

## D10 — Skills & self-improvement

### In plain words
Skills are folders that contain a `SKILL.md` file (AgentSkills format) and are loaded from seven locations in a fixed priority order. ClawHub (clawhub.ai) is the public registry for installing and publishing them. The agent also writes its own "learned skills" through a `skill_workshop` tool, and a background reviewer runs after every 10 model iterations. Every change is versioned and can be undone, and learned skills unused for 30 days are archived automatically.

### Format & locations
- Format: `SKILL.md` with YAML frontmatter `name` + `description`; it "follows the AgentSkills spec".
  - Optional keys: `user-invocable`, `disable-model-invocation`, `command-dispatch: tool`, `command-tool`, `command-arg-mode`, `homepage`.
  - `{baseDir}` is substituted with the skill's folder path.
  - Source: `docs/tools/skills.md:438-490` [Docs].
- Precedence, highest first:
  1. `<workspace>/skills`
  2. `<workspace>/.agents/skills`
  3. `~/.agents/skills`
  4. `<state-dir>/skills` (managed)
  5. `<agentDir>/workshop-skills`
  6. bundled and custodian skills
  7. `skills.load.extraDirs` + plugin skills
  - Source: `docs/tools/skills.md:39-48` [Docs]; source enum incl. `openclaw-workshop`, `openclaw-custodian`, `agents-skills-project` in `src/skills/loading/source.ts:16-26` [Verified].
- Prompt catalog: compact XML `<available_skills>` block.
  - Limits: `DEFAULT_MAX_SKILLS_IN_PROMPT = 150`, `DEFAULT_MAX_SKILLS_PROMPT_CHARS = 18_000`.
  - When the block is too large, descriptions are shortened or dropped first. — `src/skills/loading/skill-prompt-limits.ts:9-10` [Verified]; `docs/tools/skills.md:933-955` [Docs].
  - Tools `skills_search` and `skills_read` load full instructions on demand. — `src/agents/tool-catalog.ts:251-260` [Verified].
- Snapshots: a running session keeps the skill snapshot it started with, so changed skills load in new sessions. This protects prompt-prefix caching. — `docs/tools/skill-workshop.md:48-49` [Docs]; caching rule in `AGENTS.md:21,75` [Verified].
- Bundled: `skills/` has 49 skill directories (e.g. `skill-creator`, `clawhub`, `coding-agent`, `obsidian`, `tmux`). — `ls -d skills/*/` [Verified]. Policy: new skills go to ClawHub first, not core. — `VISION.md:120-124,154` [Verified].
- **Custodian skills:**
  - `custodian-skills/` contains 4 skills: `add-model-provider`, `cloud-image-bake`, `configure-channel`, `diagnose-gateway` [Verified by `ls`].
  - They are visible only to the agent named in `agents.defaults.systemAgent.agentId` (the Custodian / system agent).
  - Each follows a mandatory Gather → Mutate → Repair → Prove → Report structure.
  - Source: `docs/tools/custodian-skills.md:11-38` [Docs].
  - The tier-2/3 names listed in that doc are "roadmap entries, not bundled skills". — `docs/tools/custodian-skills.md:40-58` [Docs].

### ClawHub registry
- `openclaw skills install @owner/<slug>`, also `skills-sh:`, `git:` and local paths. `--global` installs to `~/.openclaw/skills`. Other commands: `skills verify` (trust envelope, "Skill Card") and `clawhub sync --all` for publishing. — `docs/tools/skills.md:334-360` [Docs].
- Code: `verifySkillWithClawHub`, `preflightSkillFromClawHub`, `installSkillFromClawHub`, `updateSkillsFromClawHub`. — `src/skills/lifecycle/clawhub.ts:48,99,186,198` [Verified]. ClawHub also serves per-version security-audit URLs. — `src/skills/lifecycle/clawhub.test-setup.ts:93` [Verified, test fixture].
- A security scanner audits skill bundles and workspaces (`src/skills/security/scanner.ts`, `skill-bundle-scan.ts`, `workspace-audit.ts`) [Verified by `ls`].

### Self-improvement: Skill Workshop (agent-written skills)
- The `skill_workshop` tool has actions `list`, `view`, `create`, `patch`, `write_file`, `remove_file`, `archive` (with `absorbed_into`) and `restore`. It is in the `coding` tool profile. — `src/agents/tool-catalog.ts:250`; actions `docs/tools/skill-workshop.md:149-158` [Docs].
- Storage:
  - Skill files: `<agentDir>/workshop-skills/<name>/SKILL.md` plus `references|templates|scripts|assets/`.
  - Versions: `.archive/<name>/<versionId>/`.
  - Source: `docs/tools/skill-workshop.md:209-226` [Docs].
  - `MAX_VERSIONS_PER_SKILL = 10` — `src/skills/workshop/skill-versions.ts:27` [Verified].
  - The change feed (table `skill_workshop_changes` in the global state DB) keeps 500 entries per agent. — `src/state/openclaw-state-schema.sql:80`, `src/skills/workshop/changes.kernel.ts:34` [Verified].
- Defaults:
  - `skills.workshop.autonomous.mode` defaults to `"auto"` (background review + cleanup on).
  - `maxSkillBytes` defaults to `40_000`, clamped to 1024–200000.
  - Source: `src/skills/workshop/config.ts:17-20` [Verified].
- **Background review loop:**
  - `EXPERIENCE_REVIEW_ITERATION_THRESHOLD = 10` model iterations per session. — `src/skills/workshop/experience-review-scheduler.ts:18,261` [Verified].
  - Timing: the review starts after 30 s idle, runs one at a time, and yields to any foreground run. It reads an excerpt of at most 64 transcript events / 32 KiB.
  - Eligibility: cron, heartbeat, hook, subagent and incognito sessions do not count. — `docs/tools/self-learning.md:28-66` [Docs].
  - Reviewer prompt rules: "Most reviews change nothing"; at most one change per review; must `list` and `view` before patching; merge duplicate skills; reply `NO_REPLY` if nothing qualifies. — `src/skills/workshop/experience-review-prompt.ts:24-45` [Verified].
- **Curation:** `UNUSED_ARCHIVE_MS = 30 days`, reason "unused for 30 days", actor `curator`, checked at most once a day per agent. — `src/skills/workshop/unused-archive.ts:26-29`, `experience-review-default.ts:12` [Verified]. The earlier weekly curator cron and the proposal/draft flow are retired. — `docs/tools/skill-workshop.md:112-114, 270-283` [Docs].
- **Manual triggers:** `/learn [request]`, `/learn undo <id>`, a "Learn from history" UI button, and the CLI `openclaw skills workshop list|changes|show|archive|restore`. — `docs/tools/skill-workshop.md:118-141, 238-240` [Docs].
- **Write validation** (docs): name pattern 1–63 characters; frontmatter `name` must equal the directory; description 1–1024 bytes; support files ≤256 KiB; no symlinks or traversal; critical scanner findings (including literal secrets) refuse the write; "Advisory (not blocking)" lint lines. — `docs/tools/skill-workshop.md:163-182` [Docs]; lint code `src/skills/workshop/skill-lint.ts` [Verified exists].
- Workshop skills bypass agent skill allowlists and are always visible to their owning agent. — `docs/tools/skill-workshop.md:23-25` [Docs].
- **Other learning loops:**
  - Dreaming (D9) is the memory-side reflection loop.
  - Heartbeat `heartbeat_respond` can rewrite its own monitor scratch. — `docs/gateway/heartbeat.md:542-545` [Docs].
  - Repeated requests can be promoted into cron "automations"; the agent proposes it from the conversation, with "no repetition-detection engine". — `docs/automation/cron-jobs/how-it-works.md:57-75` [Docs].

---

## D11 — Multi-agent & orchestration

### In plain words
One Gateway process hosts many agents. Each agent has its own workspace, SQLite DB and persona, and inbound chats are routed to an agent by binding rules. Any agent can spawn background sub-agents (nested up to 5 levels by default) or a large "Swarm" fan-out from Code Mode. Results flow back through an "announce" chain or a `sessions_yield` wait. A built-in scheduler ("automations", alias cron) runs timed, interval, stream or condition jobs. Heartbeat is just a system-owned 30-minute automation. An optional Kanban "Workboard" plugin can dispatch cards to sub-agent workers.

### Multiple agents & routing
- Per agent: its own workspace, `agentDir`, and `<agentDir>/openclaw-agent.sqlite` (sessions and auth). Skills are filtered by per-agent allowlists, and an explicit per-agent list *replaces* the default list rather than merging with it. — `docs/concepts/multi-agent.md:17-33` [Docs].
- Routing bindings are most-specific-wins: exact peer → parent peer → wildcard → guild+roles → guild → team → account → channel → fallback owner.
  - A message that matches no binding, in a multi-agent roster with no explicit owner, is not dispatched to an arbitrary agent; retryable messages are dead-lettered after 24 h.
  - Source: `docs/concepts/multi-agent.md:315-320` [Docs].
- **Team preset:**
  - `openclaw agents team create` creates `coordinator`, `researcher`, `writer` and `reviewer`.
  - The coordinator has `subagents.allowAgents: [researcher, writer, reviewer]` and `delegationMode: "prefer"`, which is "prompt guidance, not a scheduler". Specialists have `allowAgents: []`.
  - Source: `docs/concepts/multi-agent.md:112-165` [Docs].
- **Group-thread broadcast** (several agents in one group chat):
  - `broadcast["<channel>:<peer>"]` lists up to 16 agents, with mention gating on by default.
  - Bounds: `maxRounds` 1–4, `maxTurns` ≤32, strategy `parallel` (default) or `sequential`.
  - Source: `src/auto-reply/group-thread-config.ts:20-48`, `src/config/zod-schema.messages.ts:99` [Verified].
- `src/auto-reply/` (888 files under `reply/`) is the inbound message → agent reply pipeline (dispatch, queue, heartbeat filtering, goal continuation). It is not an "auto-responder" feature. — `src/auto-reply/dispatch.ts:1` [Verified].

### Sub-agents
- Tools:
  - Session tools: `sessions_spawn`, `sessions_yield`, `subagents` (list/cancel), `agents_wait`, `sessions_send`, `sessions_create`, `sessions_list`, `sessions_history`, `sessions_search`.
  - External conversation tools: `conversations_list`, `conversations_send`, `conversations_turn`.
  - Source: `src/agents/tool-catalog.ts:159-191` [Verified].
- Defaults:
  - `DEFAULT_SUBAGENT_MAX_CONCURRENT = 8`, `DEFAULT_SUBAGENT_MAX_CHILDREN_PER_AGENT = 5`, `DEFAULT_SUBAGENT_ARCHIVE_AFTER_MINUTES = 60`.
  - `DEFAULT_SUBAGENT_MAX_SPAWN_DEPTH = 5` ("Allow recursive delegation by default").
  - Source: `src/config/agent-limits.ts:20-24` [Verified]; admission in `src/agents/spawn-plan.ts:303-330` [Verified].
- Session key `agent:<agentId>:subagent:<uuid>`. Results are *announced* back to the requester. By default, sub-agents get no session or message tools. `context: "fork"` gives the child the parent's transcript, and thread-bound children default to fork. — `docs/tools/subagents.md:11-34` [Docs].
- Children get only `AGENTS.md` from the workspace bootstrap set (no `MEMORY.md`, `SOUL.md` or `USER.md`). — `src/agents/workspace.ts:960,1011-1012` [Verified].
- Model inheritance: same-agent children inherit the caller's model unless `agents.defaults.subagents.model` is set. `runTimeoutSeconds` defaults to 0 (no timeout). — `docs/tools/subagents/tool-reference.md:65-68` [Docs].
- **Yield handoff:** the subagent registry owns completion across `sessions_yield`. It freezes the child run IDs, then starts a "successor turn" when the children settle (`subagent-registry-requester-yield.ts`, `subagent-announce.requester-settle-wake.ts`). — `docs/concepts/subagent-yield-handoff.md:12-41` [Docs].
- **Swarm fan-out:**
  - Defaults: `enabled: true`, `maxConcurrent 32`, `maxChildrenPerGroup 50`, `maxTotalPerGroup 200`, `waitTimeoutSecondsMax 600`.
  - Hard caps: 1000 / 10000 / 100000 / 24 h.
  - Source: `src/agents/subagents/swarm/swarm-config.ts:5-51` [Verified].
  - Swarm collectors are read through `agents_wait` or Code Mode `agents.run()`, not `sessions_yield`. — `docs/tools/subagents/tool-reference.md:293-297` [Docs].
- **ACP harnesses:** external coding agents (Codex, Claude, etc.) can be spawned through ACP, with a parent-stream store. — `src/agents/subagents/spawn/acp-spawn*.ts` [Verified by `ls`]; `docs/tools/acp-agents.md` [Docs].

### Workboard (Kanban)
- Bundled plugin `workboard`, `enabledByDefault: false`. — `extensions/workboard/openclaw.plugin.json:33` [Verified].
- 17 tools listed, including `workboard_create`, `workboard_claim`, `workboard_heartbeat`, `workboard_complete`, `workboard_block` and `workboard_sessions_board_read`. — `extensions/workboard/openclaw.plugin.json:83-100` [Verified].
- Card model:
  - Statuses: `triage, backlog, todo, scheduled, ready, running, review, blocked, done`.
  - Priorities: `low` to `urgent`.
  - Execution engines: `codex`, `claude`; modes `autonomous` / `manual`.
  - Source: `packages/workboard-contract/src/index.ts:13-35` [Verified].
- Limits:
  - `MAX_CARDS 2000`, `DEFAULT_CLAIM_TTL_MS 30 min`, `RUNNING_HEARTBEAT_STALE_MS 20 min`, `BLOCKED_TOO_LONG_MS 24 h`.
  - Source: `extensions/workboard/src/store-constants.ts:14-34` [Verified].
- Dispatch:
  - Each pass promotes dependency-ready cards, expires claims, then claims and launches workers through the normal subagent runtime.
  - `DEFAULT_DISPATCH_MAX_STARTS = 3` per pass, one card per owner at a time.
  - Stable session key `agent:<id>:subagent:workboard-<boardId>-<cardId>`.
  - Source: `extensions/workboard/src/dispatcher.ts:34`, `src/session-link.ts:17` [Verified]; procedure `docs/plugins/workboard.md:489-560` [Docs].
  - Board metadata `autoDecompose` / `orchestratorProfile` is "recorded intent" only; decomposition still goes through tools. — `docs/plugins/workboard.md:578-581` [Docs].
- **Sessions board:** sessions grouped into rule-defined columns by run state, observer health and PR state. Observer health values: `on-track, grinding, stuck, waiting-on-user, wrapping-up, done, failed`. — `packages/workboard-contract/src/sessions-board.ts:6-14` [Verified].
- `src/boards/` is different: these are per-session *dashboard* boards (tabs and HTML / MCP-app widgets). — `src/boards/board-store.ts:1-40`, schema `board_tabs` / `board_widgets` [Verified].

### Scheduler (automations / cron), heartbeat, goals
- **Automations:**
  - Run inside the Gateway; jobs and history persist in the shared state DB (`cron_jobs`). — `docs/automation/cron-jobs/how-it-works.md:16-17` [Docs]; `src/state/openclaw-state-schema.sql:1439` [Verified].
  - Schedule kinds: `at`, `every`, `cron` (croner, optional tz), `on-exit`, `stream` (watch a command's output lines). Condition scripts can gate firing. — `docs/automation/cron-jobs/schedules.md:5-20, 99-103` [Docs]; `on-exit` in `src/cron/persisted-shape.ts:93,131` [Verified].
  - Payload kinds: `systemEvent`, `agentTurn`, `command`, `script`. Session target: `main`, `isolated` or `current`. — `src/cron/heartbeat-task.ts:30-36`, `src/cron/delivery-defaults.ts:21-22` [Verified].
  - Limits: `DEFAULT_CRON_MAX_CONCURRENT_RUNS = 8` (`src/config/cron-limits.ts:3`) [Verified]. Watchdogs: isolated agent turns 60 min, commands 10 min, scripts 5 min. — `docs/automation/cron-jobs/how-it-works.md:20` [Docs].
  - "Paced" jobs can call `automations(action:"next_check", in:"30m")` to choose their next run time. — `docs/automation/cron-jobs/schedules.md:77-79` [Docs].
  - Crash policy: never replay uncertain side effects; interrupted runs are marked "Unknown". — `docs/automation/cron-jobs/how-it-works.md:39-47` [Docs].
- **Heartbeat:**
  - A system-owned automation job per agent; `DEFAULT_HEARTBEAT_EVERY = "30m"` (`src/auto-reply/heartbeat.ts:26`) [Verified]. Docs: it becomes 1 h with Anthropic OAuth when unset, and the default target is `owner`. — `docs/gateway/heartbeat.md:96-99` [Docs].
  - Instructions come from DB "monitor scratch" (≤256 KiB, compare-and-swap writes). The heartbeat is skipped if the scratch is effectively empty, and it replies `NO_REPLY` when idle. — `docs/gateway/heartbeat.md:497-511`, `src/auto-reply/heartbeat.ts:29-40` [Verified].
- **Goals:**
  - One durable objective per session: `/goal start|edit|pause|resume|complete|clear`, tools `get_goal`, `create_goal`, `update_goal`, and a token budget.
  - The runtime keeps pursuing an active goal after a normal final reply by queuing a follow-up turn. — `docs/tools/goal.md:14-21` [Docs]; `GoalContinuation` in `src/auto-reply/reply/goal-continuation.ts:14-17`, `src/auto-reply/reply/queue/enqueue.ts:103` [Verified].
- **Standing orders** are prose programs in `AGENTS.md` (scope, triggers, approval gates, escalation), with no separate engine. — `docs/automation/standing-orders.md:18-36` [Docs].
- **Workflows:**
  - **Lobster**, a small typed pipeline DSL (`.lobster`) with built-in approve/resume tokens. — `docs/tools/lobster.md:13-25, 237` [Docs].
  - **llm-task**, JSON-only LLM steps. — `docs/tools/llm-task.md` [Docs].
  - The previous Tasks/TaskFlow runtime "is removed"; its tables remain unused. — `docs/reference/database-schemas/layout.md:45` [Docs].
- **Inter-agent messaging:**
  - `sessions_send` to another session or agent.
  - **A2A** plugin: Agent2Agent 1.0 JSON-RPC, a public Agent Card, bearer tokens per peer; text tasks only, no streaming or cancel. — `extensions/a2a/README.md:1-17` [Verified].
  - `openclaw agent --agent <id> --message …` from the CLI. — `docs/tools/agent-send.md` [Docs].
- **Claws** (portable agent packages):
  - A `CLAW.md` + `package.json` bundle of instructions, workspace files, skills, plugins, MCP servers and automations. "Add" creates an ordinary agent (one-way, no update or remove).
  - Gated by a Labs toggle (`gateway.controlUi.experimental.claws`).
  - Source: `docs/cli/claws.md:11-24` [Docs]; implementation `src/claws/` (109 files) incl. `clawhub-source.ts`, `cron.ts`, `mcp.ts` [Verified by `ls`].

---

## Distinctive ideas (worth stealing)
1. **Provenance in schema columns, not prose.** `origin_class` and `session_kind` are SQL CHECK columns. Untrusted and system content is structurally barred from promotion and auto-injection. — `src/state/openclaw-agent-schema.sql:680-697` [Verified].
2. **Write-path curation off the reply path.** Nightly "dreaming" applies fixed numeric gates (0.75 score, 3 recalls, 3 distinct queries) before a bounded model rewrite, with a 25% loss guard and a human diary (`DREAMS.md`). — `src/memory-host-sdk/dreaming.ts:33-41` [Verified].
3. **Two-lane recall.**
   - Lane 1: a zero-model-call trigger/importance prefilter (≥0.65, ≤3 entries).
   - Lane 2: a recall sub-agent only on recall-intent turns with no strong Lane 1 hit, protected by a circuit breaker.
   - Source: `extensions/active-memory/trigger-recall.ts:19-27`, `types.ts:24-29` [Verified].
4. **Separate user-model file** with dated directives superseded in place and a fixed 4k budget, plus per-person `USER.md` files on shared gateways. — `src/agents/embedded-agent-helpers/bootstrap.ts:88-90` [Verified].
5. **Compiled prospective memory.** "Remind me when X comes up" becomes a SQLite row with trigger keywords and embedding, cooldown, fire budget and expiry, not a prose note. — `src/state/openclaw-agent-schema.sql:722-739` [Verified].
6. **Self-written skills with real version control.**
   - 10 versions per skill, a change feed, one-click undo of a whole review.
   - The reviewer is told "most reviews change nothing".
   - Cleanup archives unused skills after 30 days, so the skill set does not keep growing.
   - Source: `src/skills/workshop/*` [Verified].
7. **Privacy-aware bootstrap per session kind.** Subagents get `AGENTS.md` only; cron, group and channel sessions get no `MEMORY.md`. — `src/agents/workspace.ts:960-1015` [Verified].
8. **Project-scoped memory keyed by git origin.** This stops lessons from one repo steering work in another. — `docs/concepts/memory-architecture.md:289-334` [Docs].
9. **Prompt-prefix stability treated as a hard contract** (a test guards it), with skill snapshots fixed per session. — `AGENTS.md:21,75` [Verified].
10. **Heartbeat implemented as an ordinary scheduled job**, with DB scratch the agent can rewrite and an empty-scratch skip that saves calls. — `docs/gateway/heartbeat.md:20, 497-511` [Docs].

## Weaknesses / risks
- **Huge complexity surface.** Memory alone spans memory-core, memory-wiki, active-memory, memory-lancedb, an external Honcho plugin, a host SDK and Doctor migrations. VISION admits it "plan[s] to converge on one recommended default path". — `VISION.md:117-118` [Verified]. Hard to reason about end to end. [Inference]
- **Fragile retention around migrations.** Docs say pre-July-2026 dreaming JSON is no longer imported, and very old workspace state requires stepping through release `2026.9.7`. — `docs/concepts/memory.md:363-370`, `docs/concepts/agent-workspace.md:134-137` [Docs].
- **Inconsistent defaults across layers.** memory-core constants say MMR and decay are off, while the host-resolved config forces them on. — `mmr.ts:14-17` vs `src/agents/memory-search.ts:126-127` [Verified]. Similar leftovers: `HEARTBEAT.md` in a config enum (`zod-schema.agent-defaults-base.ts:70`) and the unused `DEFAULT_TOOLS_FILENAME` [Verified].
- **Default embedding provider is OpenAI**, a cloud call, so semantic memory leaves the machine unless reconfigured. — `src/agents/memory-search.ts:71` [Verified].
- **Taint coverage depends on tools declaring network output.** Content derived from local file reads keeps trusted provenance. — `docs/concepts/memory-architecture.md:130-134` [Docs].
- **Dreaming and the skill review both spend model calls in the background by default** (dreaming at 03:00; skill review every 10 iterations). The cost is hidden from casual users. [Inference from defaults verified above]
- **Self-learning counters are in memory only**, so a restart resets them. — `docs/tools/self-learning.md:58-59` [Docs].
- **Shared Gateway trust model.** The default `dmScope: "main"` shares one session across all DMs. Docs warn that Alice's messages are visible to Bob unless isolation is configured. — `docs/concepts/session.md:80-104` [Docs].
- **Orchestration is wide but shallow on planning.** Team `delegationMode: "prefer"` is only prompt guidance; Workboard `autoDecompose` is recorded intent only; Tasks/TaskFlow was removed. There is no graph/workflow engine beyond Lobster pipelines. — sources above [Docs].
- **Default sub-agent depth 5 with 5 children each** allows large recursive trees. The cap is concurrency (8), not total cost. — `src/config/agent-limits.ts:20-24` [Verified] + [Inference].

## Open questions
- Exact recency/importance ranking formula at query time. Docs say "exponential recency decay × importance multiplier"; I verified the decay settings but did not trace where the importance multiplier is applied in `manager-search-orchestration.ts`.
- Whether dreaming consolidation also rewrites `USER.md` in code. The docs diagram says `MEMORY.md / USER.md`, but the verified constants mention only `MEMORY.md` promotion.
- How `sessions_send` cross-agent permissions are enforced (`tools.agentToAgent`?). Not traced.
- Whether ClawHub verification results ("trust envelope", security audit) gate installs by default or are only advisory.
- How Honcho / LanceDB interact with dreaming when they hold the memory slot. Docs imply dreaming is memory-core-only.
- Runtime behaviour, latencies and actual default activation of `active-memory` (plugin `onStartup: true`; docs say it "defaults on for personal installs" — `docs/concepts/active-memory/enabling.md:30`). Not exercised because nothing was run.
