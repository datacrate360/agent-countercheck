# OpenClaw — A: core runtime (D3 agent loop · D4 model/provider layer · D5 prompt/context/compaction)

- **Repo / commit:** `openclaw` shallow clone, commit `15ae101a` (Sun 18 Oct 2026 13:39 UTC), TypeScript monorepo.
- **Method:** static reading only (rg/grep/sed/read). I did not build, install or run anything. About 80 source/doc files read in part or in full (≈45 TS source files in `packages/agent-core`, `packages/ai`, `packages/llm-core`, `packages/tool-call-repair`, `src/agents/**`, `src/context-engine`, `extensions/{codex,anthropic,acpx}`, plus about 12 docs/guide files).
- **Tags:** [Verified] = I read it in code at the cited line · [Docs] = taken from repo docs only · [Inference] = my own reasoning.
- All paths are relative to the repo root.

---

## Layer map (orientation)

```
channel / CLI / Gateway RPC  ──>  agentCommand (src/agents/agent-command.ts:604)
   └─> runEmbeddedAgent (src/agents/embedded-agent-runner/run-orchestrator.ts:109)
        ├─ session lane + global lane queueing (run-orchestrator.ts:163-236)
        ├─ CLI-backend short-circuit (claude-cli / gemini-cli)   run-orchestrator.ts:256
        └─ runWithModelFallback  (run-entry.ts:245 → model-fallback-runner.ts:138)    ← MODEL fallback chain
             └─ runPreparedEmbeddedLoop  (run-loop.ts:68)                               ← OUTER "attempt/retry" loop
                  └─ runAgentHarnessAttempt (harness/selection.ts:205)                  ← HARNESS choice
                       ├─ built-in "openclaw" harness → runEmbeddedAttempt (run/attempt.ts:66)
                       │     └─ @openclaw/agent-core runAgentLoop (packages/agent-core/src/agent-loop.ts:90) ← INNER turn loop
                       │           └─ streamAgentResponse (agent-stream-response.ts:130) → @openclaw/ai transports
                       └─ plugin harness: codex (Codex app-server), copilot, muse, agentsapi, acpx(opencode/qwen/pi/kilocode/copilot)
```
[Verified: the files and lines listed above]

There are four nested control layers: lane queue → model-fallback chain → outer retry/recovery loop → inner turn loop (model call → tools → model call ...). Each layer has its own budgets. [Inference]

---

## D3 — Agent loop

### In plain words
OpenClaw's "brain loop" has two levels. The inner loop asks the model, runs any tools the model asked for (in parallel by default), and repeats until the model stops asking for tools. The outer loop wraps each inner run as an "attempt". When something goes wrong (rate limit, empty answer, context too long, timeout), it compacts, retries or rotates credentials, and starts a new attempt from the saved transcript. The inner loop has no fixed limit on the number of turns. Runaways are stopped by loop detectors, a 48-hour time budget and retry ceilings.

### Where it lives
- Inner loop: `runAgentLoop` → `runLoop`. The nested `while (true)` / `while (hasMoreToolCalls || pendingMessages.length > 0)` loops are at `packages/agent-core/src/agent-loop.ts:90,151,229-232`. [Verified]
- `Agent` class wrapper with `prompt()`, `continue()`, `steer()`, `followUp()` and `abort()`: `packages/agent-core/src/agent.ts:410,425,456,481,498`. A second concurrent `prompt()` throws "Agent is already processing a prompt. Use steer() or followUp()..." (`packages/agent-core/src/agent.ts:489`). [Verified]
- Per-model-call streaming and tool dispatch: `streamAgentResponse` at `packages/agent-core/src/agent-stream-response.ts:130`. [Verified]
- Outer attempt loop: `runPreparedEmbeddedLoop` at `src/agents/embedded-agent-runner/run-loop.ts:68`, with `while (true)` at `src/agents/embedded-agent-runner/run-loop.ts:233`. [Verified]
- Single attempt (prompt build, tools, stream wrappers, then the agent-core loop): `src/agents/embedded-agent-runner/run/attempt.ts:66`. The `run/` directory alone has **423 files** (`ls run | wc -l`). [Verified]
- Docs state the runtime was internalized: "No external agent framework packages remain" (`docs/agent-runtime-architecture.md` "Boundaries"). The legacy runtime alias `pi` normalizes to `openclaw` (same doc, "Runtime Selection"). [Docs] This fits the code: agent-core is in-repo. A comment says the repetition guard was "Adapted from Hermes Agent" (`packages/agent-core/src/repetition-guard.ts:1`). [Verified]

### Turn structure (inner loop)
1. The loop emits `agent_start` and `turn_start`. It pushes each prompt and emits `message_start`/`message_end`. Messages cancelled while queued are dropped. If every input was cancelled, the loop ends with **no provider call** (`packages/agent-core/src/agent-loop.ts:103-124`). [Verified]
2. Each iteration does the following. It checks abort. It commits pending steering messages. It optionally runs `prepareContinuation`, which can swap the system prompt and tools. It drains queued "system update" context. Then it calls `streamAgentResponse` (`packages/agent-core/src/agent-loop.ts:233-310`). [Verified]
3. `streamAgentResponse` runs `transformContext` (pruning/injection hook) and then `convertToLlm`. It resolves the API key **per call** for expiring OAuth tokens and opens the provider stream (`packages/agent-core/src/agent-stream-response.ts:152-172,270-276`; contract at `packages/agent-core/src/types.ts:223,232`). [Verified]
4. Any tool calls the stream did not already execute run as a "terminal batch" (`packages/agent-core/src/agent-loop.ts:313-336`). [Verified]
5. Whether to continue is decided at `packages/agent-core/src/agent-loop.ts:343-348`. The loop continues if any of these holds:
   - the stream asked for continuation (mid-stream steering);
   - the provider said `stopReason:"stop"` but `endTurn === false` (OpenAI Responses `end_turn:false`);
   - a tool batch ran and did not ask to terminate.

   [Verified]
6. After `turn_end`, two hooks run. `prepareNextTurn` may replace context, model or thinking level for the next request in the same run (`packages/agent-core/src/agent-loop.ts:404-427`). `shouldStopAfterTurn` may end gracefully (`packages/agent-core/src/agent-loop.ts:432-444`). [Verified]
7. When no tool calls or steering remain, the loop asks `getFollowUpMessages()` and then re-checks steering, so an accepted steer cannot be stranded. Only then does it `break` and emit `agent_end` (`packages/agent-core/src/agent-loop.ts:460-471`). [Verified]

