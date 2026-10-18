# OpenUI — research report

| Field | Value |
|---|---|
| Repo | `thesysdev/openui` (shallow clone, 1 commit visible) |
| Commit | `8d9c4c3` (2026-10-16 23:28 +0530) |
| Local path | `~/.hermes/profiles/datacrate360/cache/scratch/agents/openui` (all paths below are relative to it) |
| License | MIT, "Copyright (c) 2011-2024 Thesys Inc." (`LICENSE:1-3`) |
| Analyst method | Static reading only. Nothing built or run. One exception: the shipped benchmark samples were re-tokenised with Python `tiktoken` 0.12 (`o200k_base`) to check the README numbers. |
| Rubric | `omni-agent/research/RUBRIC.md` section B, T1–T14 |

## Executive summary

- OpenUI is **a small line-based language (OpenUI Lang) plus a streaming parser, an evaluator and renderers** that let an LLM write UI as `name = Component(args)` statements. Thesys (the company behind C1) builds it. The core is `@openuidev/lang-core` 1.0.1, about 12k lines of TypeScript. There is a normative spec (`spec/language.md`) and 58 conformance fixtures.
- **The core algorithm is easy to reuse.** It removes code fences and comments, sets a watermark for completed statements, re-parses only the unfinished tail after closing open strings and brackets, then validates each statement against JSON Schema. When something is invalid it drops the smallest unit possible. The result is a valid render after every chunk (`packages/lang-core/src/parser/parser.ts:490-681`).
- **The system prompt is generated from Zod component schemas.** Each component becomes a positional signature line such as `Button(label, action?: ActionExpression, …) — description`. Fixed sections are added for syntax, built-ins, Query/Mutation, actions, streaming and edit mode (`packages/lang-core/src/parser/prompt.ts:621-728`).
- **Nothing in the language runs arbitrary code on the client.** The evaluator is a closed AST interpreter: no `eval`, no calls on member access, and only built-ins and host-registered functions can be called. Data fetching (`Query`/`Mutation`) only calls tools that the **host** provides (a function map or an MCP client).
- **The paid-cloud path is different.** "OpenUI Cloud" responses can carry generated JavaScript `scripts`. `@openuidev/server`'s `createToolExecutor` sends the whole response to `api.thesys.dev/v1/app/execute`. Thesys runs the script there and calls back **your** registered tools, up to 8 rounds and 16 calls (`packages/server/src/tools/execute.ts:13-185`).
- **What goes to Thesys:**
  - Autofix (the "Gateway") posts invalid generations plus up to 8,000 characters of chat history to `api.thesys.dev/v1/autofix`.
  - `observability-cloud` defaults to `capture:"full"` and sends the raw model response (up to 16 KB) to `ingest.thesys.dev`.
  - `lang-core` runs a **postinstall** script that sends **opt-out** install telemetry to PostHog through a CloudFront proxy, including a salted hash of your git origin.
  - In dev, `react-lang` auto-loads devtools from jsDelivr using a floating `@0` tag.
- **The "up to 67% fewer tokens than JSON" claim is weaker than it looks.** 67% is the single best scenario. The overall figure is 47–53%. The JSON baseline (C1) is pretty-printed with 2-space indent. **Against minified JSON, OpenUI Lang saves only 12.5% overall, and the dashboard case is 1.6% *larger*** (my re-count). The samples also use pre-1.0 syntax.
- **For omni-agent**, depend on `lang-core` and `react-lang` (MIT) to render sprint boards, burndowns, review forms and incident panels inside chat. Use our own component library and tool provider. Disable the telemetry (`OPENUI_TELEMETRY_DISABLED=1`). Do not use Cloud scripts, Autofix or observability-cloud. Patch the unsanitised `window.open` in `react-ui`, or avoid `react-ui`'s chat shell.

---

## Pipeline diagram

```text
 Zod components ──defineComponent()──► createLibrary() ──► library.prompt() ─────────► SYSTEM PROMPT ──► LLM
   (name, props, description)          (z.registry, $defs)   (signatures + rule sections)                    │ streams text
                                              │ toJSONSchema()                                                ▼
                                              ▼                                          ┌───────── chunk N ─────────┐
                                       compileSchema() → ParamMap ──────────────────────► createStreamParser.set()   │
                                                                                         │ preprocess: stripFences,  │
                                                                                         │   stripComments           │
                                                                                         │ scanNewCompleted (depth-0 │
                                                                                         │   newline watermark)      │
                                                                                         │ pending tail → autoClose  │
                                                                                         │ tokenize → split → Pratt  │
                                                                                         │ classify (query/mut/state │
                                                                                         │   /value) → buildResult   │
                                                                                         │ materialize + validate    │
                                                                                         │   (drop smallest unit)    │
                                                                                         └────────────┬──────────────┘
                                                                                                      ▼ ParseResult
       host toolProvider (fn map | MCP client) ◄── QueryManager (only after stream end) ◄── evaluate() (AST interpreter)
                    ▲                                                                                 │ ElementNode tree
                    │ @Run(mutation)                                                                  ▼
                    └──────────── useOpenUIState.handleAction ◄── user click ◄── React/Vue/Svelte/Angular renderer
                                         │ @ToAssistant → {type:"continue_conversation", humanFriendlyMessage, params.context, formState}
                                         │ @OpenUrl     → {type:"open_url", params.url}
                                         ▼ custom @Name → {type:"Name", params}
                                   host onAction  ──► next user turn: "msg\ncontext and form values = [...]"  ──► LLM
   Optional Thesys cloud: /v1/autofix (repair) · /v1/app/execute (run Cloud scripts) · ingest.thesys.dev (render events)
```

