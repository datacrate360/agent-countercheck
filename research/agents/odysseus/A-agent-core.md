# Odysseus — A: agent core

- **Repo:** `odysseus-dev/odysseus`, shallow clone at `cache/scratch/agents/odysseus`, commit **9712a54** (Fri 16 Oct 2026 22:05 +0300, "Merge pull request #6434 …"), default branch `dev`, AGPL-3.0-or-later (`README.md:107`).
- **Size:** about 478k lines of Python across the repo (`find -name '*.py' | xargs wc -l`). There are 1,319 `tests/test_*.py` files. `src/agent_loop.py` alone is **38,039 lines**, and `stream_agent_loop` runs from line 21029 to the end of the file, so it is one function of about 17k lines.
- **Method:** static source reading only. Nothing was installed, built or run. Line numbers refer to commit 9712a54.
- **Labels:** [Verified] = I read the code at the cited line. [Docs] = a repo spec or doc says it and I did not trace it in code. [Inference] = my own reading.
- **Files read (fully or in the relevant parts):**
  - Repo docs: `README.md`, `ROADMAP.md`.
  - Specs and docs: `docs/AGENT_TURN_CONTRACT.md`, `docs/skills-lifecycle.md`, `docs/BACKGROUND_TOOL_JOBS.md`, `specs/agent-tools.md`, `specs/context-building.md`, `specs/memory-skills.md`, `specs/research.md`, `specs/llm-models.md`, `specs/cookbook-hwfit.md`, `specs/shell-mcp.md`, `specs/compare.md`, `website/agent-migration.md`.
  - Agent loop and context: `src/agent_loop.py` (targeted sections), `src/context_compactor.py`, `src/context_budget.py`, `src/model_context.py`, `src/generation_budget.py` (defs).
  - Tools: `src/tool_parsing.py` (defs), `src/tool_schemas.py` (schema list), `src/tool_index.py`, `src/agent_tools/__init__.py`, `src/agent_tools/subprocess_tools.py` (constants), `src/agent_tools/model_interaction_tools.py`, `src/agent_tools/session_tools.py`.
  - Models and providers: `src/llm_core.py` (defs, retry, provider detection, local gate), `src/model_pricing.py`, `src/model_profiles.py`, `src/chatgpt_subscription.py`, `src/copilot.py`.
  - Orchestration: `src/ai_interaction.py` (pipeline), `src/teacher_escalation.py` (header), `src/agent_runs.py`, `src/deep_research.py` (`research()`), `src/task_scheduler.py` (defs and executors), `src/clean_agent_preview.py` (constants).
  - MCP: `src/mcp_manager.py`, `src/builtin_mcp.py`.
  - Memory and state: `src/memory.py`, `src/memory_vector.py`, `src/embeddings.py`, `src/embedding_lanes.py`, `core/database.py` (tables), `core/session_manager.py` (defs), `src/session_search.py`.
  - Skills and migration: `services/memory/skill_format.py`, `skill_extractor.py`, `memory_extractor.py`, `skill_importer.py`, `skill_lifecycle.py`; `routes/chat_helpers.py` (post-turn extraction); `routes/chat_routes.py` (agent invoke); `routes/codex_routes.py`; `integrations/{claude,codex}/README.md`; `scripts/agent_migration_manifest.py`; `resources/skills/**`.
  - Cookbook and hardware fit: `services/hwfit/fit.py` (defs), `routes/cookbook_routes.py` (greps).

---

## D3 Agent loop

**In plain words.** Odysseus is a web app (FastAPI + SSE), not a CLI agent. Each agent "turn" runs a streaming Python generator. It calls the model, reads tool calls (native function calls or tool syntax written into the text), runs the tools **one at a time**, appends the results, and repeats. The default cap is 20 rounds. Most of the code is guard rails: an immutable per-turn "turn contract", loop-breakers, nudges, artifact/evidence completion gates, an optional verifier, and a "teacher" model takeover for weak local models.

**Entry point and shape**
- The main generator is `stream_agent_loop(endpoint_url, model, messages, …, max_rounds=MAX_AGENT_ROUNDS, max_tool_calls=0, …)`. It takes more than 50 keyword args, including `plan_mode`, `workspace`, `turn_contract`, `exact_approval`, `request_authority` and `context_resolution`. `src/agent_loop.py:21029-21075` [Verified]
- It is wrapped by two decorators, `@with_teacher_takeover` and `@with_completion_gate`. `src/agent_loop.py:21027-21028`; the gate is defined at `src/agent_runtime/completion.py:206` and the takeover at `src/teacher_escalation.py:51` [Verified]
- The completion gate holds answer deltas until the generator unwinds. It checks completion against a server journal/evidence ledger, and a cancelled turn can never emit a successful completion. `specs/agent-tools.md:148` [Docs]
- The module docstring says "The LLM decides when to use tools by writing fenced code blocks". This is the legacy prompted-tool design, and native function calling was added on top. `src/agent_loop.py:1-7` [Verified]
- Turns whose contract selects the compact preview go to a separate, newer loop, `src.clean_agent_preview.stream_preview` (8,364 lines). `src/agent_loop.py:21102-21125`; `src/clean_agent_preview.py:5235` [Verified]
- If a turn contract says a required capability is unavailable, the loop stops before any model call with the message "I can't perform this request with the currently permitted tools … I haven't substituted another tool." `src/agent_loop.py:21127-21138` [Verified]

**Turn structure (legacy loop)**
- Round loop: `for round_num in (count(1) if _round_limit is None else range(1, _round_limit+1))`. `max_rounds=None` is an "adaptive mode used by TUI workspace coding" with no cap. Finite values are clamped to at least 1. `src/agent_loop.py:25734-25747` [Verified]
- Each round:
  1. Build the prompt and schemas.
  2. Run `maybe_compact`. `src/agent_loop.py:25867` [Verified]
  3. Stream the model through `stream_llm_with_fallback`. `src/agent_loop.py:26914` [Verified]
  4. Collect `round_response`, `round_reasoning` and `native_tool_calls`. `src/agent_loop.py:25767-25769` [Verified]
  5. Parse the tool blocks.
  6. Execute them **sequentially**, `for i, block in enumerate(tool_blocks)`. `src/agent_loop.py:32172` [Verified]
  7. Append the results through `_append_tool_results()`. `specs/context-building.md:77` [Docs]
  8. Emit `agent_step` and continue.
- **No parallel tool execution in the loop.** Each tool runs as one `asyncio.create_task(_run_tool())`, which is awaited while its progress queue is drained. The only other `create_task` is the approved-tool replay. `src/agent_loop.py:33500`, `25246` [Verified]
- The model can still fan out inside a single tool: `web_fetch` takes up to 12 URLs "fetched concurrently" (`src/tool_schemas.py:362`), and research extraction runs with concurrency 3 (`src/deep_research.py:358`) [Verified]
- Tool execution goes through `execute_action(execute_tool_block, _runtime_action, block, …)`. The runtime "action" is proposed first with `propose_action(block, tool_call_id, native_call)`, which feeds the action journal. `src/agent_loop.py:33478-33496`, `32181` [Verified]

**Stop conditions and budgets (defaults quoted)**
- `MAX_AGENT_ROUNDS = 20` in `src/agent_tools/__init__.py:86`, with the comment "a stale/repeating tool loop must not consume a whole session" [Verified]
- The chat route reads the setting `agent_max_rounds` (default `20`, comment "clamped 1..200") and `agent_max_tool_calls` (default `0` = unlimited). `src/settings.py:115-116`; `routes/chat_routes.py:4557-4622` [Verified]
- When the tool budget is hit, the loop emits `{"type":"budget_exceeded","limit":max_tool_calls,"used":…}`. `src/agent_loop.py:32200` [Verified]
- When the round cap is hit, it emits `{"type":"rounds_exhausted","rounds":_round_limit}`. `src/agent_loop.py:37201` [Verified]
- **Loop-breaker.** The signature of a round is the sorted `tool:content[:120]` of all its calls. The breaker trips when any of these holds:
  - `_stuck_rounds >= 4` (repeated signature);
  - a `_runaway` call (the same args over and over);
  - `_blocked_status_rounds >= 2`;
  - `_read_only_inspection_rounds >= 6`.
  
  It then emits `loop_breaker_triggered` and forces one tool-less "best final answer" round. A second trip ends the turn. `src/agent_loop.py:31492-31525`, `31564-31590` [Verified]