### Stop conditions (how a turn/run ends)
- **Natural end:** no tool calls, no steering and no follow-ups (`packages/agent-core/src/agent-loop.ts:460-468`). [Verified]
- **Abort signal:** `stopIfAborted()` runs at about 7 checkpoints. It persists a synthetic aborted assistant message. Per the code comment, this is "so session post-processing does not compact or continue from the preceding toolUse message". It then appends an "interrupted turn" message, except on handoff aborts (`packages/agent-core/src/agent-loop.ts:174-202`). [Verified]
- **Provider error/aborted stopReason:** emits `agent_end` immediately. The outer loop then classifies the result (`packages/agent-core/src/agent-loop.ts:399-402`). [Verified]
- **Tool-loop intervention (critical):** the loop injects a fixed failure message, either `TOOL_LOOP_RECOVERY_TERMINATED_MESSAGE` ("OpenClaw stopped this run because tool-loop recovery encountered another critical loop...") or `REPEATED_TOOL_ERROR_MESSAGE` with code `openclaw_repeated_tool_error`. Then it ends (`packages/agent-core/src/agent-loop.ts:78-79,376-397`; `packages/agent-core/src/errors.ts:6-7`). [Verified]
- **Fatal tool-batch error:** rethrown (`packages/agent-core/src/agent-loop.ts:360-362`). [Verified]
- **Repetitive-output guard:** stops a degenerate stream. The thresholds are `FIRST_CHECK_CHARS=16_000`, `TAIL_CHARS=64_000`, `WINDOW_CHARS=60` and `MIN_REPEATS=5`. The offending text is replaced with `[Repetitive output stopped]` and `stopReason:"error"`, `errorCode: REPETITIVE_OUTPUT_ERROR_CODE` (`packages/agent-core/src/repetition-guard.ts:2-6`; `packages/agent-core/src/assistant-repetition-guard.ts:5-52`; it is wired through `config.repetitionGuard?.(model)`, a "host-resolved small-model repetition policy", at `packages/agent-core/src/types.ts:195-196` and `packages/agent-core/src/agent-stream-response.ts:282-284,352-354`). [Verified]
- **Message-tool final send:** "A confirmed final send, reply, thread reply, or poll to the current source conversation ends the run loop" (`docs/concepts/agent-loop.md` "Tool execution"). [Docs] The code hook is `completesToolTurn` (`packages/agent-core/src/types.ts` near `beforeToolBatch`, the "@internal OpenClaw-owned turn completion" comment). [Verified signature only]

### Iteration / time / token budgets
- **No max-turn or max-tool-call cap in the built-in loop.** I found no such constant in `packages/agent-core` or `src/agents`. `max_turns` appears only when parsing Claude CLI output (`src/agents/cli-output-records.ts:303-323`). [Verified by grep; absence claim]
- **Tool-loop detection** does the job a turn cap would do. The settings are:
  - `TOOL_CALL_HISTORY_SIZE=30`, `UNKNOWN_TOOL_THRESHOLD=10`, `CRITICAL_THRESHOLD=20`, `GLOBAL_CIRCUIT_BREAKER_THRESHOLD=30` (`src/agents/tool-loop-detection.ts:57-60`);
  - warning threshold `TOOL_LOOP_WARNING_THRESHOLD=10` (`src/agents/tool-loop-thresholds.ts:1`);
  - detector kinds: `generic_repeat`, `argument_churn`, `unknown_tool_repeat`, `known_poll_no_progress`, `global_circuit_breaker` and `ping_pong` (`src/agents/tool-loop-detection.ts:36-43`).

  Calls are hashed as sha256 of stable-stringified params. For `exec`, the presentation `title` is dropped first (`src/agents/tool-loop-detection.ts:72-79`). Config `tools.loopDetection.enabled` can be global or per agent (`src/agents/tool-loop-detection-config.ts:9-26`). Unknown-tool recovery stays on even when loop detection is disabled (`src/agents/embedded-agent-runner/run/attempt-stream.ts:327-330`). [Verified]
- **Post-compaction loop guard:** if the same loop persists after a compaction succeeds, the lane task is aborted (`src/agents/embedded-agent-runner/run-loop.ts:164-187`). [Verified]
- **Outer retry ceiling:** `resolveMaxRunRetryIterations = clamp(24 + max(1,profiles)*8, 32, 160)` (`src/agents/embedded-agent-runner/run/helpers.ts:52-63`). When it is exhausted, the run returns "Request failed after repeated internal retries..." or throws `FailoverError` so the next model is tried (`src/agents/embedded-agent-runner/run-loop.ts:250-288`; `src/agents/embedded-agent-runner/run/retry-limit.ts`). A successful model response resets the counted attempts to 1 (`src/agents/embedded-agent-runner/run/retry-budget.ts` `recordRunModelProgress`; called at `src/agents/embedded-agent-runner/run-loop.ts:411`). "Progress continuations" refund one count (`src/agents/embedded-agent-runner/run/retry-budget.ts` `recordRunRetry`). [Verified]
- **Same-model transient retry:** the constants are `MAX_TRANSIENT_RETRIES=8`, `MAX_OUTPUT_LIMIT_RETRIES=1`, `MAX_TRANSIENT_RETRY_TIME_MS=90_000`, base delay 1 s, max delay 30 s, exponential backoff with ×(0.5–1.5) jitter, honoring `Retry-After`, and `MAX_RATE_LIMIT_ATTEMPTS=10` (`src/agents/embedded-agent-runner/run/failover-retry-controller.ts:26-60,371-410`). [Verified]
- **Incomplete-turn retries:**
  - `DEFAULT_REASONING_ONLY_RETRY_LIMIT=2`, `DEFAULT_EMPTY_RESPONSE_RETRY_LIMIT=1` (`src/agents/embedded-agent-runner/run/incomplete-turn-recovery.ts:35-36`);
  - `MAX_MISSING_ASSISTANT_RETRIES=1` (`src/agents/embedded-agent-runner/run/terminal-resolution.ts:77`);
  - `MAX_EMPTY_ERROR_RETRIES=3` (`src/agents/embedded-agent-runner/run/assistant-failure.ts:52`);
  - context overflow: `MAX_OVERFLOW_COMPACTION_ATTEMPTS=3` (`src/agents/agent-compaction-constants.ts:16`; used in `src/agents/embedded-agent-runner/run/overflow-context-recovery.ts:256`).

  [Verified]
- **Time budgets:**
  - Agent per-attempt execution budget: `DEFAULT_AGENT_TIMEOUT_SECONDS = 48*60*60` (172 800 s) (`src/agents/timeout.ts:13`). Each fallback model gets a fresh budget. `0` means unlimited (`docs/concepts/agent-loop.md` "Timeouts"). [Verified constant; Docs semantics]
  - LLM idle watchdog: cloud `120_000` ms, self-hosted `300_000` ms, local first-event `300_000` ms, cron `60_000` ms. Self-hosted prefixes are `ollama, lmstudio, vllm, sglang, llama-cpp`, and loopback/`.local` hostnames count as local (`src/agents/embedded-agent-runner/run/llm-idle-timeout.ts:28-60`). [Verified]
  - Embedded compaction timeout `180_000` ms (`src/agents/embedded-agent-runner/compaction-safety-timeout.ts:9`). [Verified]
  - `agent.wait` defaults to 30 s and is wait-only; it does not cancel the run. Approval waits pause the attempt budget (`docs/concepts/agent-loop.md` "Timeouts"). [Docs]