---

## T1 — Identity

In plain words: OpenUI is a monorepo from Thesys. It defines a compact UI language that models write, plus libraries that turn that text into live UI in React, Vue, Svelte and Angular. The license is MIT. The company sells a cloud product (OpenUI Cloud / C1 console) on top of it.

- Purpose: "The Open Standard for Generative UI". Its headline claim is "OpenUI Lang uses up to 67% fewer tokens than JSON" (`README.md:24`, `README.md:136`). [Docs]
- License: MIT, Thesys Inc. (`LICENSE:1-3`). Every package `package.json` declares `"license": "MIT"`, for example `packages/lang-core/package.json:5`. [Verified]
- Stack: TypeScript, pnpm workspace, tsdown builds, vitest, Zod v4 (`assertV4Schema`, `packages/lang-core/src/signature.ts:43`). [Verified]
- Size: 3,859 files excluding `.git`. TS/TSX lines per package: react-ui 73.2k, lang-core 12.2k, react-headless 10.7k, devtools 7.1k, openui-cli 6.3k, react-lang 3.7k, react-email 3.3k, langchain 2.7k, a2ui 2.5k, angular-lang 2.5k, observability-cloud 1.8k, vue-lang 1.7k, server 1.7k, svelte-lang 0.8k (terminal `find|wc` over `packages/*`). [Verified]
- Maturity: lang-core and react-lang are at **1.0.1**. react-ui and react-headless are 0.18.1, server 0.3.0, langchain 0.0.3, observability-cloud 0.0.3 (versions read from each `package.json`). The spec 1.0 was released on 2026-10-12, after a 0.9 review (2026-07-22) and 1.0-beta (`spec/language.md:646-647`). [Verified]
- Pinned commit: `8d9c4c3`. The clone is shallow, so `git log` shows 1 commit and history cannot be audited. [Verified]
- Company and upsell: OpenUI Cloud (`generateSystemPrompt({cloud:true})` emits a `]]>openui:config` block, `packages/lang-core/src/parser/cloud-config.ts:5`). Also the Autofix API, script execution, observability ingest, and the devtools banners that link to `console.thesys.dev/keys` and billing (`packages/devtools/src/ui/ReliabilityBanner.tsx:11`, `packages/devtools/src/inspect/QuotaErrorRow.tsx:67`). [Verified]
- The OpenClaw OS plugin lives in a **separate repo** (`thesysdev/openclaw-os`) and is only linked from `README.md:108`. It was not analysed. [Docs]

## T2 — Core mechanism (step by step)

In plain words: the model writes one statement per line, for example `root = Card([header])`. On every streamed chunk the parser keeps the statements it has already finished. It temporarily closes any open quotes or brackets in the last half-written line so that line still parses. It checks each statement against the component library and drops only the broken pieces. An interpreter then evaluates the tree, and the renderer shows whatever is valid so far.

### 2a. Grammar (with a real example)

- EBNF: `program = statement {newline statement}`, `statement = (identifier|state_name) "=" expression`. Precedence goes from ternary down through `|| && == comparison + * unary postfix`. Calls are `ComponentName(...)` or `@fn(...)`. Commas are optional (`spec/language.md:146-167`). [Docs]
- Sigils: `$name` = state variable and `@Name` = built-in, custom function or action (`spec/language.md:34-39`). Reserved names are `Query`, `Mutation`, `Action`, 14 built-ins (`@Count … @Each`) and 5 actions (`@Set @Reset @Run @ToAssistant @OpenUrl`) (`spec/language.md:47-53`). [Docs]
- Lexing never fails. Unknown characters are skipped (`spec/language.md:81`). The tokenizer is hand-written and treats newlines as significant tokens (`packages/lang-core/src/parser/lexer.ts:13-30`). [Verified]
- Arguments are **positional**, in Zod key order. The prompt tells the model that `name: value` syntax "silently breaks" (`packages/lang-core/src/parser/prompt.ts:135`). [Verified]
- Real conformance fixture `spec/fixtures/1.0/actions/007-mutation-run/input.oui`. It is Agile-relevant: it creates a ticket.
  ```text
  $title = "Bug"
  root = Stack([create])
  newTicket = Mutation("createTicket", {title: $title})
  create = Button("Create", Action([@Run(newTicket), @ToAssistant("Created")]))
  ```
  Its `interactions.json` expects the tool call `createTicket {title:"Bug"}` and then one host event `{type:"continue_conversation", humanFriendlyMessage:"Created"}`. [Verified]
- Streaming fixture `spec/fixtures/1.0/streaming/001-worked-example/chunks.json`. Its chunk boundaries split `Te|xt(` and `Stack([open|, closed])` on purpose to test pending-tail handling. [Verified]

### 2b. Streaming parser algorithm — `createStreamParser` (`packages/lang-core/src/parser/parser.ts:490-681`)