- **Nudge caps:** `_MAX_INTENT_NUDGES = 2`, `_MAX_CLARIFICATION_NUDGES = 1`, `_MAX_EMPTY_ACTION_NUDGES = 1`, `_MAX_DECLARED_CONTRACT_NUDGES = 1`. When exhausted, the loop emits `intent_nudge_exhausted`. `src/agent_loop.py:24860-24868`, `31122` [Verified]
- **Failed-call retry suppression:** `_failed_call_history[_call_signature]` blocks an identical failed retry under a terminal contract and emits `tool_retry_blocked`. `src/agent_loop.py:32182-32185` [Verified]
- Spec summary: "Repeated successful exact calls are suppressed while changed arguments or pagination can proceed". `specs/agent-tools.md:180` [Docs]
- **Verifier subagent (off by default).** `_run_verifier_subagent` makes one fresh-context call: user request plus action snapshot, `temperature=0.0`, `max_tokens=600`, `timeout=60`. The model must output `VERIFICATION: SUCCESS|FAIL: …`. `src/agent_loop.py:16793-16830` [Verified]
  - It runs only if the turn used effectful tools, the model claimed done, `_verifier_rounds < _VERIFIER_MAX_ROUNDS (=2)` and setting `agent_verifier_subagent` is true. The default is `False`, with the comment that "on weak local models the verifier … false-rejects". `src/agent_loop.py:16342`, `30512-30525` [Verified]
- **Completion / evidence gates.** These SSE events show the loop refusing to finish while required artifacts are missing or unverified, and steering into "mutation recovery": `EvidenceLedger.from_tool_events(…).evaluate().missing_artifacts`, `completion_blocked`, `artifact_repair_required`, `artifact_finish_nudge`, `source_evidence_required`. `src/agent_loop.py:25776-25779`, `30500-30511`, `31530-31563` [Verified]
- **Compact preview budgets** (a separate loop):
  - `NATIVE_TOOL_CALL_LIMIT = 32`, `NATIVE_ROUND_LIMIT = 64`, `INTERACTIVE_ROUND_LIMIT = 8`. `src/clean_agent_preview.py:154-165` [Verified]
  - Interactive rounds clamp to 1–200 with an eight-round fallback. At most 2 context-size retries and 1 pre-response transport retry per turn. `specs/agent-tools.md:178-180` [Docs]
- "Budgets are checked between rounds and do not interrupt an already-running call." `specs/agent-tools.md:180` [Docs]

**Streaming**
- The loop emits SSE `data:` JSON events. The docstring lists `delta`, `tool_start`, `tool_output`, `agent_step`, `metrics` and `[DONE]`. `src/agent_loop.py:21076-21085` [Verified]
- A grep finds about 40 distinct event types. Examples: `final_response`, `tool_progress`, `compacted`, `doc_stream_open/delta`, `doc_update`, `doc_suggestions`, `ask_user`, `plan_update`, `ui_control`, `generated_image`, `web_sources`, `turn_contract`, `tool_routing_audit`, `model_request_snapshot`, `completion_decision`, `agent_terminal`. (grep on `src/agent_loop.py`) [Verified]
- Long-running bash/python tools push `{elapsed_s, tail}` progress through a callback, and the loop forwards each one as `tool_progress`. `src/agent_loop.py:33443-33512` [Verified]
- Runs are detached: `src/agent_runs.py` keeps an in-memory `_RUNS` registry. Subscribers replay by sequence number with a 10 s heartbeat, and finished runs are evicted after `_EVICT_GRACE_S = 180`. `src/agent_runs.py:48`, `212-260` [Verified]
- Detached runs "do not survive server restart"; background job state is disk-backed. `specs/agent-tools.md:94` [Docs]

**Interrupts**
- **Stop:** `agent_runs.stop(session_id, expected_run_id)` cancels the asyncio task. It fails closed if the run id doesn't match, for example a stale tab. `src/agent_runs.py:270-283` [Verified]
- **Soft finish:** `request_finish()` sets `finish_requested` and `finish_event` ("finish after its completed editor work"), and `should_finish()` can be polled. `src/agent_runs.py:286-305` [Verified]
- **Tool cancel on disconnect:** if the generator closes while awaiting a tool, `finally:` cancels and awaits `_tool_task` so that subprocesses aren't orphaned. `src/agent_loop.py:33513-33527` [Verified]
- **Background preemption:** scheduled tasks are cancelled when foreground activity appears (`has_foreground_activity()` triggers `current_task.cancel()`). `src/task_scheduler.py:1000-1007` [Verified]
- Local-model traffic is also serialized through `_local_model_slot`, where "foreground chat taking priority". `src/llm_core.py:111-135` [Verified]

**Retries and fallbacks**
- `LLMConfig`: `MAX_RETRIES = 3`, `RETRY_DELAY = 0.5`, `DEFAULT_TIMEOUT = 30`, `STREAM_TIMEOUT = 300`, `DEFAULT_MAX_TOKENS = 32768`, `DEFAULT_TEMPERATURE = 1.0`, `CONNECT_TIMEOUT = env LLM_CONNECT_TIMEOUT or 10`. `src/llm_core.py:180-195` [Verified]
- Async calls retry HTTP `429/502/503/504` up to `max_retries`, sleeping `RETRY_DELAY` between tries. `src/llm_core.py:2780-2799` [Verified]
- A dead-host cooldown applies (`_mark_host_dead`). `src/llm_core.py:570-610` [Verified]
- If a local stream emits no first event, it is retried once without cache affinity. `src/llm_core.py:3866-3876` [Verified]
- **Context-overflow recovery:** `_stream_candidate_with_context_recovery` uses `generation_budget.parse_context_error` / `plan_context_recovery`. `src/llm_core.py:4058`; `src/generation_budget.py:155-188` [Verified]
- **Foreground model fallback** is strict by default and needs owner consent: `foreground_fallback_enabled=true` plus an ordered list of up to 10 models.
  - Eligible statuses are 408/425/429/500/502/503/504/507/508/529, and only before substantive output. Once a candidate produces text or a tool call it is committed. `specs/llm-models.md:87-89` [Docs]
  - This plugs in through `stream_llm_with_fallback(candidates, …)`. `src/llm_core.py:4436` [Verified]
- If a provider rejects native tool transport, native tools are disabled temporarily for that route (`_NATIVE_TOOL_REJECTIONS[...] = time.monotonic()`) and the loop falls back to text tools. `src/agent_loop.py:8323-8324`; `src/llm_core.py:4039` [Verified]

**Plan mode / guide-only**
- Plan mode adds a denylist of mutating tools and hides write/unknown MCP tools. `plan_mode_disabled_tools()` runs at `src/agent_loop.py:21095-21096` [Verified]
- The current browser UI forces `plan_mode` off. `specs/agent-tools.md:49` [Docs]
- `ToolPolicy` handles guide-only / no-tools turns by hiding schemas, skipping retrieval and blocking at execution. `specs/agent-tools.md:47` [Docs]

---

## D4 Model / provider layer

**In plain words.** All model calls go through one 4,700-line adapter (`src/llm_core.py`). It speaks the OpenAI Chat Completions shape to almost everything and adds special handling for Anthropic, Ollama, ChatGPT-subscription (Codex Responses API), Copilot, Kimi/Moonshot, Mistral and others. Local models are first-class. A "Cookbook" downloads and serves them with llama.cpp, vLLM, SGLang, Ollama, MLX or Diffusers, and a "HW Fit" module (adapted from llmfit) recommends models that fit your hardware. Models without native tool calling get a very wide text tool-call parser.

**Provider detection and adapters**
- `_detect_provider(url)` matches on hostname. Unknown hosts default to the `"openai"` adapter (OpenAI-compatible). `src/llm_core.py:1069-1108` [Verified]
- Recognized providers: `ollama`, `anthropic`, `opencode-go`, `opencode-zen`, `openrouter`, `groq`, `nvidia`, `kimi-code`, `moonshot`, `chatgpt-subscription`, `copilot`, `featherless`, `cerebras`, `mistral`. Same lines [Verified]
- Anthropic gets its own payload and headers through `_build_anthropic_payload`, `_build_anthropic_headers` and `_convert_openai_content_to_anthropic`. `src/llm_core.py:1779-1935` [Verified]
- Ollama native `/api` normalization: `_build_ollama_payload` and `_ollama_normalize_messages`. `src/llm_core.py:738-873` [Verified]
- GPT-OSS harmony: tool-name aliasing (`_alias_harmony_tools` / `_unalias_harmony_tool_name`) and harmony suffix hold-back. `src/llm_core.py:348`, `1406-1431` [Verified]
- DeepSeek V4 reasoning defaults: `src/llm_core.py:1443` [Verified]
- Temperature omission rules (Opus 4.7+, Moonshot/Kimi, reasoning models): `_omit_temperature`, `_anthropic_rejects_temperature`, `_restricts_temperature`. `src/llm_core.py:1657-1740` [Verified]
- Kimi Code User-Agent negotiation: on a 403 it retries a whitelisted set of UAs and caches the accepted value. `src/llm_core.py:914-1066` [Verified]
- Provider docs exist for 36 providers or engines under `specs/model-providers/`, for example anthropic, bedrock, azure-openai, google, groq, xai, deepseek, mistral, openrouter, together, fireworks, perplexity, cohere, nvidia-nim, cloudflare-workers-ai, github-models, github-copilot, chatgpt-subscription, opencode, venice, zai, minimax, moonshot-kimi, siliconflow, hugging-face, llama-cpp, lm-studio, ollama, vllm, sglang and local-compatible-engines [Verified dir listing; adapter depth per provider not verified]
- Provider-capability readers (no network I/O) exist for generic-openai, openai, openrouter, google, ollama, lmstudio and llamacpp. `src/model_capability_readers/` [Verified listing]
- The spec states these canonical records are **"not wired into runtime discovery"**. `specs/llm-models.md:47` [Docs]