- **Token budgets:** handled by compaction/pruning (see D5), not by the loop itself. [Verified by structure]

### Streaming
- Provider events (`text_*`, `thinking_*`, `toolcall_*`, `done`, `error`) become `message_update` events on the agent event stream (`packages/agent-core/src/agent-stream-response.ts:311-415`). [Verified]
- The Gateway bridges these to the `assistant`, `tool` and `lifecycle` streams (`phase: start|finishing|end|error`) via `subscribeEmbeddedAgentSession`. The chat layer buffers deltas and gives retryable errors a 15 s grace window (`docs/concepts/agent-loop.md` "Run sequence", "Chat channel handling"). [Docs]
- **Streaming tool execution (distinctive).** A tool call marked `async: true` is executed **while the model is still streaming**. On `toolcall_end` for an async call, the completed prefix is committed to the transcript ("Await transcript persistence before admitting side effects") and the call is enqueued (`packages/agent-core/src/agent-stream-response.ts:374-405`). Batches chain: admission is serialized, but bodies may overlap (`packages/agent-core/src/agent-stream-response.ts:226-268`). [Verified]
  - Today only the OpenAI Responses transport sets `async:true`, and only for `params.model === "gpt-6-astra"` without `multi_agent`. Every function tool is async unless the tool declares `async === false` (`packages/ai/src/transports/openai-responses-client.ts:256-270`; stream side at `packages/ai/src/transports/openai-responses-stream-internal.ts:236-238`). [Verified]
- Serialized XML/GLM tool-call text is hidden from streamed and final replies (`docs/concepts/agent-loop.md` "Streaming"). [Docs] The code is in `packages/tool-call-repair/src/stream-normalizer*.ts` (see D4). [Verified file existence and roles]

### Interrupts / steering
- **Between turns:** `getSteeringMessages()` is polled at checkpoints. With sequential tools it is checked before each later call; when steering arrives, the remaining unstarted calls are skipped with "Skipped to process an incoming message." Parallel batches never skip (`packages/agent-core/src/agent-loop.ts:80,599-607`; contract at `packages/agent-core/src/types.ts:260-271`). [Verified]
- **Mid-response steering (distinctive):** `createStreamSteering` forwards queued user input into the **active provider response** through `control.steer(userMessages)` (`packages/agent-core/src/stream-steering.ts:5-100`). It does this only if the projected context is a pure append, i.e. the prefix is byte-identical (`packages/agent-core/src/agent-stream-response.ts:190-199`). Otherwise the input waits for an ordinary new request. [Verified]
- **Queue modes per channel:** `steer` (default), `followup`, `collect` and `interrupt`; 500 ms debounce, `cap: 20`, `drop: "summarize"` (`docs/concepts/queue.md` "Defaults", "Queue modes"). The choices list is in code at `src/auto-reply/commands-registry.shared.ts:574`. [Docs + Verified choices list]
- **Codex harness steering:** a batched `turn/steer` JSON-RPC to the Codex app-server (`extensions/codex/src/app-server/attempt-steering.ts:338`), and interrupt via `turn/interrupt` (`extensions/codex/src/app-server/attempt-client-cleanup.ts:109`). [Verified]
- **Concurrency:** runs are serialized per session lane plus a global lane (`src/agents/embedded-agent-runner/run-orchestrator.ts:163-236`). The docs say the `main` lane is `max(8, CPU*4)` and subagents default to 8 per spawning session (`docs/concepts/queue.md`). There is also a durable `activeWriterRunId` fence on transcript writes (`docs/concepts/agent-loop.md` "Queueing and concurrency"). [Verified lanes; Docs numbers]

### Parallel tool calls
- The default is `"parallel"` (`packages/agent-core/src/agent.ts:359`; `packages/agent-core/src/types.ts:287-288`). A batch becomes sequential if config says so **or if any tool declares `executionMode:"sequential"`** (`packages/agent-core/src/agent-loop.ts:535-547`). Tools that declare it include `computer`, `mobile-ui` and node-plugin network tools (`src/agents/tools/computer-tool.ts:202`, `src/agents/tools/mobile-ui-tool.ts:414`, `src/agents/node-plugin-tools.ts:128`). [Verified]
- Parallel groups prepare together, launch via `launchParallelToolCalls` and **commit results in source order** (`packages/agent-core/src/agent-loop.ts:611-669,872`). [Verified]
- Hooks run in this order: arguments validated against the tool schema (`validateToolArguments`, `packages/agent-core/src/agent-loop.ts:1053`), `beforeToolBatch` (internal admission and loop intervention, `packages/agent-core/src/agent-loop.ts:507-533`), `beforeToolCall` (can `{block:true}`, `packages/agent-core/src/agent-loop.ts:997`), then `afterToolCall` (`packages/agent-core/src/agent-loop.ts:1240`). [Verified]

### Retries and error handling
- **Tool errors are not fatal.** A thrown tool error becomes an `isError` tool result returned to the model (`packages/agent-core/src/agent-loop.ts:1150-1166`). [Verified]
- **Provider failures:** classified into the 16 frozen `FAILOVER_REASONS`: `auth, auth_permanent, format, rate_limit, overloaded, billing, server_error, timeout, tls_certificate, context_overflow, model_not_found, session_expired, empty_response, no_error_details, unclassified, unknown` (`packages/gateway-protocol/src/failover-reasons.ts:1-18`). The large classifier corpus is in `src/agents/failover/`. [Verified]
- **Retry ownership** is written down in a comment: per-attempt LLM retry is `src/llm/utils/retry.ts`; per-run rotation is `src/agents/embedded-agent-runner/run/retry-budget.ts`; the cross-run probe throttle is `src/agents/model-fallback-cooldown.ts`; persisted auth cooldowns are `src/agents/auth-profiles/usage.ts` (`src/agents/failover/signal.ts:3-7`). [Verified]
- **Recovery continues from the saved transcript.** It does not replay the request. The docs say "The agent is instructed to inspect interrupted actions before deciding whether to repeat them" (`docs/concepts/model-failover.md`). [Docs]
- **Malformed tool call rejected by the provider:** allowed one continuation, only if all earlier failures carry "host evidence that execution never started" (`docs/concepts/agent-loop.md` "Run sequence"). [Docs]

### How a turn ends
- Final payloads are assembled from assistant text, optional reasoning, inline tool summaries and error text. `NO_REPLY` means intentional silence. An empty final is **not** silence: when bounded recovery is exhausted, the run records failure and posts a one-line system notice instead of inventing an answer (`docs/concepts/agent-loop.md` "Reply shaping"). [Docs]
- **Tool-free finalization pass:** if a required-reply turn ends after a settled tool batch with no composed answer, the harness's `finalizeSettledTurn` produces "one final answer from a settled tool transcript without exposing capabilities that can repeat or extend the completed work" (`src/agents/harness/types.ts:592-598`). The built-in implementation is at `src/agents/harness/builtin-openclaw.ts:97-`. [Verified]
- The lifecycle `end`/`error` is published once; later duplicate terminals are suppressed (`docs/concepts/agent-loop.md` "Event streams"). [Docs]