1. **`set(fullText)`** compares the new text with the buffer. If the new text is not an extension of the old one (it was replaced, not appended), it **resets** everything (`parser.ts:671-678`). [Verified]
2. **`refreshCleaned()`** re-runs `preprocess(buf)` on the whole buffer: `stripFences` then `stripComments` then trim (`parser.ts:447-449`, `519-527`). `stripFences` joins the contents of every ```` ``` ```` block and ignores backticks inside double-quoted strings. An unclosed fence runs to the end of the text (`parser.ts:319-410`). If the already-completed prefix is no longer a prefix (for example, a fence opener arrived late), the cache is discarded (`parser.ts:521-525`). This matches spec rule 4.4 "Reconciliation" (`spec/language.md:325`). [Verified]
3. **`scanNewCompleted()`** scans from the `completedEnd` watermark with a small state machine that tracks string and escape state, bracket depth and ternary depth. A newline at depth 0 is a statement boundary, unless the next non-blank character is `?`, or `:` while a ternary is open. Each completed slice is tokenised, split, Pratt-parsed and classified **once**, then stored in `completedStmtMap` (`parser.ts:502-512`, `529-587`). [Verified]
4. **Pending tail.** `autoClose(pendingText)` closes an unterminated string (adding `\` first if the text ends mid-escape), then pops the bracket stack in reverse order. A closer that does not match is ignored (`packages/lang-core/src/parser/statements.ts:18-59`). [Verified]
5. **Pending never overwrites completed.** If an auto-closed pending statement has the same id as a completed one, the pending one is skipped (`parser.ts:629`). [Verified]
6. **Partial `$name` guard.** A `$d` at the very end of the buffer might still grow into `$dishes`, so it is not auto-declared as state (`parser.ts:635-643`). [Verified]
7. **`buildResult`** chooses the entry statement: `root`, otherwise the first value statement that calls the library root component (`parser.ts:178-189`). It materialises from the entry, tracking `unresolved` and `orphaned` names, then collects state, query and mutation statements and auto-declares any `$vars` it finds (`parser.ts:105-168`, `210-295`). [Verified]
8. **Classification order:** `Query(...)` then `Mutation(...)` then `$x` then value. So `$x = Query()` counts as a query (`parser.ts:66-93`). [Verified]
9. Complexity: every chunk calls `preprocess` on the **whole** buffer and rescans from the watermark. Statement parsing is cached, but fence and comment stripping costs O(n) per chunk, so O(n²) per stream. This is fine for UI-sized outputs. [Inference]

### 2c. Error recovery ("drop and render")

- Spec rule: "A mistake removes the smallest possible unit, and everything else renders." The levels are prop, component, statement (`spec/language.md:472-474`). There are 14 error codes with stated recovery, and fatal vs non-fatal is defined (`spec/language.md:482-497`). [Docs]
- Code: an invalid value uses the schema default if there is one. Otherwise it returns `INVALID` (required, so the component is dropped) or `undefined` (optional, so the prop is omitted) (`packages/lang-core/src/parser/validation.ts:193-199`). [Verified]
- A line without `=` or without an identifier is skipped silently (`statements.ts:81-95`). [Verified]
- Render-time crashes are caught by `ElementErrorBoundary`, which keeps the `lastValidChildren` (`packages/react-lang/src/Renderer.tsx:96-127`). A parser exception becomes a `parse-exception` error instead of a crash (`packages/react-lang/src/hooks/useOpenUIState.ts:129-139`). [Verified]
- Errors use one wire shape, `{source, code, statementId, component, path, message, hint}`. It is designed to be pasted back to the model, with `hint` = the component signature (`spec/language.md:505-519`). [Docs]
- Batch and stream must agree: "At stream end, the result MUST equal a batch parse" (`spec/language.md:330`). This is tested by the conformance fixtures (`packages/lang-core/src/__tests__/conformance.test.ts:1`). [Verified]

### 2d. Prompt generation from a component library

1. `defineComponent({name, props: z.object, description, component})` tags the Zod schema with its name (`packages/lang-core/src/library.ts:77-91`). [Verified]
2. `createLibrary` registers each schema in a `z.registry` and adds the action `$defs`. It **throws** if a function or action name collides with a built-in or a component (`library.ts:246-289`). [Verified]
3. `buildComponentSpecs` → `schemaSignature` → `buildSignature` produces `Name(a: T, b?: U, [list]?)`, with key order = positional order. Component references print as their names, and bindable props print as `$binding<type>` (`packages/lang-core/src/signature.ts:225-251`, `library.ts:125-137`). [Verified]
4. `generatePrompt` assembles, in order: preamble ("Your ENTIRE response must be valid openui-lang code"), syntax rules, component signatures (grouped by `componentGroups`), built-in functions, Query/Mutation sections (only when `toolCalls`), the Action section (only if some prop uses `ActionExpression`), interactive filters, tool workflow, tools list, streaming rules, examples, edit mode, inline mode, important rules, `additionalRules` (`packages/lang-core/src/parser/prompt.ts:121`, `621-728`). [Verified]
5. Flags cascade: `toolCalls` defaults to true when tools exist, `bindings` defaults to `toolCalls`, and built-ins default to `toolCalls||bindings` (`prompt.ts:626-628`, `645`). [Verified]
6. `generateSystemPrompt({cloud:true})` emits **no prompt**. It emits only `]]>openui:config\n{json}`, and Thesys's server builds the real prompt (`prompt.ts:769-772`, `cloud-config.ts:28-58`). [Verified]
7. The benchmark's real system prompt is 13,194 bytes, which is **3,281 tokens** (`o200k_base`, my count of `benchmarks/system-prompt.txt`). That is a fixed per-request cost the token benchmark leaves out (see T9). [Verified]

### 2e. Runtime evaluation (what can execute)

- `evaluate()` is a `switch` over AST kinds: literals, `StateRef`, `Ref`, `Arr`, `Obj`, `Comp`, `BinOp`, `UnaryOp`, `Ternary`, `Member`, `Index`, `Assign` (`packages/lang-core/src/runtime/evaluator.ts:70-281`). [Verified]
- Only these things can be called: `BUILTINS[name]`, action steps, and host-registered `functions` and `actions` looked up with `hasOwnProperty` (`evaluator.ts:104-133`). A member access is never callable (`spec/language.md:126`). [Verified]
- There is no `eval`, `new Function` or `innerHTML` in non-test package sources (grep over `packages/` for `new Function|eval(|dangerouslySetInnerHTML|innerHTML` returned nothing). [Verified]
- Division by zero returns 0, and loose `==` is used (`evaluator.ts:211-218`). [Verified]
- `Member` reads `obj[field]` with no own-property check, so `x.constructor` returns a function value. It cannot be invoked, so I rate this as low risk (`evaluator.ts:251-260`). [Inference]

## T3 — Architecture and process model

In plain words: it is a layered set of npm packages. A framework-neutral core does the parsing and evaluation. Thin framework bindings sit on top, then optional UI kits, then optional server and cloud adapters. Everything runs inside the host app. There is no daemon.

- Layers:
  - `lang-core`: parser, prompt, validation, runtime evaluator, `QueryManager`, MCP glue (`packages/lang-core/src/index.ts`).
  - Framework bindings: `react-lang`, `vue-lang`, `svelte-lang`, `angular-lang`.
  - Headless chat state and stream adapters: `react-headless`.
  - Components: `react-ui` (73k lines, the reference library and chat shell), `react-email`.
  - Integrations: `assistant-ui`, `a2ui`, `langchain`.
  - Server: `@openuidev/server`.
  - Telemetry and devtools: `observability`, `observability-cloud`, `devtools`.
  - Scaffolder: `openui-cli`.
  [Verified]
- Process model: the parser runs in the browser (or on the server for Autofix). Queries run in the browser and call a host `toolProvider`, which is often a fetch to the host's own API or an MCP client (`packages/lang-core/src/runtime/queryManager.ts:16-25`). [Verified]
- React flow: `parseResponseBundle` strips Cloud markers, then `createStreamParser.set(program, !isStreaming)`, then evaluate, then render. Queries are blocked while `isStreaming` (`packages/react-lang/src/hooks/useOpenUIState.ts:91-92`, `130`). [Verified]
- Examples cover agent frameworks (google-adk, langgraph-platform, mastra, vercel-ai-sdk, vercel-eve) and harnesses (grok-build, pi) (`examples/agent-frameworks/`, `examples/harnesses/`). [Verified]

## T4 — Data model

In plain words: the "data" is plain text (the program), a JSON Schema for the library, and a small JSON tree after parsing. Stored chat messages wrap the program in marker lines such as `]]>openui:content`.

- `LibraryJSONSchema` = `z.toJSONSchema` of an object whose properties are the components, plus `$defs`, plus the extra keys `functions`/`actions` with descriptions merged in (`packages/lang-core/src/library.ts:360-377`). [Verified]
- `ParseResult = {root: ElementNode|null, rootExpression?, meta:{incomplete, unresolved, orphaned, statementCount, errors}, stateDeclarations, queryStatements, mutationStatements}` (`packages/lang-core/src/parser/parser.ts:281-294`). [Verified]
- State store: a flat `$name → value` map per rendered program. Re-parsing never overwrites values the user set and never deletes keys (`spec/language.md:346-351`; `@Reset` writes with `{pristine:true}`, `useOpenUIState.ts:421-426`). [Verified]
- Message protocol: the marker lines `]]>openui:content`, `]]>openui:context`, `]]>openui:end`, `]]>openui:config`, `]]>openui:scripts {"count":N}` followed by a JSON array of `{name, code}` (`packages/react-lang/src/responseBundle.ts:8-14`, `spec/prompt.md` §7). [Verified]
- Edit mode: the model returns only changed statements. `mergeStatements(existing, patch)` replaces statements by name, `name = null` deletes, and statements that become unreachable are GC'd (`packages/lang-core/src/parser/merge.ts:141`, `spec/language.md:459-466`). [Verified]
- Fixture layout: `spec/fixtures/1.0/<area>/<NNN-name>/{input.oui|chunks.json, expected.json, interactions.json}`. There are 8 areas and 58 `expected.json` files (`find`). [Verified]

## T5 — Interfaces (exact names)

In plain words: it is a TypeScript library. You define components, generate a prompt, render with a `<Renderer>`, and receive `onAction` callbacks. Optional server helpers wrap OpenAI and Vercel streams.

- lang-core:
  - `defineComponent`, `createLibrary`, `library.prompt/toSpec/toJSONSchema/extend`
  - `generateSystemPrompt`
  - `createParser(schema).parse`, `createStreamingParser(schema).push/set/getResult`, `compileSchema`
  - `mergeStatements`, `evaluate`, `createQueryManager`, `extractToolResult`, `ToolNotFoundError`, `McpToolError`
  - `artifactTool` (Cloud)

  (`packages/lang-core/src/library.ts:77,246,305-334`; `parser/parser.ts:749-772`; `runtime/mcp.ts:56`; `cloud.ts`) [Verified]
- react-lang: `<Renderer library response isStreaming toolProvider onAction …>`, where `onAction` is typed `LibraryActionEvent<L>`, a union discriminated by custom action names (`packages/react-lang/src/Renderer.tsx:41`, `packages/lang-core/src/library.ts:214-219`). [Verified]
- ToolProvider contract: `callTool(toolName, args) => Promise<unknown>`, **or** any MCP `Client` with `callTool({name, arguments})`. Results go through `extractToolResult`, which prefers `structuredContent` and falls back to JSON-parsing the text (`queryManager.ts:24-25`, `runtime/mcp.ts:56-86`). [Verified]
- server:
  - `createAutofix(...)` from `@openuidev/server/openai` and `/vercel`, exposing `.completions.fix()` and `.stream().toResponse()`
  - `createToolExecutor({tools, apiKey})`, exposing `{definitions, execute}`
  - LangGraph helper

  (`packages/server/README.md:9-60`, `src/shared/create-autofix.ts:16-43`, `src/tools/execute.ts:25`) [Verified]
- langchain: `openUIStreamTransformer()` (LangGraph `streamTransformers`), `streamOpenUI(...)`, `createLangChainStreamResponse(...)` (`packages/langchain/src/transformer.ts:46`, `stream-relay.ts:64`, `request-handler.ts:56`). [Verified]
- CLI: `openui` with `generate --spec`, `generate-api-key`, `deploy` (Vercel) (`packages/openui-cli/src/commands/`). [Verified]
- There is **no MCP server** exposed by OpenUI itself. It is only an MCP *client consumer* (`runtime/mcp.ts:1-21`). [Verified]

## T6 — How it plugs into agents

In plain words: any agent that can stream text can use it. You put the generated prompt in the system message and pipe the streamed text into the renderer. For LangGraph there is a ready AG-UI bridge.

- Generic path: system prompt = `library.prompt()`. The assistant text stream goes to `<Renderer isStreaming>`, and an action event becomes the next user turn: `"msg\ncontext and form values = [...]"` (`spec/prompt.md` §6.3, lines 305-336). [Docs]
- AG-UI: `openUIStreamTransformer` maps LangGraph protocol-v2 message, tool and error events to AG-UI events: `TEXT_MESSAGE_START/CONTENT/END`, `TOOL_CALL_START/ARGS/END/RESULT`, `RUN_ERROR` (`packages/langchain/src/transformer.ts:46-180`, `284`, `355`). [Verified]
- The server relay adds `RUN_STARTED`/`RUN_FINISHED`/`RUN_ERROR` lifecycle events and sends them as SSE. Error messages are replaced with "LangGraph run failed" unless `debug` is set (`stream-relay.ts:167-232`, `444-445`). [Verified]
- The frontend `agUIAdapter()` is a plain SSE `data:` JSON line reader (`packages/react-headless/src/stream/adapters/ag-ui.ts:4-19`). Depends on `@ag-ui/core ^0.0.57` (`packages/langchain/package.json:82-83`). [Verified]
- Other adapters: `assistant-ui` renderers, `a2ui` (Google A2UI surfaces), and OpenAI/Vercel stream adapters in server. The OpenClaw OS plugin is external (`README.md:108`). [Verified/Docs]
- Agent-aware scaffolding: the CLI detects the calling coding agent (`packages/openui-cli/src/lib/detect-agent.ts`) and writes an agents guide (`lib/scaffold/agents-guide.ts`). [Verified]

## T7 — LLM usage

In plain words: the open-source core never calls a model. The model is yours. Only the optional Thesys cloud services, plus the benchmark script, make LLM calls.

- lang-core, react-lang and the renderers contain no LLM calls. They work offline once the model output is local. [Verified by reading the `lang-core` imports; telemetry is the only network code, see T8]
- Autofix: `@openuidev/server` first validates locally. If there are errors it POSTs `{messages:[system: cloud config, …last ≤20 msgs/≤8,000 chars, assistant: generation]}` to `https://api.thesys.dev/v1/autofix` with a Bearer key. Generations are capped at 100,000 characters (`packages/server/src/shared/fix.ts:22-47`, `utils.ts:83-99`, `constants.ts:1`, `types.ts:4`). The model that does the fixing is hidden on Thesys's side. [Verified; model identity unknown]
- Cloud scripts: `/v1/app/execute` runs generated scripts remotely (T8). [Verified]
- Benchmarks call OpenAI (`gpt-5.2`, temperature 0) once per scenario (`benchmarks/README.md:23-24`, `generate-samples.ts:52-55`). [Verified]
- Cost drivers for a host: a fixed system prompt of about 3.3k tokens (reference library), plus output tokens. Edit mode reduces output by sending only patches (`spec/language.md:459`). [Verified/Inference]

