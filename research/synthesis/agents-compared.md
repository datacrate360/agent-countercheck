# Hermes vs OpenClaw vs Odysseus: feature-by-feature

Sources: the seven agent reports in `research/agents/` (Hermes v0.22.2; OpenClaw `15ae101a`; Odysseus `9712a54`). Each
cell is summarised from a cited report section; follow the section ID (D3, D9…) to find the `path:line` evidence.
✅ = has it · ◐ = partial / off by default / opt-in · ❌ = not found in the code.

## 1. In plain words

All three are the same machine underneath: **a model in a loop, with tools, memory files, skills and approvals**. They
differ in what they're for and what they trust.

- **Hermes**: a personal agent that *learns*. It has a small core, a lot of self-improvement machinery, and runs everywhere.
- **OpenClaw**: a personal or team assistant that is *a plug-in for everything*. Huge, with the best memory hygiene and secret handling, but it trusts its operator completely.
- **Odysseus**: a *private workspace* (email, documents, calendar, research, local models) with an agent inside. It has the safest default shell, but a simpler loop and an AGPL license.

## 2. Identity (D1)

| | Hermes | OpenClaw | Odysseus |
|---|---|---|---|
| License | MIT | MIT | **AGPL-3.0-or-later** |
| Steward | Nous Research (company) | OpenClaw Foundation, 501(c)(3), no paid tier | odysseus-dev; outside bulk AI PRs refused |
| Main language | Python (+ TS desktop/TUI) | TypeScript (+ Swift, Kotlin, Rust) | Python (FastAPI) + no-build JS |
| Size | 18.6k files; ~46k Python test functions | 57k files; ~4.9M non-test LOC; ~6.3M test LOC | ~2.6k files; 38k-line `agent_loop.py`; ~10k test functions |
| Release cadence | 41 releases; latest v0.22.2 | ~10 releases/month | 0 releases (rolling `dev`) |

## 3. Capability matrix

| Area | Capability | Hermes | OpenClaw | Odysseus |
|---|---|---|---|---|
| **Loop (D3)** | Default turn cap | none (guardrails instead) | none (loop detectors instead) | **20 rounds** (1–200) |
| | Parallel tool calls | ✅ up to 8 threads | ✅ default, source-order commit | ❌ sequential |
| | Retries + model fallback | ✅ 3 retries → fallback chain | ✅ 8 transient retries/90 s; fallback **blocked after side effects** | ◐ 3 retries; fallback only with owner consent |
| | Mid-turn steering | ◐ interrupt/redirect | ✅ steer into live response when cache-safe | ◐ |
| | "Done needs evidence" | ◐ stop gates (verify-on-stop off by default), `/goal` judge | ◐ goals with auto-continuation | ✅ completion gate + optional fresh-context verifier |
| **Models (D4)** | Providers | 5 wire protocols, ~80 keys | 10 wire APIs, ~63 provider plugins | ~14 types + ChatGPT/Copilot logins |
| | Local models first-class | ◐ any OpenAI-compatible | ◐ providers + text tool-call repair | ✅ built-in serving (llama.cpp, vLLM, SGLang, Ollama, MLX) + hardware fit |
| | Text tool-call repair for weak models | ◐ | ✅ JSON/XML/Harmony/ReAct, allowed names only | ✅ ~10 formats incl. Hermes JSON |
| | Swap the whole harness | ❌ (is the harness) | ✅ Codex app-server, ACP harnesses; Claude Code as CLI backend | ❌ |
| | Prompt-cache discipline | ✅ "sacred", 4 breakpoints | ✅ cache-boundary marker, keyed by compaction | ◐ |
| **Context (D5)** | Compaction | 0.50 of window (0.75 floor <512K) | reserve ≥20k, TTL pruning of tool results, structured checkpoint | at 85%: summarise older half |
| | Project instruction files | one type, first found | AGENTS/SOUL/IDENTITY/USER/BOOTSTRAP/MEMORY (20k/file, 60k total) | up to 8 AGENTS.md, injected as untrusted |
| **Tools (D6)** | Built-ins | 51 core + tool search | ~70 + tool search + "Code Mode" | 88 native |
| | MCP client / server | ✅ / ✅ | ✅ / ✅ (4 server surfaces) | ✅ / 4 built-in servers |
| | ACP (editors) | ✅ agent | ✅ both directions | ❌ |
| **Execution (D7)** | Default | **host** | **host** (sandbox off) | **bubblewrap sandbox, refuse if missing** |
| | Container options | 7 backends (Docker, SSH, Modal, Daytona…) | Docker/Podman/SSH/cloud workers; strict when on | Linux bubblewrap only |
| **Security (D8)** | Default approvals | smart: 12 hard blocks + LLM guardian | exec "full", ask "off" | exact-action approval once untrusted content enters |
| | Prompt-injection stance | 36 patterns on context + memory | "out of scope" unless a code boundary breaks; untrusted fences | untrusted wrapping + approval gate |
| | Secrets kept from the model | ✅ credential vault | ✅ **network-layer secret substitution** | ◐ encrypted in DB (MCP env plaintext) |
| | Multi-user auth | pairing codes for chat users | gateway token, device pairing | **bcrypt + TOTP 2FA, roles** |
| | Audit log | ◐ | ✅ always-on metadata ledger | ◐ |
| **Memory (D9)** | Transcripts | SQLite + FTS5 | SQLite (global + per agent), FTS5 | SQLite + FTS5 |
| | Long-term memory | capped MEMORY.md / USER.md | Markdown + hybrid vector/keyword index; **provenance columns** | JSON facts (+ ChromaDB), auto-extracted |
| | Consolidation | background review; curator | **nightly "dreaming", promote only after ≥3 recalls**, DREAMS.md diary | ❌ |
| **Skills (D10)** | Format | SKILL.md | SKILL.md (AgentSkills) | SKILL.md (modelled on Hermes) + status/confidence |
| | Agent writes skills | ✅ `skill_manage` | ✅ `skill_workshop`, 10 versions, undo | ✅ drafts |
| | Skill must prove value | ❌ | ❌ | ✅ **audit vs a no-skill baseline** |
| | Registry | skills hub (9 sources incl. ClawHub) | ClawHub with sha256 + verdict gate | GitHub / skills.sh import |
| **Orchestration (D11)** | Sub-agents | ✅ up to 10, depth 1 | ✅ depth 5, 8 concurrent; Swarm 32 | ❌ |
| | Kanban / work board | ✅ SQLite dispatcher | ◐ Workboard plugin (off) | ❌ |
| | Schedules | cron, `/loop`, `/heartbeat` | cron/at/every/on-exit; heartbeat 30 min | cron/event/webhook; pauses while user active |
| | Goal loop with judge | ✅ `/goal` | ◐ session goals | ❌ |
| | Workflow / graph engine | ❌ | ◐ Lobster approve/resume pipelines | ◐ Deep Research pipeline |
| **Surfaces (D12)** | Chat platforms | ~30 adapters | 25 channels | ❌ (web app; phone pairing API) |
| | Apps | CLI, TUI, Electron desktop, web dashboard | CLI, TUI, Control UI, macOS/iOS/Android/Watch | web workspace + macOS/Windows launchers |
| | Productivity suite | ❌ | ❌ | ✅ email, documents, notes, calendar, compare, research |
| **Extensibility (D13)** | Plugins | 44 hook points; 614-entry catalog | 171 manifests, 60 register hooks; in-process | MCP, presets, skills, webhooks; no plugin system |
| **Quality (D14)** | QA | 15 AGENTS.md + size caps, `scripts/check` | 601 QA scenarios, 118 CI workflows, maturity scorecard | 11 CI workflows (CodeQL, Trivy, gitleaks) |
| **Ops (D15)** | Telemetry | not audited | opt-in; daily version check | none found |
| **Repo practice (D16)** | AI-dev harness | AGENTS.md hub + CI caps | 28 AGENTS.md, ratchet lints, PR evidence rules | `specs/` as the map, provenance ledger |