---

## D4 — Model / provider layer

### In plain words
OpenClaw speaks about 10 wire protocols itself (OpenAI chat/responses, Anthropic, Google, Bedrock, Mistral, Azure). Around 80 provider IDs (OpenRouter, Groq, Ollama, LM Studio, vLLM ...) come as plugins that reuse those protocols. If a model fails, OpenClaw first retries it, then rotates API keys/accounts, then moves down a configured fallback list.

A "harness plugin" is a swappable **engine for the whole agent turn**. Selecting the `codex` harness hands the full turn to OpenAI's Codex app-server process, which runs its own loop. OpenClaw keeps the transcript, channels and tools, and Codex calls back into OpenClaw for OpenClaw's tools. Claude Code is **not** a harness plugin. It is a "CLI backend" that runs `claude -p` as a subprocess, and OpenClaw tools are exposed to it over MCP.

### Providers supported
- **Built-in wire APIs** (`KnownApi`): `openai-completions, mistral-conversations, openai-responses, azure-openai-responses, openai-chatgpt-responses, anthropic-messages, bedrock-converse-stream, google-generative-ai, google-interactions, google-vertex`. The comment calls them "Provider API families with first-class request/stream adapters". Custom string IDs are allowed (`packages/llm-core/src/types.ts:15-30`). Adapters load lazily on first use (`packages/ai/src/providers/register-builtins.ts:53-90`). [Verified]
- **Provider plugins:** I parsed `extensions/*/openclaw.plugin.json`. 63 extensions declare `providers`, and together they cover about 85 provider IDs. Examples: `anthropic, anthropic-vertex, amazon-bedrock, openai, google, google-vertex, groq, cerebras, deepseek, mistral, openrouter, together, fireworks, xai, moonshot/kimi, qwen/dashscope, zai, minimax, nvidia, huggingface, litellm, vercel-ai-gateway, cloudflare-ai-gateway, github-copilot, ollama, ollama-cloud, lmstudio, llama-cpp, vllm, sglang, omlx, onnx, apple-fm`. [Verified via manifest parse]
- **Local models:** these are ordinary provider plugins (`ollama`, `lmstudio`, `llama-cpp`, `vllm`, `sglang`, `omlx`, `onnx`, `apple-fm`). They differ from cloud providers in a few places:
  - longer idle/first-event timeouts (`src/agents/embedded-agent-runner/run/llm-idle-timeout.ts:28-36`);
  - a llama.cpp GBNF schema cleaner (`packages/ai/src/providers/clean-for-llamacpp-gbnf.ts`);
  - Gemma/DeepSeek DSML text tool-call grammars in the completions transport (`packages/ai/src/transports/openai-completions-gemma.ts`, `packages/ai/src/transports/openai-completions-dsml.ts`, `packages/ai/src/transports/deepseek-dsml-grammar.ts`).

  [Verified file existence/roles; internals not fully read]
- **Default model** when nothing is configured: `DEFAULT_PROVIDER = "openai"`, `DEFAULT_MODEL = "gpt-6-astra"`, `DEFAULT_CONTEXT_TOKENS = 200_000` (`src/agents/defaults.ts:5-8`). [Verified]

### How a model is chosen
- The order is: per-run/session override → agent model → `agents.defaults.model`. The candidate chain then appends `agents.defaults.model.fallbacks` (or `fallbacksOverride`) and a utility model, cached by a key over all those inputs (`src/agents/model-fallback-candidates.ts:197-260`). A user session model override **disables configured fallbacks**, with a logged warning (`src/agents/embedded-agent-runner/run-orchestrator.ts:451-467`). [Verified]
- The operator model policy can filter candidates (`src/agents/model-fallback-runner.ts:172-178`). [Verified]
- Prepared "model runtime generations": a per-agent atomic snapshot of auth, registry and catalog, built at Gateway startup or on config change. Runs fork from it (`docs/agent-runtime-architecture.md` "Model Runtime Generations"). The code side is `acquireAgentRunPreparedModelRuntime` (`src/agents/embedded-agent-runner/run-orchestrator.ts:342-371`). [Docs + Verified call site]

### Fallbacks / routing
- There are two stages: (1) **auth-profile rotation** within the provider, with persisted cooldowns, then (2) **model fallback** to the next candidate (`docs/concepts/model-failover.md`). Implementation: `runWithModelFallback` (`src/agents/model-fallback-runner.ts:138-220`) wraps the run at `src/agents/embedded-agent-runner/run-entry.ts:245`. [Verified]
- **Side-effect safety:** fallback is refused once a block reply was already delivered to the channel or a side effect committed (`canFallback`, `src/agents/embedded-agent-runner/run-entry.ts:226-238`). [Verified]
- A process-local, per-account rate-limit admission gate is shared across calls. Interactive work has priority. Queued requests resume with 100–250 ms jitter (`docs/concepts/model-failover.md`). [Docs]
- Codex "cyber" refusal escalation: a refused Codex attempt is retried once on a configured escalation model, with a per-workspace cool-off on denial (`extensions/codex/harness.ts:322-400`). [Verified]

### Tool-call parsing for models without (reliable) native tool calling
This is applied by stream wrappers that are installed on every attempt (`src/agents/embedded-agent-runner/run/attempt-stream.ts:320-360`). [Verified]
1. `wrapStreamFnSanitizeMalformedToolCalls` runs with the replay-allowed tool names (`src/agents/embedded-agent-runner/run/attempt-stream.ts:320-325`).
2. **`wrapStreamFnPromoteStandaloneTextToolCalls`** turns plain-text tool calls into real `toolCall` blocks (JSON, XML `<tool_call>`/`<function=…>`, bracketed `[TOOL]`, Harmony `<|channel|>…<|call|>`, ReAct `Action:`). It matches only *allowed tool names* and skips anything inside Markdown code regions (`findCodeRegions`). Promotion happens only on stop reasons `stop`/`toolUse` (`src/agents/embedded-agent-runner/run/attempt-tool-call-text-promotion.ts:22-150`). The grammar is in `packages/tool-call-repair/src/grammar.ts:1-8` (Harmony and `[END_TOOL_REQUEST]` markers) and `packages/tool-call-repair/src/attempt.ts:7-15` (detector kinds `json_tool_call | xml_tool_call | bracketed_tool_call | react_action`; caps `MAX_SCAN_CHARS=20_000`, `MAX_JSON_CANDIDATES=20`). [Verified]
3. `wrapStreamFnTrimToolCallNames` normalizes near-miss tool names (`src/agents/embedded-agent-runner/run/attempt-stream.ts:327-330`; `src/agents/embedded-agent-runner/run/attempt-tool-call-name-resolution.ts`). [Verified]
4. **Argument repair** for malformed JSON args covers smart quotes, leading prose and trailing junk, with `MAX_TOOLCALL_REPAIR_BUFFER_CHARS=64_000`, at most 96 leading and 3 trailing chars. It is enabled only for `openai-completions`, `azure-openai-responses`, `openai-chatgpt-responses`, and Kimi on `anthropic-messages` (`src/agents/embedded-agent-runner/run/attempt.tool-call-argument-repair.ts:12-19,571-582`; installed at `src/agents/embedded-agent-runner/run/attempt-stream.ts:332-339`). [Verified]
5. HTML-entity decoding of arguments for models flagged `html-entities` (`src/agents/embedded-agent-runner/run/attempt-stream.ts:341-343`). [Verified]
6. Schema-side argument recovery keeps `anyOf`/`oneOf` values that already match. For example, `"00123"` stays a string (`docs/concepts/agent-loop.md` "Tool execution"). [Docs]