## T8 — Security, privacy, safety, legal

In plain words: on the client, the language is sandboxed by design. It can only call functions you register. The real risks are around it: install-time telemetry is on by default, a production chat helper opens model-chosen URLs without checking them, dev mode pulls code from a CDN, and the cloud features send prompts, outputs and generated code to Thesys.

- **Model-generated code execution (client):** none. The interpreter is closed and only reaches whitelisted calls (`evaluator.ts:104-133`). Custom functions "MUST be pure and synchronous" (`spec/language.md:309`). [Verified]
- **Data-fetching blast radius:** a `Query(tool, args)` can call **any** tool in the host's `toolProvider` with model-chosen args, as soon as streaming ends. It can also re-run on a model-chosen `refreshInterval`, which is `setInterval(n*1000)` with **no minimum clamp** (`queryManager.ts:374-389`, `useOpenUIState.ts:228-230`). A host that passes a full MCP client therefore gives the model's UI the whole tool surface. Mutations require a user click (`@Run`) (`spec/language.md:406`). [Verified; abuse scenario is Inference]
- **Unsanitised `window.open` (react-ui chat):** `runChatAction` calls `window.open(url, "_blank")` on `@OpenUrl` with **no scheme check and no `noopener`** (`packages/react-ui/src/components/OpenUIChat/utils/actionMessage.ts:26-28`). This breaks the package's own rule that `safeOpenUrl` is "the single allowed entry point for `window.open`" (`packages/react-ui/src/components/_shared/utils/safeUrl.ts:25-45`, scheme blocklist at `:9`). assistant-ui and a2ui use `noopener,noreferrer` but also skip the scheme check (`packages/assistant-ui/src/renderers.tsx:125`, `packages/a2ui/src/Renderer.tsx:111`). [Verified code; exploitability Inference, not run]
- **Install telemetry (opt-out):** `lang-core` declares `"postinstall": "node ./postinstall.cjs"` (`packages/lang-core/package.json:56`). On every install it:
  - writes `~/.config/openui/telemetry.json` (random id and salt)
  - runs `git -C $INIT_CWD config --get remote.origin.url`
  - sends a salted SHA-256 project id plus Node/OS/package-manager/CI/Docker info to a PostHog capture endpoint at `https://dgoeivjus9jfp.cloudfront.net`

  (`packages/lang-core/src/telemetry/install.ts:129-177`, `203-205`, `265-267`, `shared.ts:3`). Disable with `OPENUI_TELEMETRY_DISABLED=1` or `DO_NOT_TRACK=1` (`install.ts:106-110`, `shared.ts:24-26`). [Verified]