**Subscription logins**
- **ChatGPT subscription (Codex backend):**
  - Base URL is `https://chatgpt.com/backend-api/codex` (env `CHATGPT_SUBSCRIPTION_BASE_URL`). `src/chatgpt_subscription.py:23-26` [Verified]
  - OAuth device flow: client id `app_EMoamEEZ73f0CkXaXp7hrann`, token URL `https://auth.openai.com/oauth/token`, refresh skew 120 s, and a usage endpoint `chatgpt.com/backend-api/wham/usage`. `src/chatgpt_subscription.py:35-42`, `323-409` [Verified]
  - Payloads are converted to a Responses-API shape (`_build_chatgpt_responses_payload`). `src/llm_core.py:1531` [Verified]
  - Supports multiple accounts and Codex reasoning levels. `src/chatgpt_subscription.py:150-243` [Verified]
- **GitHub Copilot:** OpenAI-compatible at `https://api.githubcopilot.com` with OAuth device flow, client id from env `ODYSSEUS_COPILOT_CLIENT_ID`, default `01ab8ac9400c4e429b23`. `src/copilot.py:4`, `34-35` [Verified]
- Credentials are stored as `ProviderAuthSession` rows (`provider_auth_sessions` table). `core/database.py` table list [Verified]
- Endpoint API keys are encrypted in `ModelEndpoint.api_key`. `specs/llm-models.md:122` [Docs]
- "Codex" in `routes/codex_routes.py` is the **reverse direction**: scoped API-token HTTP surfaces that let the Codex CLI / Claude Code plugin *call Odysseus*, with scopes such as `memory:read`, `email:send`, `cookbook:launch`. `routes/codex_routes.py:1-40`; `integrations/claude/README.md` [Verified]

**Local models: Cookbook + HW Fit**
- Cookbook routes cover download, serve, GPU state, kill-pid, vLLM recipe lookup, serve diagnosis and task status. Serves run detached through tmux on POSIX or with PID/log wrappers on Windows. `specs/cookbook-hwfit.md:37-60` [Docs]
- `model_serve` is the HTTP handler. `routes/cookbook_routes.py:2019` [Verified]
- Engines named in cookbook code: `llama-server`/`llama_cpp`, `vllm serve`, `sglang`, `ollama serve`, `mlx_lm`, `diffusion_server`. (grep counts in `routes/cookbook_routes.py` / `cookbook_helpers.py`) [Verified]
- The agent tool `serve_model` description says: "Start serving a model with vLLM, SGLang, llama.cpp, Ollama, MLX Image, or Diffusers". `src/tool_schemas.py:1318` [Verified]
- vLLM recipes are fetched from `vllm-project/recipes`. `specs/cookbook-hwfit.md:56` [Docs]
- Serving auto-registers a `ModelEndpoint`. `specs/cookbook-hwfit.md:53` [Docs]
- **Hardware-aware fit** (`services/hwfit/fit.py`):
  - Uses bandwidth tables (`GPU_BANDWIDTH`, Apple per-core tables) and per-backend fallback constants `FALLBACK_K = {"cuda":220,"rocm":180,"metal":150,"cpu_x86":70,"cpu_arm":90}`. `services/hwfit/fit.py:9-52` [Verified]
  - `analyze_model(model, system, target_quant, scoring_use_case, target_context)` scores quality, speed, fit and context per quant. `services/hwfit/fit.py:184-431` [Verified]
  - Hardware is detected for NVIDIA, AMD, Apple Silicon, CPU and remote SSH. The fit uses a 90-percent VRAM budget. It is described as an MIT llmfit adaptation, and the bundled catalogs are intentionally empty (populated by "Rescan" from the HF API). `specs/cookbook-hwfit.md:97-164`, `201` [Docs]
- **Local inference fairness:** `_local_model_slot` serializes local-endpoint traffic and puts foreground first, so background tasks don't "cross streams" on one GPU. `src/llm_core.py:111-178` [Verified]
- llama.cpp slot-affinity caching (`_apply_local_cache_affinity`) applies to local endpoints only. `src/llm_core.py:1134` [Verified]
- Context windows:
  - `model_context.get_context_length` probes `/v1/models`, plus llama.cpp `/slots` → `/props`. `src/model_context.py:269`, `401` [Verified]
  - It falls back to `DEFAULT_CONTEXT = 128000` with `known=False`. `src/model_context.py:106`, `246-285` [Verified]
  - There is a static `KNOWN_CONTEXT_WINDOWS` table. `src/model_context.py:112` [Verified]
  - Token estimation is heuristic: non-ASCII characters count individually, Latin characters at about 0.3 tokens. `specs/llm-models.md:110`; `src/model_context.py:502-519` [Docs/Verified defs]
- **Odysseus-tuned local models:** model IDs whose leaf contains `odysseus` or `ajax` get a compact tool-schema profile and are forced onto native tools. `src/model_profiles.py:7-38`; `src/agent_loop.py:8397-8401` [Verified]
- ROADMAP mentions fine-tuned "Odysseus-tuned local models" and speculative decoding. `ROADMAP.md` [Docs]

**Native vs text tool transport (models without native tools)**
- `_agent_route_tool_mode()` decides native vs text tools in this order:
  1. the endpoint's `supports_tools` flag;
  2. otherwise a keyword list (`gpt-4`, `claude`, `gemini`, `qwen3`, `llama-3.x`, `hermes`, `deepseek-v`, …);
  3. a deny list (`deepseek-r1`, `gpt-oss`);
  4. Ollama native or `/v1` on port 11434 always means text tools.
  
  `src/agent_loop.py:8388-8472` [Verified]