### Prompt caching handling
- **AGENTS.md rule:** "Prompt prefix: within a session, tool definitions, system prompt and earlier history depend only on session and config facts; history only appends." A dedicated test guards it: `src/agents/agent-tools.prefix-stability.test.ts` (`AGENTS.md` "Runtime and code safeguards"). [Verified rule text]
- **System prompt split:** stable prefix + `SYSTEM_PROMPT_CACHE_BOUNDARY` (`"\n<!-- OPENCLAW_CACHE_BOUNDARY -->\n"`) + dynamic suffix (`packages/ai/src/utils/system-prompt-cache-boundary.ts:3`; `src/agents/system-prompt.ts:647`; `src/agents/embedded-agent-runner/run/attempt-system-prompt.ts:64`). The stable prefix is memoized by a sha256 key over its inputs, in an LRU (`src/agents/system-prompt.ts:106-118,593-607`). The Anthropic and OpenAI-completions cache-control transports consume the boundary (`packages/ai/src/transports/anthropic-payload-policy.ts`, `packages/ai/src/transports/openai-completions-cache-control.ts`). [Verified]
- **`cacheRetention`:** `none | short | long`, default `short`. Env `OPENCLAW_CACHE_RETENTION=long` upgrades it (`packages/ai/src/providers/cache-retention.ts`). On Anthropic, `long` gives `{type:"ephemeral", ttl:"1h"}`. With implicit/env-driven long retention, the 1h TTL is granted only on `api.anthropic.com` or Vertex aiplatform hosts (`packages/ai/src/transports/anthropic-payload-policy.ts:156-188`). [Verified]
- Up to two conversation checkpoints plus a separate tool-prefix checkpoint (`docs/reference/prompt-caching.md` "Anthropic"). [Docs]
- **OpenAI `prompt_cache_key`:** `<sessionId>:<compactionBoundaryCount>`, truncated to OpenAI's 64-code-point limit. The key changes only when compaction or reset changes the boundary (`src/agents/embedded-agent-runner/run/session-boundary-prompt-cache-key.ts:25-41`). [Verified]
- Google explicit context caching: `src/agents/embedded-agent-runner/google-prompt-cache.ts`. [Verified file; not read in depth]
- Tool-result caps are applied identically before the first request and when the result is persisted, so replays stay byte-stable (`docs/reference/prompt-caching.md` "Keep replayed tool results stable"). [Docs]
- Heartbeat keep-warm (e.g. `every: "55m"`) keeps caches alive (`docs/reference/prompt-caching.md`). [Docs]

### "Harness as plugin": what it means exactly
- **Contract:** `AgentHarnessContract` (`src/agents/harness/types.ts:505-729`). The core method is `runAttempt(params): Promise<AgentHarnessAttemptResult>` (`src/agents/harness/types.ts:591`). A harness therefore replaces **one whole attempt**: the model loop, native tool calls and native streaming. Optional members:
  - `compact()` for native compaction, `finalizeSettledTurn()`, `runIsolatedCompletionV2()`, `runSideQuestion()`, `classify()`, `reset()`;
  - session fork, `loadModelCatalog()`, `loadMcpToolCatalog()`, `authBootstrap:"harness"`, `contextManagement:"native"`;
  - policy-certification flags such as `conversationToolPolicySupport:"exact"` and `nativeModelPolicySupport:"exact"`.

  [Verified]
- **What OpenClaw keeps around a harness:** lane queueing, admission and authority checks, model-fallback chain, outer retry/recovery loop, session transcript (the primary transcript, or a mirror for Codex), channel delivery, context-engine assembly that is "projected" into the native turn, and lifecycle/terminal publication. `runAgentHarnessAttempt` selects the harness and asserts model-policy support (`src/agents/harness/selection.ts:205-240`). It is called from inside the outer loop through `runEmbeddedAttemptWithBackend` (`src/agents/embedded-agent-runner/run/backend.ts:23-52`; `src/agents/embedded-agent-runner/run/run-attempt-dispatch.ts:690`). [Verified]
- **Built-in harness:** id `openclaw`. Its `runAttempt` lazily imports and calls `runEmbeddedAttempt`, which is OpenClaw's own loop (`src/agents/harness/builtin-openclaw.ts:85-96`). [Verified]
- **Codex harness (`extensions/codex`, 781 files in `src/app-server`):** id `codex`, auto-selectable for providers `codex` and `openai` (`extensions/codex/harness.ts:24,123-126`).
  - `supports()` rejects authored request-transport overrides and routes it cannot reproduce, and returns `fallbackRuntime:"openclaw"` (`extensions/codex/harness.ts:266-321`).
  - `runAttempt` → `runCodexAppServerAttempt`, which spawns `codex app-server --listen stdio://` (`extensions/codex/src/app-server/config-options.ts:309`). It drives the turn over JSON-RPC: `turn/start` (`extensions/codex/src/app-server/run-attempt-turn-request.ts:302`), `turn/steer` and `turn/interrupt`.
  - OpenClaw's own tools are offered to Codex as "dynamic tools". Codex calls back through `item/tool/call` server requests (`extensions/codex/src/app-server/run-attempt-server-requests.ts:154`; `extensions/codex/src/app-server/event-projector-tool-items.ts:86`). Codex-native shell and file tools are bridged through a "native hook relay" for approvals (`src/agents/harness/native-hook-relay*.ts`).
  - Native compaction goes through `harness.compact` → `maybeCompactCodexAppServerSession` (`extensions/codex/harness.ts:472-475`).
  - **So yes: with Codex, OpenClaw delegates the entire inner loop** to Codex's own agent. Per the docs, "Codex owns its native stream deadlines and network retries" and "native Codex owns provider liveness and the exact `turn/completed` outcome" (`docs/concepts/agent-loop.md`). [Verified + Docs]