- **Runtime telemetry:** opt-in only (`OPENUI_RUNTIME_TELEMETRY_ENABLED=1`), sampled at 10%, server-side, and limited to hashes and counts. The hook points are in `createParser().parse` and `generateSystemPrompt` (`telemetry/runtime.ts:14-45`, `parser.ts:753-760`, `prompt.ts:775-780`). [Verified]
- **CLI telemetry:** sent through posthog-node. Opt out with `DO_NOT_TRACK`, `OPENUI_TELEMETRY_DISABLED` or `--no-telemetry` (`packages/openui-cli/src/lib/telemetry.ts:81-85`, `122`). [Verified]
- **observability-cloud:** defaults to `capture:"full"` (`packages/observability-cloud/src/index.ts:47-48`). That mode sends the **raw model response** (truncated at 16,384 characters) and error messages to `https://ingest.thesys.dev/v1/events` (`index.ts:6`, `events/stream.ts:17`, `145-151`). The beacon path puts the client API key in the query string (`core/transport.ts:24-27`, `118`). `minimal` mode still sends `unresolved`/`orphaned` identifier names (`events/stream.ts:132-142`). [Verified]
- **Gateway/Autofix:** sends recent conversation text, which may contain PII, to Thesys (`fix.ts:36-47`). [Verified]
- **Cloud scripts:** the full OpenUI response, including model-generated JS (`"async ({ tools }) => …"`, `responseBundle.ts:8-9`), goes to `api.thesys.dev/v1/app/execute`. Thesys then drives your tools through `tool_calls` rounds. Each batch is validated before any tool runs, and the limits are 8 rounds, 16 calls and a 60 s timeout (`packages/server/src/tools/execute.ts:13-15`, `79-185`). Trust boundary: a remote party decides which of your tools to call, with which args. [Verified]
- **Dev-time remote code:** with `NODE_ENV==="development"`, react-lang auto-mounts devtools, which `import()`s `https://cdn.jsdelivr.net/npm/@openuidev/devtools@0/dist/devtools.browser.js`. The major tag `0` floats, and the module is handed your React and react-lang (`packages/react-lang/src/devtoolsBootstrap.ts:20-40`, `packages/devtools/src/cdn.ts:23-29`, `48-75`). [Verified]
- Legal: MIT is compatible with omni-agent's MIT license. Keep the copyright notice. [Verified]