## 4. Shared principles (all three arrived at them independently)

1. **Files are memory.** Markdown that a person can read, plus SQLite for search. No hidden vector-only memory.
2. **The agent edits its own procedures.** SKILL.md files the model writes and later reads back.
3. **Guards instead of turn caps.** Two of three ship no turn limit and detect loops instead ("caps caused silent truncation").
4. **Code enforces, prompts advise.** Approvals, allow/deny lists and sandboxes live in code, never only in the prompt.
5. **Prompt-cache economics shape the design.** Stable system prompts, tool lists fixed per session, tool search for rare tools.
6. **MCP is the universal plug.** All three are MCP clients; Hermes and OpenClaw are also servers.
7. **Untrusted content is marked.** Tool output, web pages and retrieved files are wrapped or scanned as data.

## 5. Where each is uniquely best (worth copying)

| From | Idea | Why it matters |
|---|---|---|
| Hermes | Small core ("narrow waist") + `/goal` judge + curator | Keeps the agent cheap and steadily improving |
| Hermes | 7 execution backends behind one `BaseEnvironment` | "Where it runs" is configuration |
| OpenClaw | Dreaming: promote a memory only after ≥3 recalls across ≥3 queries, with a diary | Memory that earns its place, auditable |
| OpenClaw | Provenance columns: untrusted content can never be promoted | Stops memory poisoning |
| OpenClaw | Secrets substituted at the network layer | The model never sees a credential |
| OpenClaw | Harness-as-plugin, fallback blocked after a side effect | Swap engines safely |
| Odysseus | Sandbox-or-refuse shell | Safe by default, honest failure |
| Odysseus | Skill audit vs a no-skill baseline | Self-improvement that's measured, not assumed |
| Odysseus | Teacher takeover when a local model fails | Cheap models first, escalate only when needed |
| Odysseus | Hardware-fit local model serving | Private, low-cost inference |

## 6. Shared gaps (none of the three has them)

- **A team work system**: sprints, estimates, WIP limits per column, review stages, release gates. Hermes's kanban
  and OpenClaw's Workboard are task dispatchers, not Agile.
- **Evidence-gated "done" across a whole work item** (not just one turn): an independent checker whose verdict is
  the only path to "passing".
- **Safe defaults for a team**: Hermes and OpenClaw both run commands on the host by default.
- **Domain depth for data work**: no warehouse, dbt, Airflow or data-contract awareness in any of them.