- **Implicit Codex selection:** an OpenAI model is routed to `codex` automatically only on an exact official HTTPS Responses/ChatGPT route with no authored overrides. "A provider or model prefix alone never selects a harness" (`docs/agent-runtime-architecture.md` "Runtime Selection"; `src/agents/harness/policy.ts:22-62` calls `resolveOpenAIImplicitAgentRuntime`). Runtime IDs: unset or `default` resolves to `auto` (`src/agents/harness/policy.ts:56-62`). [Verified]
- **Other harness plugins:** `copilot` (GitHub Copilot CLI), `muse` (Meta models through the Muse Code CLI), `agentsapi` (OpenAI Agents API with hosted/self-hosted sessions), and `acpx`. `acpx` registers one ACP harness per native agent: `opencode, qwen, pi, kilocode, copilot` (`extensions/acpx/src/config-schema.ts:8`; `extensions/acpx/index.ts:24-40`; descriptions in each `openclaw.plugin.json`). [Verified]
- **Claude Code is a CLI backend, not an AgentHarness.** "`claude-cli` is not an embedded harness id and must not be passed to AgentHarness selection" (`docs/concepts/agent-runtimes.md`). [Docs] In code:
  - `buildAnthropicCliBackend` gives `id: "claude-cli"` (`extensions/anthropic/cli-constants.ts:3`).
  - Command: `claude -p --output-format stream-json --include-partial-messages --verbose --setting-sources user --allowedTools mcp__openclaw__* --disallowedTools ScheduleWakeup,CronCreate,Bash(run_in_background:true),Monitor` (`extensions/anthropic/cli-backend.ts:43-55,229-230`).
  - Session handling: `--resume {sessionId}`, `--fork-session`, `--resume-session-at`, `--session-id`. A live stdio session `claude-stdio`. Prompt via `--append-system-prompt-file`.
  - OpenClaw tools are exposed through a bundled MCP server named `openclaw`; `exec` and `process` stay host-owned (`extensions/anthropic/cli-backend.ts:181-190,227-248`; `src/agents/cli-runner/bundle-mcp-claude.ts:7-17`).
  - `ownsNativeCompaction: true` uses Claude's `/compact` and validates `compact_boundary` (`extensions/anthropic/cli-backend.ts:190-222`).
  - `subscriptionAuthDispatch: true` routes subscription-OAuth runs through the CLI so they are billed against the plan (`extensions/anthropic/cli-backend.ts:223-227`).
  - Dispatch happens **before** the embedded loop: `runEmbeddedAgentViaCliBackendIfEligible` (`src/agents/embedded-agent-runner/run-orchestrator.ts:256-261`; `src/agents/embedded-agent-runner/cli-backend-dispatch.ts:21-142`).
  - Gemini CLI is the same kind of backend (`extensions/google/index.ts:42`).

  [Verified]
- **Third path, subagent spawn:** "Claude Code, Gemini CLI, OpenCode, Cursor ... → ACP/acpx" through `sessions_spawn(runtime:"acp")`. The system prompt teaches this ("Do in claude code/cursor/gemini/opencode" = ACP intent, `src/agents/system-prompt.ts:686-690`). [Verified]
- **Bottom line:** "harness as plugin" is true in a precise sense. A pluggable object implements `runAttempt` and takes over the model loop, while OpenClaw keeps the session/channel/fallback/admission shell around it. The README phrase "(Claude, Codex, local models)" is loose. Codex is a true harness. Claude runs through a CLI-backend adapter, or through ACP as a subagent. Local models are providers that run on the built-in `openclaw` harness. [Inference from Verified code]

---

## D5 — Prompt / context assembly & compaction

### In plain words
Each turn, OpenClaw builds a long system prompt. It contains fixed rules (act now, be careful with config edits, how to use tools) and the user-editable workspace files `AGENTS.md`, `SOUL.md`, `IDENTITY.md`, `USER.md`, `BOOTSTRAP.md` and `MEMORY.md`, each capped in size. The prompt is split into a "stable" half that stays identical across turns, so the provider's cache can reuse it, and a "dynamic" half.

When the conversation gets too long, OpenClaw first trims old tool outputs. If that is not enough, it summarizes older history into a structured checkpoint, using guardrails that audit the summary's quality. A plug-in "context engine" slot lets someone replace this whole strategy.

### System prompt sources
- **Builder:** `buildAgentSystemPrompt` (`src/agents/system-prompt.ts:284`). Three modes: `none` (one line, "You are a personal assistant running inside OpenClaw." plus the model identity line), `minimal` (subagents) and `full` (`src/agents/system-prompt.ts:354-362,424`). [Verified]
- **Stable prefix, in order** (`src/agents/system-prompt.ts:607-649`):
  1. identity line;
  2. provider `interaction_style` override;
  3. "## Execution Bias" with 8 terse rules, e.g. "Actionable request: act now." and "Final claim needs evidence or named blocker." (`src/agents/system-prompt.ts:613-624`);
  4. promised-work section;
  5. provider stable prefix;
  6. "## Runtime Context" (per-turn facts arrive as a separate runtime-context message, not by editing the system prompt);
  7. Workspace / Directory Roles;
  8. bootstrap sections;
  9. "## Workspace Files (injected)" → Project Context with the files;
  10. optional `<think>/<final>` reasoning-tag format for models that need it (`src/agents/system-prompt.ts:431-440`);
  11. then `SYSTEM_PROMPT_CACHE_BOUNDARY`.

  [Verified]
- **Dynamic suffix** (`src/agents/system-prompt.ts:651-962`): tooling section and tool list, workflow hints (subagent delegation, `sessions_spawn` context `isolated` vs `fork`, no busy-polling, media-generation de-dupe), "## Tool Call Style", "## Care" (inspect before config edits, credential safety), "## OpenClaw Control", Model Aliases, Sandbox, Silent Replies (`NO_REPLY`), Authorized Senders, Control UI, Voice/TTS, Reactions and "## Runtime". Skills catalog: `buildSkillsSection` is shown only when a `read`/`skills_read` tool exists (`src/agents/system-prompt.ts:512-519`). [Verified]
- **Bootstrap files** in prompt order: `AGENTS.md, SOUL.md, IDENTITY.md, USER.md, BOOTSTRAP.md, MEMORY.md` (`src/agents/workspace-bootstrap-policy.ts:10-36`). `TOOLS.md` is still defined as a constant (`:12`) but is **not** in `WORKSPACE_BOOTSTRAP_FILENAMES`, so it is apparently retired from injection. [Verified constant; Inference on retirement]
  - Subagents get only `AGENTS.md`; cron gets `AGENTS.md, SOUL.md, IDENTITY.md, USER.md` (`src/agents/workspace.ts:960-967`). [Verified]
  - Caps: `DEFAULT_BOOTSTRAP_MAX_CHARS = 20_000` per file, `DEFAULT_BOOTSTRAP_TOTAL_MAX_CHARS = 60_000` total, `USER_BOOTSTRAP_MAX_CHARS = 4_000` (`src/agents/embedded-agent-helpers/bootstrap.ts:86-90`), project-memory bootstrap `2_000` chars (`src/agents/project-memory-bootstrap.ts:23`). Truncation warnings are computed in `src/agents/bootstrap-budget.ts:127-164`. [Verified]