## T9 — Quality: tests, benchmarks (re-checked), CI

In plain words: testing is strong for a project this age. There is a written spec, 58 shared conformance cases, and 128 test files run in CI. The token benchmark is real and reproducible, but its headline uses a pretty-printed JSON baseline. Against compact JSON the gain is small.

- Tests: 128 `*.test.ts(x)` files under `packages/` (`find`). Conformance runners exist for both the parser and React interactions (`packages/lang-core/src/__tests__/conformance.test.ts`, `packages/react-lang/src/__tests__/conformance.interactions.test.tsx`). [Verified]
- CI: `.github/workflows/build-js.yml:24-32` runs `pnpm install --frozen-lockfile`, builds packages, then runs each package's `ci` script (lint + format + test + typecheck, `packages/lang-core/package.json` `"ci"`). There are also workflows for cli-e2e, lighthouse, changesets and publishing. [Verified, not executed]
- **Benchmark methodology** (`benchmarks/README.md:21-28`):
  1. 7 prompts, **one** model (gpt-5.2), one run each, temperature 0.
  2. **Only OpenUI Lang is generated by the model.** The YAML, Vercel JSONL and C1 JSON are *projections of the parsed OpenUI AST*, so no model ever wrote those formats. This measures encoding size, not how well a model writes each format or how often it gets it right.
  3. Token count uses `tiktoken encoding_for_model("gpt-5")` (`run-benchmark.ts:5`, `18-20`). Latency is a fixed 60 tok/s estimate, not measured (`run-benchmark.ts:16`).

  [Verified]