- Per-model overrides come from `model_tool_modes: none|compact|full`. `specs/llm-models.md:112` [Docs]
- **Text tool-call parser** (`src/tool_parsing.py`, 2,202 lines, entry point `parse_tool_blocks` at `:1850`). It accepts:
  - fenced blocks with the tool name as language tag (`_TOOL_BLOCK_RE`, `:35`);
  - `[TOOL_CALL]{…}[/TOOL_CALL]` (`:110`);
  - XML `<tool_call>/<function_call><invoke name=…><parameter …>`, including minimax (`:124-166`);
  - StepFun/DeepSeek DSML `<｜tool▁call▁begin｜>` tokens (`:180-184`);
  - `<tool_code>` and Gemma tool-call markers (`:188-201`);
  - Qwen `<function=…><parameter=…>` (`:209-212`);
  - `<function_model>` from MLX/Exo (`:231-238`);
  - raw OpenAI `{"function":…}` JSON (`:845-1009`);
  - bare web-search JSON (`:551`);
  - **Hermes/Qwen JSON inside `<tool_call>`** (`_parse_json_tool_call_body`, `:1334`). This is strict: it needs a string `name`, and `arguments` must be an object. Tests: `tests/test_tool_parsing_hermes_json.py` (issue #5187).
  
  [Verified]
- "Hermes" in that parser is the NousResearch **Hermes-model tool-call format**, not the Hermes agent. [Verified from docstring]

**Pricing**
- `MODEL_PRICING` holds per-1M-token USD (input, output) prices for Anthropic, OpenAI, DeepSeek, Google and others. It is ported from the JS table `static/js/chatRenderer.js`. `src/model_pricing.py:1-60` [Verified]
- Policy: "Unknown models return None — we never guess a price." Local, self-hosted and subscription endpoints are free, and a provider-reported `usage.cost` (OpenRouter) is preferred. `src/model_pricing.py:1-11` [Verified]
- Risk: the table looks stale. It lists `claude-opus-4` at (15, 75) and `gpt-5` at (2.00, 8.00). `src/model_pricing.py:22-35` [Inference: values not checked against current list prices]

---

## D5 Prompt / context assembly & compaction

**In plain words.** The system prompt is built from parts, and each tool contributes a section that is included only if the tool is offered this turn. On top of that come injected messages: date/time, `AGENTS.md` from the workspace, active document/email, matched skills and memories. All user-editable or external content is wrapped as "untrusted" user-role data, not system text. When the context reaches 85% of the window, the older half of the conversation is summarized by a utility model into a structured summary.

**Prompt assembly**
- `_assemble_prompt(tool_names, disabled_tools, compact)` builds `_AGENT_PREAMBLE`, then full-block tool sections, then a `## Additional tools` list of one-liners, then `_AGENT_RULES`, then domain rules. `src/agent_loop.py:8213-8269` [Verified]
- A compact variant is used for native-schema turns: "Use only the native tool schemas provided for this turn; do not write tool syntax in chat". `src/agent_loop.py:8218-8241` [Verified]
- `TOOL_SECTIONS` holds about 70 per-tool prompt sections. `src/agent_loop.py:7896-8212` [Verified]
- `AGENT_SYSTEM_PROMPT` is a legacy full prompt (all tools) "when RAG unavailable". `src/agent_loop.py:8272` [Verified]
- `_AGENT_PREAMBLE` / `_AGENT_RULES` are defined twice; the second definition (`:7032-7036`) overrides the first (`:6907-6913`). The first one still holds text such as "60s timeout per tool, 10K char output limit". `src/agent_loop.py:6907-6920`, `7032-7040` [Verified; the override is Python semantics]
- `_build_system_prompt(...)` (`src/agent_loop.py:14411`) [Verified]:
  - It caches the base prompt by `(disabled tools, mcp, admin, relevant tools, compact, owner, …)`. `:14517`
  - It appends MCP schemas through `mcp_mgr.get_all_openai_schemas(...)`. `:14554-14556`
  - It injects the request-local datetime message `current_datetime_context_message()`. `:14562-14579`
  - The active document and email go in as **separate** `untrusted_context_message(...)` messages. `:14713`, `:14796`
  - Workspace rules (coding, artifact or media) are appended to the agent prompt. `:14876-14895`
  - Qwen-3.8 "tool router" models get a special minimal prompt plus the latest user message only. `:14440-14467`
- **Workspace `AGENTS.md`:** `_workspace_agents_context_message(workspace)` walks from the workspace up to the filesystem root looking for `AGENTS.md` and loads at most 8, bounded. `src/agent_loop.py:4267-4291`, `14477` [Verified]
  - No `CLAUDE.md` loading was found in that function [Verified by grep; absence elsewhere not proven]
- **Tool retrieval.** `ToolIndex.retrieve(query, k=8)` uses embeddings plus keywords and falls back to keyword search. `src/tool_index.py:199`, `357`, `601` [Verified]
  - `ALWAYS_AVAILABLE` includes `bash, python, read_file, grep, glob, ls, web_search, web_fetch, pdf_extract, inspect_media, extract_text, transcribe_media, youtube_tool, manage_memory, ask_user, update_plan, …`. `src/tool_index.py:37-62` [Verified]
- **Skills injection:**
  - Skills matched to the last user message go in as an untrusted message with `threshold=0.25` and `max_items = skill_max_injected` (default 3, clamped 0..12). The minimum confidence comes from `skill_min_confidence`, else `skill_autosave_min_confidence` (default 0.85). `src/agent_loop.py:14951-14990` [Verified]
  - A TUI can also force `active_skills`, which adds `manage_skills` and the skill's `requires_toolsets` tools. `src/agent_loop.py:14483-14505` [Verified]
  - A "level-0 skill index" (names plus short descriptions) is built in `_build_base_prompt`. Full procedures load on demand (progressive disclosure). `src/agent_loop.py:14523-14540`; `docs/skills-lifecycle.md:17-19` [Verified/Docs]
- **Memory, RAG, web and URL preface** are built in `ChatProcessor.build_context_preface()` and `routes/chat_helpers.build_chat_context()`. Pinned memories plus keyword/vector-retrieved memories are wrapped as untrusted, and use counts are incremented. `specs/context-building.md:94-101`; `specs/memory-skills.md:30-36` [Docs]
- **Untrusted wrapper:** `untrusted_context_message(label, content)` builds a user-role message with `metadata.trusted=False` and an `arm_tool_gate` flag. Once untrusted content is visible to the model, high-impact, unknown and MCP tool calls need an exact one-use approval. `specs/context-building.md:40-54`, `77-79` [Docs]; imported at `src/agent_loop.py:60` [Verified]
- `MAX_CONTEXT_MESSAGES = 90`; tool output caps are `MAX_OUTPUT_CHARS = 10_000`, `MAX_READ_CHARS = 20_000`, `MAX_DIFF_LINES = 400`. `src/constants.py:121-134` [Verified]

**Budgets**
- `compute_input_token_budget`: `DEFAULT_BUDGET = 6000` acts as an "auto" sentinel. The auto budget is `context_length × 0.85`, capped at `DEFAULT_HARD_MAX = 200_000`. An unknown window means the conservative 6000. An explicit user budget is honoured, clamped to the window. `src/context_budget.py:16-68` [Verified]
- `generation_budget` estimates the token cost of tool schemas and images (`MULTIMODAL_IMAGE_TOKEN_RESERVE = 1024`) and fits the output limit. `src/generation_budget.py:46-155` [Verified]

**Compaction**
- `COMPACT_THRESHOLD = 0.85`, overridable by the setting `auto_compact_threshold_percent`, clamped to 50–95. Also `SUMMARY_MAX_TOKENS = 1024` and `SMALL_CONTEXT_LIMIT = 8192`. `src/context_compactor.py:95-106` [Verified]
- `maybe_compact()` algorithm (`src/context_compactor.py:474-630`) [Verified]:
  1. Estimate tokens; below the threshold, do nothing.
  2. Split out system messages. If there are fewer than 4 conversation messages, do nothing.
  3. **Summarize the older half** (each message truncated to 2,000 chars), keep the recent half.
  4. Summarize with the **utility model** (`resolve_endpoint("utility")`), falling back to the session model, at temp 0.2 with a 30 s timeout.
  5. Use a "Cursor-style" structured prompt: User Goal / What Was Done / Current State / Pending / Key Context, under 1000 tokens.
  6. Insert `{"role":"system","content":"[Conversation summary — earlier messages were compacted]\n…"}`.
  7. Persist to session history unless `persist=False`.
  8. If the summary call fails, keep the conversation intact.
- Deterministic mode (unattended workers) trims to 75% with no LLM call. `src/context_compactor.py:507-528` [Verified]
- `trim_for_context()` strips RAG and memory system messages first, then older turns. `_protected` messages, such as the active document, are never trimmed. `src/context_compactor.py:305-330` [Verified]
- `_sanitize_tool_messages` repairs orphaned `tool` messages after front-trimming. `src/context_compactor.py:146-160` [Verified]
- When foreground fallback is active, compaction is speculative per candidate and persisted only for the route that actually answers (`apply_compaction_state`). `src/context_compactor.py:633-660` [Verified]
- The agent loop calls `maybe_compact` before the first round (`:24097`) and on every round (`:25867`), and emits `{"type":"compacted","context_length":…}`. `src/agent_loop.py:27299-27449` [Verified]
- `prune_multimodal_images` drops old image blocks. `src/context_compactor.py:45` [Verified]

---

## D6 Tools catalog, schema, MCP, shell/file

**In plain words.** About 88 built-in native tools covering shell, files, web, documents, email, calendar, notes, memory, skills, model delegation, Cookbook model serving and admin. They are defined as OpenAI function schemas in `src/tool_schemas.py`. A parallel prompt-text registry serves text-tool models, and a retrieval index picks the relevant subset each turn. MCP servers (stdio/SSE/Streamable HTTP with OAuth) are added as `mcp__{server}__{tool}`. Shell and Python run in a bubblewrap sandbox on Linux when available. Every non-trivial action goes through an approval/authority layer.

**Schema and registry**
- `FUNCTION_TOOL_SCHEMAS` (`src/tool_schemas.py:287`) has **88 unique tool names** [Verified by extraction].
- Handlers are spread across these registries [Verified/Docs]:
  - `TOOL_HANDLERS` in `src/agent_tools/__init__.py:38-77`, extended by `ADMIN_TOOL_HANDLERS`;
  - domain `do_*` handlers in `src/tools/{calendar,contacts,cookbook,image,notes,research,search,system,vault}.py`;
  - aliases in `_TOOL_NAME_MAP` (`src/tool_parsing.py:308`);
  - retrieval text in `BUILTIN_TOOL_DESCRIPTIONS` (`src/tool_index.py`);
  - dispatch and hard gates in `execute_tool_block` (`src/tool_execution.py`).
- The spec admits the "registry consistency is manual" and that tool descriptions are duplicated three times. `specs/agent-tools.md:206-210` [Docs]
- Non-admin or public users are blocked from `bash`, `python`, file tools, `manage_mcp` and all `mcp__*`. `specs/shell-mcp.md:135` [Docs]

**Catalog (name — purpose; schema line in `src/tool_schemas.py`)** [Verified, descriptions paraphrased from the schema strings]
- **Execution:**
  - `bash` (291) — shell command, "full access";
  - `host_shell` (305) — shell through an advertised TUI-host bridge, not the container;
  - `python` (320) — run Python;
  - `manage_bg_jobs` (1910) — list/control detached `#!bg` bash jobs.
- **Files/code:**
  - `read_file` (516), `write_file` (587), `edit_file` (605, exact string replace), `apply_patch` (622, multi-file patch);
  - `grep` (532, ripgrep), `glob` (550), `ls` (565);
  - `get_workspace` (579);
  - `todowrite` (639) — structured coding task list.
- **Web/media:**
  - `web_search` (334, SearXNG by default), `web_fetch` (362, up to 12 URLs concurrently), `pdf_extract` (379);
  - `private_browser` (395, session metadata only, page ops denied);
  - `inspect_media` (424, multimodal), `extract_text` (456, OCR), `transcribe_media` (475, local Whisper);
  - `youtube_tool` (496);
  - `get_weather` (350, Open-Meteo).
- **Documents (editor panel):** `create_document` (665), `edit_document` (685), `suggest_document` (716, inline comment bubbles), `update_document` (743, full rewrite), `manage_documents` (1265).
- **Sessions / multi-model:**
  - `search_chats` (757, FTS over past chats);
  - `chat_with_model` (771, ask another model);
  - `create_session` (786), `list_sessions` (801), `send_to_session` (815);
  - `pipeline` (830, Draft→Critique→Revise chain);
  - `manage_session` (855, rename/archive/fork/truncate);
  - `ask_teacher` (1136, ask a stronger model);
  - `list_models` (892).
- **Memory/skills:** `manage_memory` (872), `manage_skills` (1151).
- **Interaction/UI:** `ask_user` (959, multiple-choice), `update_plan` (986), `ui_control` (906).
- **Productivity:**
  - `manage_tasks` (1000, scheduled tasks), `manage_calendar` (1046), `manage_notes` (1081);
  - `resolve_contact` (1565), `manage_contact` (1579, CardDAV);
  - `api_call` (1119, registered API integrations).
- **Email:** `list_email_accounts` (1600), `send_email` (1611), `draft_email` (1628), `draft_email_reply` (1648), `list_emails` (1667), `search_emails` (1687), `read_email` (1705), `download_attachment` (1721), `scan_email_unsubscribes` (1738), `scan_spam` (1754), `unsubscribe_email` (1770), `reply_to_email` (1788), `bulk_email` (1805), `block_sender` (1824), `manage_email_state` (1842), `delete_email` (1860), `archive_email` (1877), `mark_email_read` (1893).
- **Research/images:** `trigger_research` (1533, background deep research), `manage_research` (1549), `generate_image` (1499), `edit_image` (1516).
- **Cookbook (local model ops):** `download_model` (1301), `serve_model` (1318), `list_served_models` (1335), `stop_served_model` (1343), `tail_serve_output` (1357), `list_downloads` (1372), `cancel_download` (1380), `search_hf_models` (1394), `list_cookbook_servers` (1412), `list_serve_presets` (1420), `adopt_served_model` (1428), `serve_preset` (1448), `list_cached_models` (1463).
- **Admin:**
  - `manage_endpoints` (1194), `manage_mcp` (1212), `manage_webhooks` (1231), `manage_tokens` (1249), `manage_settings` (1284);
  - `app_api` (1480) — generic loopback to allowed internal Odysseus HTTP endpoints, using OpenAPI discovery and blocklists. `specs/agent-tools.md:118` [Docs]

**Shell / file execution details**
- Defaults: `DEFAULT_BASH_TIMEOUT = 120`, `DEFAULT_PYTHON_TIMEOUT = 60*60`. `src/agent_tools/subprocess_tools.py:27-28` [Verified]
- Legacy constants `SHELL_TIMEOUT = 60`, `PYTHON_TIMEOUT = 30` live in `src/agent_tools/__init__.py:87-88` [Verified]
- The two sets of values disagree with each other and with the prompt text ("60s timeout per tool"). [Inference: unclear which applies on each path]
- Output is capped at `MAX_OUTPUT_CHARS` (10k). `src/agent_tools/subprocess_tools.py:583-589` [Verified]
- **Containment:**
  - `src.containment.agent_spec()` requires filesystem, process-tree and wall-clock enforcement. Linux uses **bubblewrap** after a functional mount/PID probe. If a required mechanism is missing, the tool returns containment-unavailable **before code runs**.
  - Network, memory and process-count limits are not assumed to be enforced.
  - Native enforcing execution is unavailable on Windows.
  
  `specs/shell-mcp.md:54`, `149-159` [Docs]
- Background jobs: a `#!bg` first line detaches the job. State lives in `data/bg_jobs.json` with `DEFAULT_MAX_RUNTIME_S = 3600` and `_MAX_OUTPUT_CHARS = 16000`. `bg_monitor` auto-continues the chat when the job finishes. `src/bg_jobs.py:47-49`; `specs/shell-mcp.md:54-65` [Verified/Docs]
- File tools are confined to the workspace or allowed roots, with case-insensitive sensitive-path denial (`.env`, SSH/GPG, `id_rsa`), denial of hardlinked files, and exclusive creation. `specs/agent-tools.md:106`, `174` [Docs]
- **Approvals:**
  - `ODYSSEUS_TOOL_APPROVAL_GATE` defaults on (`"1"`). `src/tool_capabilities.py:686` [Verified]
  - Exact approvals are sealed to owner, session, run, content, workspace and document digest. Choices are task, chat or deny. `specs/agent-tools.md:110`, `172` [Docs]
- **Durable effects journal:** proposed → normalized → authorized → dispatched → settled. Intent, outcome and observed state are kept apart, so "a successful receipt alone does not establish resulting state". `specs/agent-tools.md:148`, `182` [Docs]
  - `src/agent_runtime/{journal,effects,effect_log,completion}.py` exist [Verified listing]

**MCP client**
- `McpManager` supports three transports: stdio (`mcp.client.stdio.stdio_client`), SSE (`sse_client`) and Streamable HTTP (`streamablehttp_client(url, auth=provider)`, with OAuth). `src/mcp_manager.py:214-386` [Verified]
- Qualified tool names are `mcp__{server_id}__{tool_name}`, and `get_all_openai_schemas(disabled_map)` converts tools to OpenAI function schemas. `src/mcp_manager.py:561-562`, `688-714` [Verified]
- Each server has a 20 s connect timeout (`timeout=20`, `src/mcp_manager.py:524`) [Verified]. Servers start concurrently. `specs/shell-mcp.md:78` [Docs]
- Built-in MCPs reconnect once on error, but only when there is no bound backend identity. User MCP servers don't auto-reconnect. `specs/shell-mcp.md:97` [Docs]
- OAuth tokens are stored encrypted in `McpServer.oauth_tokens`, but **`McpServer.env` is plaintext JSON**. `specs/shell-mcp.md:76`, `126` [Docs]
- Disabled MCP tools are hidden from prompts and schemas but are "not a complete execution-time gate". `specs/shell-mcp.md:114` [Docs]
- MCP output has **no central size cap** before it goes back to the model. `specs/agent-tools.md:211` [Docs]
- The agent can add stdio MCP servers itself through `manage_mcp`, with a command allowlist/denylist. `specs/shell-mcp.md:82` [Docs]

**MCP servers shipped**
- `BUILTIN_PYTHON_SERVERS`: `image_gen`, `memory`, `rag`, `email` (`mcp_servers/*.py`, built with the `mcp.server` SDK). `src/builtin_mcp.py:73-76` [Verified]
- Optional browser server: `npx -y @playwright/mcp@latest --headless --caps vision`. It is cache-gated and not downloaded at boot. `src/builtin_mcp.py:84`, `204-247` [Verified]
- Image, memory and RAG built-ins "normally defer to native" tools. `specs/shell-mcp.md:95` [Docs]
- Odysseus is **not** exposed to other agents as an MCP server. Integration goes the other way: a scoped REST API (`/api/codex/*`) plus SKILL.md bundles for Claude Code (`~/.claude/skills/odysseus`) and a Codex plugin. `integrations/claude/README.md`; `integrations/codex/.codex-plugin/plugin.json` [Verified]

---

## D9 Memory & state

**In plain words.** App state lives in **SQLite** through SQLAlchemy: sessions, messages, tasks, endpoints, MCP servers and so on, 32 tables. Long-term "memories" are short facts in a **JSON file** (`data/memory.json`), optionally mirrored into **ChromaDB** for semantic search. The default local embedding model is FastEmbed `all-MiniLM-L6-v2`. An LLM extracts memories automatically every 4th message. Past chats are searchable with SQLite FTS5.

- **DB:** default `sqlite:///{DATA_DIR}/app.db`. `core/database.py:44` [Verified]
- The 32 tables include `sessions`, `chat_messages`, `background_tool_jobs`, `documents`, `document_versions`, `model_endpoints`, `provider_auth_sessions`, `mcp_servers`, `comparisons`, `api_tokens`, `webhooks`, `crew_members`, `scheduled_tasks`, `task_runs`, `memories`, `notes`, `calendars`, `calendar_events`, `integrations`, …. `core/database.py` (`__tablename__` grep) [Verified]
- **Sessions:** the `Session` row has per-session toggles `memory_extraction_enabled`, `memory_injection_enabled`, `skill_injection_enabled`, plus `thinking_mode`, `temperature_override`, `max_tokens_override`, `cwd`. `core/database.py:185-205` [Verified]
- `SessionManager` is "the single place" for loading and saving sessions and messages. `core/session_manager.py:1-10`, `86`, `241`, `740` [Verified]
- **Long-term memory store:** `MemoryManager` uses `data/memory.json` and supports pinned memories and use counts. `src/memory.py:47-49` [Verified]
  - Retrieval is `get_relevant_memories(query, memories, threshold=0.05, max_items=8)`, keyword-based. `src/memory.py:361` [Verified]
  - It refuses to overwrite a corrupt store (`MemoryStoreUnreadable`), and migrates the legacy `memory.txt`. `src/memory.py:140-165` [Verified]
  - A `memories` SQL table also exists. `core/database.py:893-916` [Verified]
  - A comment says `core/database.py` "rewrites [memory.json] … during migration". `src/memory.py:147` [Verified] — so there are two representations (see Open questions).
- **Vector memory:** `MemoryVectorStore`, `COLLECTION_NAME = "odysseus_memories"`, on ChromaDB. It uses embedding "lanes": `fastembed` (local) and `custom` (an HTTP embedding endpoint). Collections are fingerprinted by lane, URL, model and dimension so they reset when the embedder changes. `src/memory_vector.py:25-49`; `src/embedding_lanes.py:19-275` [Verified]
- The default FastEmbed model is `sentence-transformers/all-MiniLM-L6-v2` (env `FASTEMBED_MODEL`), and `FASTEMBED_THREADS` is validated to 1..256. `src/embeddings.py:39`, `145`, `187-194` [Verified]
- Chroma runs at `localhost:8100` natively or `chromadb:8000` in Docker. Memory degrades to keyword-only if Chroma is down **at startup**, but a failure after startup "can still break memory retrieval". `specs/memory-skills.md:77-79`, `146` [Docs]
- **Automatic extraction:**
  - Runs only when `msg_count >= 4 and msg_count % 4 == 0`, and not when incognito, in compare mode, when the session toggle is off, or when the user pref `auto_memory` is off. `routes/chat_helpers.py:1736-1747` [Verified]
  - The extractor uses `CONTEXT_WINDOW = 6` messages, audits memories every 5 new ones (`AUDIT_INTERVAL = 5`) and auto-pins at most `AUTO_PINNED_IDENTITY_LIMIT = 5` identity facts. It also has a text-dedupe threshold of 0.6. `services/memory/memory_extractor.py:91`, `137-138`, `262` [Verified]
  - Persona memories are stored separately on presets (`update_persona_memory`). `services/memory/memory_extractor.py:546` [Verified]
- **Session search:** the `search_chats` tool uses a `chat_messages_fts` FTS5 table (with query sanitizing) when it is present. `src/session_search.py:79-125` [Verified]
- **RAG over documents:** `src/rag_vector.VectorRAG` is owner-filtered; the RAG MCP server is built in. `specs/context-building.md:90` [Docs]
- **Owner isolation:** most rows carry `owner`. The memory MCP server fails closed when owner-bearing rows exist but no owner env is set. `specs/memory-skills.md:60` [Docs]

---

## D10 Skills & self-improvement

**In plain words.** Skills are `SKILL.md` files with YAML frontmatter, explicitly modelled on the **Hermes agent's skill format**. They are mostly compatible with Hermes and Claude Code skills, plus extra lifecycle fields. Skills are written by: (a) an LLM that inspects completed complex agent runs, (b) a "teacher" model that takes over when a local model fails, (c) the user, and (d) GitHub/skills.sh import. A background audit then tests each skill against a no-skill baseline before it is "approved" for injection.

**Format**
- The docstring says "Inspired by Hermes' skills format (hermes-agent.nousresearch.com/docs/user-guide/features/skills)". `services/memory/skill_format.py:1-5` [Verified]
- Frontmatter fields: `name, description, version, category, tags, platforms, requires_toolsets, fallback_for_toolsets, status (draft|published), confidence (0..1), source (learned|taught|imported), teacher_model, created`. Body sections: `## When to Use / ## Procedure / ## Pitfalls / ## Verification`, and any other content is kept in `body_extra`. `services/memory/skill_format.py:7-41` [Verified]
- Built-ins also carry `owner: ""` and `source: builtin`. `resources/skills/agent/tool-discovery/SKILL.md:1-12` [Verified]
- Usage counters are kept in a sidecar `_usage.json` "so the SKILL.md file doesn't churn". `services/memory/skill_format.py:43-44` [Verified]
- On-disk layout is `data/skills/<category>/<name>/SKILL.md`, with legacy `data/skills.json` read-only. `specs/memory-skills.md:44` [Docs]
- **Compatibility** [Inference]:
  - Hermes skills (`name`, `description`, `version`, `platforms`, `metadata.hermes.tags`, …) and Claude Code skills (`name`, `description`) should parse, because both need only name and description.
  - Odysseus adds `status`, `confidence` and `requires_toolsets`. Imported skills without `status: published` may not be injected when auto-approve is on.
  - Hermes' `metadata.hermes.*` nesting and `references/` linked files are not modelled in the documented frontmatter.
  - The importer fetches bundle files (≤64 files, ≤2 MB total, ≤400 KB each, allowed suffixes `.md/.py/.sh/.json/…`). `services/memory/skill_importer.py:19-30` [Verified]
- **Built-in skills:** 12 under `resources/skills/` across `agent/`, `communication/`, `dev/`, `general/`, `media/`, `research/`, for example `tool-discovery`, `terminal-recovery`, `verified-state-change`, `artifact-completion`, `web-research-fallback`, `test-driven-development`. They are synced at startup into an immutable shared catalog with confidence 1.0. `specs/memory-skills.md:110` [Verified listing/Docs]
- The lifecycle doc credits **NousResearch/hermes-agent** (MIT, commit `cfdbbb6e…`), specifically `tools/skills_tool.py` and `agent/prompt_builder.py`, for progressive disclosure, and says "no Hermes implementation code was copied". `docs/skills-lifecycle.md:21-26` [Verified]

**Who writes skills**
- **1. Auto-extraction after an agent turn:**
  - Gate: `extract_skills` and user pref `auto_skills` (default True), not incognito, not compare. `routes/chat_helpers.py:1775-1815` [Verified]
  - The run must be complex: `tool_calls >= 4 or (rounds >= 5 and tool_calls >= 3)`. `routes/chat_helpers.py:57-59` [Verified]
  - The extractor has its own floor of rounds ≥2 or tools ≥2. `services/memory/skill_extractor.py:146-148` [Verified]
  - Prompt: "Extract a reusable 'skill' ONLY IF … a concrete, repeatable procedure … ON THE COMPUTER". Otherwise it returns `null`, and it prefers "a specific successful workaround, an unexpected pitfall". `services/memory/skill_extractor.py:15-46` [Verified]
  - Context is `CONTEXT_WINDOW = 12` messages, `MIN_CONFIDENCE = 0.6`. Saved skills get `status="draft"` and `source="learned"`. `services/memory/skill_extractor.py:49-52`, `~270-283` [Verified]
- **2. Teacher escalation:**
  - When a **self-hosted** student model fails in agent mode and the `teacher_model` setting is configured, a frontier "teacher" endpoint produces the corrective reply **and writes a SKILL.md** so the student can do it next time.
  - The skill is saved only if the teacher's own reply passes the same evaluation.
  - Detection Tier 1 is regex. Tier 2 is an LLM self-eval (`evaluate_turn_llm` exists, but the docstring still says "TODO").
  
  `src/teacher_escalation.py:1-23`, `147`, `446`, `532`, `607-801` [Verified]
- **3. Manual:** `manage_skills` tool, `/api/skills` CRUD, `scripts/odysseus-skills`. `specs/memory-skills.md:20`, `60` [Docs]
- **4. Import** from GitHub or skills.sh URLs (admin-gated, SSRF-pinned). `services/memory/skill_importer.py:1`; `specs/memory-skills.md:46`, `98` [Verified/Docs]

**Lifecycle / evaluation ("self-improvement")**
- UI tabs: All / Built-in / Approved / Draft. "Approved" means published, passing the audit, at or above the confidence threshold, and not "unnecessary". `docs/skills-lifecycle.md:3-7` [Docs]
- **Audit protocol v2** runs the task **with and without the skill** and stores the functional verdict, the baseline verdict, usefulness, saved turns and saved tool calls. `specs/memory-skills.md:114` [Docs]
- Confidence after a manual test: pass 0.95, needs_work 0.6, fail 0.4. After an audit: clean pass 0.95, self-edit 0.85, teacher rewrite 0.8; a continued fail becomes a draft at 0.35. `specs/memory-skills.md:136` [Docs]
- Duplicates are grouped by token-Jaccard ≥ 0.38 and "binned". `specs/memory-skills.md:122` [Docs]
- **Queue:** `automatic_audit_candidates(skills, limit=8)`. `services/memory/skill_lifecycle.py:5` [Verified]
  - New skills are eligible immediately, inconclusive ones retry daily, failed ones retry weekly. A daily "Skills Audit" scheduled task drives the queue. `docs/skills-lifecycle.md:9-15` [Docs]
- Pipeline modules: `services/skills/{agent_runner,audit_workflow,evaluation,job_coordinator,model_selection,prompts,publication}.py`, covering self-edit, worker retry and "distinct-teacher rewrite". `specs/memory-skills.md:132` [Docs/listing Verified]
- Skill retrieval combines lexical scores with semantic scores from `src/skill_index.py`, which reuses the ToolIndex embedding lane. `remote-only` skills are hidden from local models. `specs/memory-skills.md:112` [Docs]

---

## D11 Orchestration

**In plain words.** Odysseus doesn't spawn full agent subprocesses the way Claude Code's Task tool does. It offers several lighter mechanisms:
- tools that call **another model** once (`chat_with_model`, `ask_teacher`) or in a chain (`pipeline`);
- tools that message **other chat sessions** (`create_session`, `send_to_session`);
- a fresh-context **verifier** and a **teacher takeover**;
- a **scheduler** that runs full agent loops on cron/event triggers;
- a built-in **Deep Research** loop (plan → search → read → synthesize → decide → report);
- a **Compare** UI for blind multi-model A/B testing.

**Delegation tools**
- `chat_with_model` and `ask_teacher` are single `llm_call_async` calls with `AI_CHAT_TIMEOUT = 120`. `src/agent_tools/model_interaction_tools.py:30-109`; `src/ai_interaction.py:30` [Verified]
- `pipeline` takes JSON `{"steps":[{"model","instruction"}]}` or `model | instruction` lines, up to `MAX_PIPELINE_STEPS = 10`.
  - All models are resolved first (fail fast). Then the steps run sequentially, with each step's output fed in as "Previous step's output". These are plain LLM calls without tools.
  
  `src/ai_interaction.py:32`, `222-300` [Verified]
- `send_to_session` sends a message to another live chat. It is implemented as `llm_call_async` with that session's model, **not a full agent loop**. `src/agent_tools/session_tools.py:177-243` [Verified]
- The prompt advertises `create_session` "for background/parallel work". `src/agent_loop.py:8119` [Verified]
- The verifier subagent (fresh context, off by default) is covered in D3. `src/agent_loop.py:16793` [Verified]
- Teacher takeover: `@with_teacher_takeover` wraps the agent loop, and the teacher runs after the parent stream contexts close. `src/agent_loop.py:21027`; `specs/agent-tools.md:180` [Verified/Docs]

**Scheduled agent tasks**
- `TaskScheduler._execute_task_locked` branches on `task_type`:
  - `"action"` runs a built-in action;
  - `"research"` runs `DeepResearcher` directly;
  - anything else runs `_execute_llm_task`, which uses `_run_agent_loop`, i.e. the full `stream_agent_loop` with tools.
  
  `src/task_scheduler.py:1009-1022`, `1952-1975` [Verified]
- Agent-task rounds: `task.max_steps if > 0 else 20`. Research tasks use `max_rounds=8`. `src/task_scheduler.py:1997`, `2198` [Verified]
- `ScheduledTask` columns:
  - schedule `once/daily/weekly/monthly`, plus `cron_expression`;
  - `trigger_type` `schedule|event` with `trigger_event` (e.g. `session_created`, `message_sent`) and `trigger_count` ("fire every N events");
  - `then_task_id` for **chaining**, with a cycle check `_has_chain_cycle(max_depth=10)`;
  - `webhook_token` for webhook triggers;
  - `crew_member_id` / `character_id` (personas);
  - `output_target`, `email_results`, `request_authority_json` (a sealed authority snapshot).
  
  `core/database.py:776-810`; `src/task_scheduler.py:2269-2296` [Verified]
- Results go to a session, email, or an MCP tool (`_deliver_via_mcp`). `src/task_scheduler.py:1772-1952`, `2311` [Verified]
- Time zones are per task (`compute_next_run(..., tz_name)`). `src/task_scheduler.py:126-135` [Verified]
- Background runs yield to the foreground: they are cancelled when the user becomes active. `src/task_scheduler.py:1000-1007`, `2414` [Verified]

**Deep research pipeline** (`DeepResearcher.research`, `src/deep_research.py:438-622`) [Verified]
- Defaults: `max_rounds=8, max_time=300, max_urls_per_round=3, max_content_chars=15000, max_report_tokens=8192, extraction_timeout=90, planning_timeout=90, query_timeout=120, extraction_concurrency=3, min_rounds=2, max_empty_rounds=2, synthesis_window=10`. `src/deep_research.py:345-366`
- Steps:
  1. **PLAN** with `RESEARCH_PLAN_PROMPT`. Skipped for "small local model" names (≤10B parameters, or names containing `odysseus`/`heretic`/`trial55`). `:413-424`, `:468-490`
  2. **CLASSIFY** into a category with `CATEGORY_PROMPTS`. `:491-494`
  3. Per round, **THINK**: `_plan_research_actions` produces bounded `ResearchAction`s (web_search/fetch/browser_read). Explicit URLs from the question are added in round 1. If no actions come back, it falls back to `_generate_queries`. `:513-529`
  4. **SEARCH + EXTRACT**: `_execute_research_actions` uses the provider chain (SearXNG/Brave/Tavily, …) and fetches and extracts each page with the content wrapped as untrusted. `:536`, `:1136-1451`
  5. **SYNTHESIZE**: builds an evolving report. `:572`, `:1700`
  6. **DECIDE**: after `min_rounds`, the loop first checks `_needs_more_evidence_before_stop` (source coverage), then asks the LLM `STOP_PROMPT`. `:575-589`, `:1678-1771`
  7. **FINAL REPORT**: `FINAL_REPORT_PROMPT`. If synthesis failed, the gathered findings are returned as a fallback report. `:591-622`
- A "fast fact" mode applies to small models when the question starts with where/what/who/when/how many: 1 round and a deterministic compact report. `:427-464`
- Two consecutive empty rounds are treated as "search engine unavailable". `:547-560`
- Chat-triggered research defaults to **2 rounds and a 120 s soft budget**, runs as a durable background job, and summarizes its result back into the originating chat. `docs/BACKGROUND_TOOL_JOBS.md:1-25` [Docs]
- Reports are persisted to `data/deep_research/<session_id>.json` with an HTML visual report, and "Spinoff/Discuss" can start a new chat from a report. `specs/research.md:77-89` [Docs]

**Compare / synthesis**
- Blind side-by-side compare creates `[CMP]` sessions, one per model, and each pane streams through the normal `/api/chat_stream` with `compare_mode=true`. Compare mode strips document tools and suppresses memory, skill and webhook side effects. Votes are stored in `comparisons`, and the scoreboard is in localStorage. "Search compare" adds synthesis sessions. `specs/compare.md:20-50` [Docs]
- Blind mode masks model identity only in the UI/API. It is "not a full confidentiality boundary". `specs/compare.md:52` [Docs]

---

## Migration from other agents

- **What exists:** only a **read-only manifest builder**, `scripts/agent_migration_manifest.py` (635 lines). Its docstring says "does not import the Odysseus application package, write to data/, call an LLM, or apply anything". `scripts/agent_migration_manifest.py:1-8` [Verified]
- **No importer/apply step is implemented.** The doc says "A future Odysseus importer should…". `website/agent-migration.md:170-185` [Verified; the only code references are the script and `tests/test_agent_migration_manifest.py`]
- **Output schema** `agent-migration.v1` (`SCHEMA_VERSION`, `scripts/agent_migration_manifest.py:22`) has four item kinds [Verified]:
  - `memory`: from JSON lists of strings or `{text, category, source}`, or objects under `memories|memory|items|data`. `:112-150`
  - `skill`: recursively scans directories for `SKILL.md` and parses the frontmatter. `:350-407`
  - `conversation_thread`: generic `{id,title,messages}` or **ChatGPT `conversations.json` `mapping` exports**. Metadata only by default, with `--include-conversation-content` and `--max-conversation-messages`. `:237-349`
  - `archive_document`: text files from folders, path/hash/size only unless `--include-archive-content`. `:408-505`
- **Hermes/OpenClaw specifics:** `--source-kind` is only a label ("e.g. generic, openclaw, hermes"). `:564` [Verified]
  - Nothing parses Hermes or OpenClaw config, sessions or memory files. The documented Hermes example just points `--skills-dir ~/.hermes/skills`.
  - The doc says source-specific adapters (OpenClaw workspace files, Hermes `config.yaml`, Claude export) "can" exist. They are not shipped.
  
  `website/agent-migration.md:107-115`, `187-198` [Verified]
- **Recommended apply policy (not implemented):** dry-run, then back up `data/`, import archives as documents (not memory), review memory candidates, check skill conflicts, skip secrets by default. `website/agent-migration.md:170-185` [Docs]
- Related live import paths:
  - skill import from GitHub/skills.sh (D10);
  - `/api/import` for an Odysseus JSON backup (memories and skills). `specs/memory-skills.md:64` [Docs]

---

## Distinctive ideas (worth stealing)

1. **Turn contract.** Before each turn, deterministic per-turn capability classification freezes an immutable set of schemas (`required ⊆ offered ⊆ executable`). If a required tool is unavailable, the turn stops instead of substituting another tool. `docs/AGENT_TURN_CONTRACT.md:8-29`; `src/agent_loop.py:21127-21138`
2. **Selection ≠ authority.** Retrieval, schemas and provider tool calls are all "requests". Real authority is a server-side `RequestAuthority` plus resource binding (PID+start-token, file identity, document digest, backend incarnation) plus exact one-use approvals sealed to those bindings. `specs/agent-tools.md:144-172`
3. **Taint-armed approval gate.** Once any untrusted external content becomes model-visible, high-impact, unknown and MCP actions require exact approval. Failed tool results that carry remote content still arm the gate. `specs/context-building.md:77-79`
4. **Effects journal with intent/outcome/observed separation**, and a completion gate that holds back the final answer until evidence supports it. It can strip unsupported "all tests pass" claims from the final prose. `specs/agent-tools.md:148`, `182`, `190-192`
5. **Teacher→student distillation loop.** A local model fails, a frontier teacher fixes the turn and writes a SKILL.md, and the skill is then A/B-audited against a no-skill baseline before publication. `src/teacher_escalation.py:1-23`; `specs/memory-skills.md:114`, `136`
6. **Skill audit with baseline utility** ("does the skill actually save turns/tool calls?") plus daily/weekly retry queues and Jaccard dedupe binning. `docs/skills-lifecycle.md`; `specs/memory-skills.md:122`
7. **Very broad text-tool-call recovery**, covering about 10 wire formats (fenced, Hermes JSON, Qwen XML, minimax, DSML, Gemma, `<function_model>`, raw OpenAI JSON). It uses forward-only scanners to avoid O(n²) regex. `src/tool_parsing.py:35-1345`
8. **Foreground-priority local GPU gate**, so background jobs don't compete with the user on a single local inference pipe. Scheduled tasks are cancelled on user activity. `src/llm_core.py:111-178`; `src/task_scheduler.py:1000-1007`
9. **Hardware-fit model recommendation plus one-click serve/auto-register** across llama.cpp, vLLM, SGLang, Ollama and MLX, also exposed to the agent as tools (`serve_model`, `tail_serve_output`, …). `services/hwfit/fit.py:431`; `src/tool_schemas.py:1301-1463`
10. **Route-neutral compaction during fallback.** Compaction is speculative per candidate model and persisted only for the route that actually answered. `src/context_compactor.py:633-660`
11. **Loop-breaker with a forced tool-less answer round**, plus an evidence-driven artifact recovery mode that narrows tools to "mutation" tools when required outputs are missing. `src/agent_loop.py:31492-31590`
12. **Fine-tuned house models** (`odysseus`/`ajax` profiles) with compact schemas and an SFT corpus from real sessions. `src/model_profiles.py:7-38`; `docs/ODYSSEUS_SFT_ALEX_CORPUS.md`

## Weaknesses / risks

- **Monolith.** `src/agent_loop.py` is 38k lines and the loop function is about 17k lines, with heavy model-name special-casing (`_qwen38_tool_router`, `_ody_qwen_terminal_tool_summary`, SFT-fixture owner checks such as `sft_alex_creator`). This makes it hard to reason about and easy to regress. `src/agent_loop.py:21029`, `21177-21198`; `docs/AGENT_TURN_CONTRACT.md:54` [Verified/Inference]
- **No parallel tool calls.** Multiple tool calls in one round run sequentially. `src/agent_loop.py:32172`, `33500` [Verified]
- **Two loops.** The legacy loop and the "clean v3 preview" loop share the executor but have different budgets and semantics. `specs/agent-tools.md:150` [Docs]
- **Heavy prompts** for small local models. The ROADMAP lists "Agent prompt/context bloat" as high priority, and the spec admits it. `ROADMAP.md`; `specs/agent-tools.md:207` [Docs]
- **Inconsistent timeouts:** the prompt says "60s timeout per tool", `SHELL_TIMEOUT = 60`, but `DEFAULT_BASH_TIMEOUT = 120` and `DEFAULT_PYTHON_TIMEOUT = 3600`. `src/agent_loop.py:6919`; `src/agent_tools/__init__.py:87-88`; `src/agent_tools/subprocess_tools.py:27-28` [Verified]
- **Compaction may use a guessed window.** `maybe_compact` calls `get_context_length`, which returns `DEFAULT_CONTEXT = 128000` for unknown models, so a small local model with an unknown window might never cross 85%. Callers *can* pass `context_length`. `src/context_compactor.py:494-495`; `src/model_context.py:106` [Inference]
- **Compaction is lossy:** each message is truncated to 2,000 chars before summarizing, and the summary is capped at 1,024 tokens. `src/context_compactor.py:548-551`, `96` [Verified]
- **Token estimation is heuristic**, with no tokenizer. `specs/llm-models.md:110` [Docs]
- **Memory:** it is a JSON file (not transactional across processes) with a parallel `memories` SQL table, and keyword retrieval when Chroma is down. A post-startup vector failure can break retrieval, the admin-wipe vector clear is "ineffective", and JSON import doesn't rebuild vectors. `specs/memory-skills.md:142-147` [Docs]
- **MCP gaps:** no central output cap, disabled tools are not enforced at execution, `McpServer.env` is plaintext, and there is no automatic reconnect for user servers. `specs/shell-mcp.md:187-198` [Docs]
- **Private browser is disabled.** `private_browser` admits only `session_info`, so there are no browser page actions despite the Playwright MCP. `specs/shell-mcp.md:153`; `src/tool_schemas.py:395` [Docs/Verified]
- **No true subagents.** Delegation is single LLM calls without tools (`chat_with_model`, `pipeline`, `send_to_session`). No isolated sub-agent with its own tool loop was found. [Verified for the three tools; full absence is Inference]
- **Detached runs are in memory** and lost on restart. `specs/agent-tools.md:94` [Docs]
- **Stale pricing table** (claude-opus-4 at 15/75, `gpt-5` at 2/8), which must be kept in sync with the JS table by hand. `src/model_pricing.py:18-35` [Verified]
- **Canonical model-capability records are not wired into runtime**, so tool-support detection is still model-name keyword matching. `specs/llm-models.md:47`; `src/agent_loop.py:8448-8458` [Docs/Verified]
- **Migration is manifest-only.** There is no importer, and no Hermes/OpenClaw adapters exist. `website/agent-migration.md:170` [Verified]
- **Self-described maturity:** the ROADMAP's first line is "SQUASH BUGS", and the author jokes "I don't know what I'm doing, help". `ROADMAP.md` [Docs]

## Open questions

- How do the `memories` SQL table (`core/database.py:893`) and `data/memory.json` (`src/memory.py:49`) relate? Which one is the source of truth at runtime, and when does the database migration rewrite the JSON?
- Which timeout actually applies to agent `bash`/`python` on the native contained path versus the compatibility path (60 vs 120 vs 3600 s)?
- Does the legacy loop pass a known `context_length` into `maybe_compact` for unknown local models, or does it fall back to the 128k default? (The call at `src/agent_loop.py:25867` was not traced in full.)
- How often is the compact-preview loop (`clean_agent_preview`) selected in practice compared with the legacy loop? `runtime_selection.py` was not read.
- Is Tier-2 LLM self-evaluation (`evaluate_turn_llm`, `src/teacher_escalation.py:446`) active, given that the docstring says TODO?
- Do imported Hermes/Claude skills (no `status`/`confidence`) ever become injectable without passing an audit, i.e. what defaults does the importer stamp?
- Exactly which provider adapters (Bedrock, Azure, Cohere, Gemini native) have dedicated code, as opposed to docs only? `_detect_provider` has no `bedrock`/`azure`/`google` branch (`src/llm_core.py:1069-1108`); the Gemini catalog is handled in `routes/model_routes.py` according to `specs/llm-models.md:75`.
- Nothing was run, so every runtime claim, including the spec claim of "78 passing tests" (`specs/shell-mcp.md:145`), is unverified here.