- Per-run extras: `extraSystemPrompt`, provider prompt contributions (`stablePrefix`, `dynamicSuffix`, `sectionOverrides`) (`src/agents/system-prompt.ts:415-423`). Plugin hooks can add sections (`src/agents/embedded-agent-runner/run/attempt-system-prompt-additions.ts`, not read in depth). [Verified partial]
- Agent-core prompt templates and skills are also loadable from package manifests (`openclaw.extensions/skills/prompts/themes`) (`docs/agent-runtime-architecture.md` "Manifests"). [Docs]

### Context engine
- **Pluggable interface** `ContextEngine` (`src/context-engine/types.ts:355-548`). Required members are `info`, `ingest`, `assemble({messages, tokenBudget, availableTools, model, prompt})` and `compact({tokenBudget, force, currentTokenCount, compactionTarget:"budget"|"threshold", abortSignal, onProgress})`. Optional members are `bootstrap`, `maintain` (transcript rewrites via `rewriteTranscriptEntries`), `ingestBatch`, `afterTurn` (proactive compaction) and `commitTurn` (idempotent advancement key). The runtime context exposes an `llm.complete` capability and prompt-cache telemetry (`src/context-engine/types.ts:320-347`). [Verified]
- **Selection:** plugin slot `plugins.slots.contextEngine`, default `"legacy"` (`src/plugins/slots.ts:19`; `src/context-engine/registry-selection.ts:12-28`). [Verified]
- **The default `LegacyContextEngine` is a pass-through.** `ingest` is a no-op. `assemble` returns the messages unchanged with `estimatedTokens: 0`. `compact` delegates to the built-in runtime compaction (`src/context-engine/legacy.ts:14-37`). The real assembly is the "sanitize → validate → limit → repair pipeline in attempt.ts" (comment at `src/context-engine/legacy.ts:28-29`). [Verified]
- **No bundled extension registers an alternative engine.** A grep for `registerContextEngine(` finds only the registry API (`src/plugins/registry-api.ts:145`). [Verified by grep] Harnesses declare the context-engine host capabilities they support; engines with requirements are rejected on harnesses that don't (`src/agents/harness/types.ts:527-531`). [Verified]

### Compaction / pruning strategy and thresholds
- **Agent-core base trigger:** `shouldCompact` fires when `contextTokens > contextWindow − reserveTokens`. Defaults are `reserveTokens: 16384` and `keepRecentTokens: 20000` (`packages/agent-core/src/harness/compaction/compaction.ts:163-167,260-269`). [Verified]
- **OpenClaw overrides:**
  - reserve floor `DEFAULT_AGENT_COMPACTION_RESERVE_TOKENS_FLOOR = 20_000` (`src/agents/agent-settings.ts:10`);
  - reserve capped at `MAX_COMPACTION_RESERVE_RATIO = 0.25` of the window, "so prompts retain at least three quarters of the model window" (`src/agents/agent-compaction-constants.ts:2-14`);
  - configurable `agents.defaults.compaction.{enabled, keepRecentTokens, mode}` (`src/agents/agent-settings.ts:22-60`).

  [Verified]
- **Preemptive (pre-prompt and mid-turn) precheck** chooses a route: `fits | truncate_tool_results_only | compact_then_truncate | compact_only`. Truncate-only is chosen only when reducible tool-result chars ≥ `max(overflow + 512 tokens·4, 1.5 × overflow)` (`src/agents/embedded-agent-runner/run/preemptive-compaction.ts:25,187-366`). A mid-turn precheck throws `MidTurnPrecheckSignal` before sending a request that would not fit (`src/agents/embedded-agent-runner/run/preemptive-compaction.ts:307-318`). [Verified]
- **Summarization** (`packages/agent-core/src/harness/compaction/`):
  - The summary is a structured checkpoint: Goal / Constraints / Done / In Progress / Blocked / Decisions / Next Steps / Critical Context. An UPDATE prompt merges into `<previous-summary>` and must "PRESERVE exact file paths, function names, and error messages" (`packages/agent-core/src/harness/compaction/compaction.ts:271-300`).
  - Summary capped at `MAX_COMPACTION_SUMMARY_CHARS = 16_000` (`packages/agent-core/src/harness/compaction/compaction.ts:57`).
  - Summarizer input capped at `MAX_SUMMARY_INPUT_CHARS = 160_000`, split as newest tail 50%, user messages 25% (each ≤2 000 chars), oldest head 10%, and 8 evenly spaced middle slices (≤6 000 chars each), with omission markers (`packages/agent-core/src/harness/compaction/utils.ts:392-406`).
  - Tool results inside the summary input are capped at 2 000 chars (`packages/agent-core/src/harness/compaction/utils.ts:147`).
  - A split-turn prefix gets its own prompt (`packages/agent-core/src/harness/compaction/compaction.ts:541`).
  - Cut points keep toolUse/toolResult pairs together (`packages/agent-core/src/harness/compaction/compaction-cut-point.ts`; `docs/concepts/compaction.md`).

  [Verified]
- **Safeguard mode** (the docs say it is the default for new configs): it preserves recent turns (`DEFAULT_RECENT_TURNS_PRESERVE=3`, max 12) and runs a quality guard (`DEFAULT_QUALITY_GUARD_MAX_RETRIES=1`, max 3) that audits structure, pending asks and exact identifiers. It degrades lossily, setting `details.qualityDegraded`, with a visible warning (`src/agents/agent-hooks/compaction-safeguard.ts:69-74`; `docs/concepts/compaction.md`). [Verified constants; Docs behaviour/default]
- **Provider-native compaction:** OpenAI `/responses/compact` and v2 `compaction_trigger` items (`packages/ai/src/transports/openai-responses-client.ts:271-274`), Anthropic server compaction with `ANTHROPIC_COMPACT_THRESHOLD_MIN = 50_000` (`packages/ai/src/transports/anthropic-payload-policy.ts:52`), and Codex/Claude native compaction via harness/CLI. Checkpoint replay is repaired if rejected (`src/agents/embedded-agent-runner/run/attempt-stream.ts:297-303`). [Verified]
- **Session pruning (`contextPruning.mode: "cache-ttl"`)** trims old tool results without rewriting the transcript. It stores a projection in an `openclaw.cache-ttl` marker.
  - Client-side settings: TTL default 5 min. Skipped below 30% window usage. Soft-trim results over 4 000 chars to head/tail 1 500 each. Hard-clear to `[Old tool result content cleared]` when usage is ≥50% and ≥50 000 prunable chars remain (`src/agents/embedded-agent-runner/tool-result-truncation.ts:54,70,166-172,238,271-276`).
  - Direct Anthropic API-key requests instead use **server-side `clear_tool_uses`** with trigger `max(50_000, 0.3·window)` and `clear_at_least` `max(12_500, 0.05·window)` (`packages/ai/src/transports/anthropic-payload-policy.ts:414-427`).
  - Never pruned: the last 3 assistant turns and anything before the first user message (`docs/concepts/session-pruning.md`).

  [Verified thresholds; Docs for the safety rules]