- **My re-count** (`o200k_base`, on the shipped `benchmarks/samples/*`):

  | Format | README total | My total | OpenUI saving |
  |---|---:|---:|---:|
  | OpenUI Lang | 4,800 | 4,800 | — |
  | Vercel JSONL | 10,180 | 10,180 | 52.8% |
  | YAML | 9,122 | 9,129 | 47.4% |
  | Thesys C1 JSON (pretty, indent 2) | 9,948 | 9,650 | 50.3% |
  | **C1 JSON minified** (`separators=(",",":")`) | — | **5,487** | **12.5%** |

  The minified C1 drift (9,948 vs 9,650) is probably a tokenizer or newline difference. [Verified count; cause Inference]
- The C1 baseline is pretty-printed: `JSON.stringify({component, error:null}, null, 2)` (`benchmarks/thesys-c1-converter.ts:36-46`). Per scenario against minified JSON: simple-table −18.2%, chart −14.8%, contact-form −31.8%, **dashboard +1.6% (OpenUI is larger)**, pricing −13.3%, settings −17.7%, e-commerce −14.5%. [Verified]
- "Up to 67%" is the single best cell (contact-form vs Vercel, −67.1%, `README.md:148`). [Verified]
- The samples are **stale against spec 1.0**. They use string rule lists (`["required","minLength:2"]`) and string action ids (`Button("Submit", "submit:contact", …)`) (`benchmarks/samples/contact-form.oui`). The benchmark prompt has 0 occurrences of `Action(`/`@ToAssistant` (grep `benchmarks/system-prompt.txt`), while 1.0 uses `{required:true}` objects and `@ToAssistant` (`spec/language.md:378`, `416-433`). [Verified]
- Not measured at all: the fixed system-prompt cost (3,281 tokens), error rate per format, and real streaming time to first render. [Verified absence]

## T10 — Operations

In plain words: you `npm install` a couple of packages. There is no server to run unless you choose the Thesys services.

- Minimal footprint: `@openuidev/lang-core` (runtime dep only `ci-info`; peers `zod`, optional `@modelcontextprotocol/sdk`) plus `@openuidev/react-lang` (`packages/lang-core/package.json:61-69`, `96-98`). [Verified]
- Lifecycle hooks: `postinstall` (telemetry, swallows errors) and `prepare` (build, only relevant to the monorepo) (`packages/lang-core/package.json:56-57`, `postinstall.cjs`). Set `OPENUI_TELEMETRY_DISABLED=1` in CI and dev shells. [Verified]
- `react-ui` is heavy (792 TS/TSX files, 73k lines). Use it only if you want the reference chat shell. [Verified]
- Scaffolding: `openui` CLI (`packages/openui-cli/src/program.ts`) with Vercel deploy helpers. [Verified]

## T11 — New vs marketing

In plain words: the genuinely new part is a small, well-specified streaming language with sensible "render what's valid" rules and a prompt generated from schemas. The token-savings claim and the "open standard" label are partly marketing.

- **Novel and real:**
  - Statement-per-line DSL with hoisted references, so the model can write `root` first and children later and the UI still renders progressively (`spec/language.md:212-224`).
  - Watermark-plus-autoclose streaming parser that must equal the batch parse (`parser.ts:490-681`).
  - A single error shape aimed back at the model (`spec/language.md:505-519`).
  - Prompt derived from Zod schemas (`prompt.ts:556-616`).
  - Reactive `$state`, `Query`/`Mutation` and `@Each` in a declarative UI language.

  [Verified]
- **"Open Standard":** the spec is MIT and versioned, and there are conformance fixtures. But there is a single vendor, and the Cloud features rely on proprietary marker blocks and endpoints. [Verified/Inference]
- **"Up to 67% fewer tokens than JSON":** misleading. 47–53% is against verbose formats, and about 12.5% is against compact JSON (T9). [Verified]
- **"Built-in data fetching":** true, but it is host tool calls (map or MCP), not a fetch layer. Server-side "script" execution is a paid Thesys service (`execute.ts:88-93` requires `apiKey`). [Verified]
- **React, Vue, Svelte, Angular:** all present (`packages/{vue,svelte,angular}-lang`). [Verified]

## T12 — Weaknesses, limits, risks

In plain words: it is young, owned by one vendor, and the comfortable defaults send data to that vendor. The language is UI-only, and models have to learn positional arguments.

- Positional args are fragile. Reordering Zod keys silently changes meaning for stored programs. The spec addresses this with backward-compat rules (`spec/prompt.md` §3.2). [Inference]
- Per-chunk `preprocess` of the whole buffer is O(n²) per stream (`parser.ts:519-527`). [Inference]
- `refreshInterval` has no lower bound, so a model could write a 0.01 s refresh and hammer a tool (`queryManager.ts:380-387`). [Verified code; impact Inference]
- `Query` runs automatically after the stream ends with model-chosen args. There is no per-tool allowlist inside OpenUI, so the host must scope `toolProvider` (`spec/language.md:396`). [Verified]
- The unsanitised `window.open` in react-ui chat (T8). [Verified]
- Opt-out install telemetry plus a full-capture default in observability-cloud is privacy debt for enterprise users (T8). [Verified]
- The benchmark is stale and single-run (T9). The shallow clone hides commit history, so churn and bus-factor cannot be assessed. [Verified]
- Many satellite packages are 0.0.x (langchain, observability-cloud), so expect API churn. [Verified]

## T13 — How it fits into omni-agent

In plain words: OpenUI is the "show, don't tell" layer. Instead of answering in markdown, omni-agent agents would emit OpenUI Lang. The chat would then show real sprint boards, burndown charts, review checklists and incident dashboards, and clicks would flow back to the agent as structured events. Use the MIT core as a library with our own components and tools, and leave out the Thesys cloud pieces.

**Agile-lifecycle roles**

| Phase | UI the agent emits | OpenUI features used |
|---|---|---|
| Discover | Option comparison cards, interview/intake forms | `Form` + `@ToAssistant(msg, context)` round-trip |
| Plan | Sprint board, backlog table with filters, estimate sliders | `$state` + `@Filter/@Sort` + `Query("listIssues", {sprint:$s})` |
| Build | PR/diff summary cards, task checklists | static components, `@OpenUrl` (sanitised) |
| Verify | Review form (approve/request changes), test-result table | `Mutation("submitReview")` + `@Run` + form validation rules |
| Release | Release checklist, changelog preview, go/no-go buttons | custom actions `@Approve(...)` → typed `onAction` |
| Operate | Incident dashboard with auto-refresh KPIs | `Query(..., refreshSeconds)` (we clamp the minimum) |
| Learn | Retro board, burndown/velocity charts | chart components, edit-mode patches for live updates |

**Decision: DEPEND on `@openuidev/lang-core` + `@openuidev/react-lang`. PORT the ideas, not the packages, for the server side. SKIP Cloud, Autofix, observability-cloud and devtools auto-mount.**

- License: MIT, compatible. Keep the notice (`LICENSE`). [Verified]
- Required guards:
  - Set `OPENUI_TELEMETRY_DISABLED=1` in every environment (or `pnpm install --ignore-scripts` for lang-core) (`install.ts:133`).
  - Never mount `react-ui`'s `OpenUIChat` without routing `open_url` through our own sanitiser (`actionMessage.ts:28`).
  - Build with `NODE_ENV=production`, or block the jsDelivr devtools import (`devtoolsBootstrap.ts`).
  - Wrap `toolProvider` with an allowlist, arg schemas and a minimum refresh interval (≥ 5 s).

  [Inference]
- Effort:
  - About 3–5 days for an "Agile component library": about 12 Zod components (SprintBoard, IssueRow, Burndown, VelocityChart, ReviewForm, IncidentPanel, KPI, Checklist, Timeline) plus renderers on our UI kit.
  - About 2 days for tool-provider wiring to omni-agent's tool bus and the action→turn bridge.
  - About 1 day for a conformance run on our build.

  [Inference]
- Interface contract (proposed):
  ```ts
  // omni-agent ↔ OpenUI boundary
  interface GenUISurface {
    library: Library;                         // createLibrary({ components, actions: [Approve, Assign, …], root: "Page" })
    systemPrompt(opts: { tools: ToolSpec[]; editMode?: boolean }): string;   // library.prompt(...)
    render(stream: AsyncIterable<string>, ctx: { runId: string }): void;     // <Renderer isStreaming …>
  }
  interface GenUIToolProvider {               // passed as Renderer.toolProvider
    callTool(name: AllowedTool, args: Record<string, unknown>): Promise<unknown>;  // allowlisted, schema-checked, audited
  }
  type GenUIEvent =                           // Renderer.onAction → omni-agent event bus
    | { type: "continue_conversation"; humanFriendlyMessage: string; params: { context?: unknown }; formName?: string; formState?: Record<string, unknown> }
    | { type: "open_url"; params: { url: string } }                          // sanitised by omni-agent
    | { type: "Approve" | "Assign" | "CreateTicket"; params: Record<string, unknown> };  // custom actions → workflow transitions
  // Errors: OpenUIError[] (spec §8.3) fed back as a hidden system note on the next turn.
  ```
- Why not port the parser: lang-core is about 12k lines and has a conformance suite. Re-implementing it is a weeks-long job with drift risk. A Python port is only worth doing if omni-agent needs server-side validation without Node; otherwise call `createParser` in a Node sidecar. [Inference]

## T14 — Open questions

In plain words: these could not be settled from the code alone and need a run or a vendor answer.

- Does `window.open("javascript:…", "_blank")` from `actionMessage.ts:28` execute in the host origin in current Chromium/Firefox/Safari? It needs a browser test. [Open]
- Token savings with **model-generated** minified JSON and current 1.0 syntax, and the error rate per format. The benchmark does not answer either (T9). [Open]
- What do Thesys's `/v1/autofix` and `/v1/app/execute` retain, and for how long? There is no data-retention statement in the repo (grep found none). [Open]
- Will "Open Standard" governance be opened up (multiple vendors, RFC process)? The spec is versioned but has a single owner (`spec/language.md:646-647`). [Open]
- How well do smaller and open-weight models follow positional signatures compared with JSON-schema tool calls? There is no eval in the repo. [Open]
- The OpenClaw OS plugin (separate repo) was not inspected. How it serves OpenUI workspaces is unknown. [Open]