---

## Distinctive ideas (things a typical agent lacks)
1. **Streaming tool execution.** Async tool calls start *before the model finishes its response*. Each call's prefix is durably committed before the side effect, and results commit in order (`packages/agent-core/src/agent-stream-response.ts:374-405`). It is currently gated to `gpt-6-astra` on OpenAI Responses. [Verified]
2. **Mid-response steering.** New user input is injected into the *live* provider response, only when it is a pure append that keeps the cache prefix (`packages/agent-core/src/stream-steering.ts`; `packages/agent-core/src/agent-stream-response.ts:190-199`). [Verified]
3. **Swappable whole-turn engines (`AgentHarness.runAttempt`)** with a fail-closed policy certification model (`conversationToolPolicySupport:"exact"`, `nativeModelPolicySupport:"exact"`) and automatic fallback to the built-in runtime (`src/agents/harness/types.ts:505-591`; `extensions/codex/harness.ts:266-321`). [Verified]
4. **Claude Code as a subprocess backend**, with OpenClaw tools served to it over MCP and subscription billing kept on the Claude plan (`extensions/anthropic/cli-backend.ts`). [Verified]
5. **Prompt-prefix stability treated as an invariant:** stable/dynamic split marker, memoized stable prefix, cache key tied to the compaction boundary, a test that fails on prefix drift, and identical tool-result caps on first send and persist (`AGENTS.md`; `src/agents/system-prompt.ts:593-649`; `src/agents/embedded-agent-runner/run/session-boundary-prompt-cache-key.ts`). [Verified]
6. **Rich loop-pathology detectors instead of a turn cap:** ping-pong, argument churn, no-progress polling, unknown-tool streaks, a global circuit breaker, a post-compaction loop guard, and a repetitive-output stream guard (`src/agents/tool-loop-detection.ts`; `packages/agent-core/src/repetition-guard.ts`; `src/agents/embedded-agent-runner/run-loop.ts:164-187`). [Verified]
7. **Side-effect-aware failover:** no model fallback after a reply was delivered or a side effect committed. Recovery continues from the transcript rather than replaying (`src/agents/embedded-agent-runner/run-entry.ts:226-238`; `docs/concepts/model-failover.md`). [Verified + Docs]
8. **Robust text-tool-call promotion** for weak or local models: multiple grammars, code-region protection, an allowed-name matcher, and hiding of shadow XML in streamed output (`packages/tool-call-repair`; `src/agents/embedded-agent-runner/run/attempt-tool-call-text-promotion.ts`). [Verified]
9. **Compaction quality audits** with a structured checkpoint, identifier retention and a degraded-quality flag surfaced to the user (`src/agents/agent-hooks/compaction-safeguard.ts`; `docs/concepts/compaction.md`). [Verified constants + Docs]
10. **Tool-free finalization pass**, so a run never ends silently after tools have settled (`src/agents/harness/types.ts:592-598`). [Verified]
11. **Server-side context editing** (Anthropic `clear_tool_uses`) with size thresholds tied to the context window, falling back to local TTL pruning on other routes (`packages/ai/src/transports/anthropic-payload-policy.ts:414-427`). [Verified]

## Weaknesses / risks / complexity costs
- **Extreme complexity.** `src/agents` has 5 158 files. `embedded-agent-runner/run/` alone has 423 files. One attempt passes through about 15 stacked stream wrappers (`src/agents/embedded-agent-runner/run/attempt-stream.ts:240-360`), and there are four nested control layers, each with its own budget. The repo's own `AGENTS.md` warns that "machinery nobody can follow fails in ways nobody predicted". [Verified counts; Inference on risk]
- **No hard turn/step cap** in the built-in loop. Termination depends on heuristics plus a 48 h per-attempt default, so a slowly progressing but useless agent can burn tokens for a long time. Cost is bounded only indirectly (`src/agents/timeout.ts:13`; loop-detection thresholds). [Verified + Inference]
- **Numbers are spread over many files.** Retry constants live in at least 6 files. Docs and code mostly agree, but some facts are docs-only. [Inference]
- **The default context engine is a no-op wrapper.** Real assembly logic lives in the runner, and no alternative engine ships, so the "pluggable context engine" is a contract with no second implementation in-repo (`src/context-engine/legacy.ts`; grep). [Verified]
- **Streaming tool execution is model-gated** to one model ID string (`packages/ai/src/transports/openai-responses-client.ts:258`), so the most distinctive loop feature does not help other providers today. [Verified]
- **Harness delegation fragments behaviour.** Under Codex, retries, deadlines, tools and compaction semantics are Codex's own; OpenClaw mirrors them. Under Claude CLI, they are Claude's. Parity is documented as a support contract, not guaranteed (`docs/concepts/agent-runtimes.md`). [Docs + Inference]
- **Heuristic tool-call promotion** from plain text can produce false-positive executions. It is mitigated by allowed-name matching and code-region protection, not eliminated (`src/agents/embedded-agent-runner/run/attempt-tool-call-text-promotion.ts`). [Inference]
- **Dated, fast-moving contracts:** deprecated harness APIs with removal dates (`src/agents/harness/types.ts:558,599,653-656`) and model IDs hard-coded in transports. [Verified]

## Open questions (not determined)
- The exact order and conditions inside `prepareAndDispatchEmbeddedRunAttempt` and the terminal-resolution path, i.e. which recovery wins when several apply. I read the constants and entry points, not every branch of the about 420 `run/` files.
- How `transformContext` is wired by OpenClaw: which pruning, sanitize and limit steps run per call vs per attempt, and in what order. I identified the components (tool-result truncation, cache-ttl projection, replay sanitizers) but did not trace the full composition.
- Exact behaviour of the loop-detection "warning" level: whether it injects a message to the model or only logs. Messages are built at `src/agents/tool-loop-detection.ts:556-680`; I did not trace delivery.
- Whether `TOOLS.md` is fully retired or still loaded on some path. The constant is imported in `src/agents/workspace.ts:77`, but it is absent from the prompt-order list.
- Google prompt-cache (`src/agents/embedded-agent-runner/google-prompt-cache.ts`) and Bedrock caching specifics were not read.
- The docs say safeguard compaction is the default for *new configs*. I did not find where that default is written: config wizard vs runtime default.
- Codex app-server internals (781 files) were only sampled: harness definition, spawn args, `turn/start|steer|interrupt`, `item/tool/call`.
