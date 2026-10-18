# omni-agent: proposed architecture and build order

Built from `agents-compared.md`, `capability-map.md` and the 16 cited reports. Facts trace back to those reports;
design choices and estimates are mine and marked [Inference].

## 1. In plain words

omni-agent shouldn't be one giant agent. It should be **one platform**: a work board that turns goals into small work
items, hands each item to the right agent, accepts it as "done" only when an independent check passes, and remembers
what the team learned. The agent engines can be our own or existing ones (Claude Code, Codex, Hermes, OpenClaw),
plugged in through one contract.

## 2. Design principles (the mindset, distilled from all 12 repos)

1. **The harness is the product; the model is a part.** Every repo studied wins or loses on its harness, not on the model.
2. **Small core, capability at the edges** (Hermes). Every tool is a cost on every call; most capability lives in plugins, MCP servers and skills.
3. **Code enforces, prompts advise.** Budgets, approvals, scope and sandboxes live in code (all three agents; Paperclip budgets).
4. **"Done" means evidence, checked by someone who didn't do the work** (Learn Harness Engineering; Odysseus completion gate; tutorial Stage 9).
5. **Files and SQL are memory; memory must earn its place** (Hermes caps; OpenClaw dreaming + provenance; Odysseus skill audit).
6. **Untrusted by default.** Anything read from outside is data, marked as such, and can't be promoted into memory or instructions (OpenClaw, REA).
7. **Safe defaults** (Odysseus sandbox-or-refuse). The failure mode is "refuse and explain", never "run on the host".
8. **Interoperate, don't out-feature.** Use MCP, ACP and adapters to drive existing agents rather than re-implementing them.
9. **Licenses decide the architecture.** MIT core; AGPL projects only as separate services or clean-room ideas.

## 3. Layers

```
                 ┌───────────────────────── Surfaces ─────────────────────────┐
                 │ Web app (React + OpenUI generated views) · CLI · chat via  │
                 │ Hermes/OpenClaw gateways · editors (ACP) · MCP server      │
                 └──────────────────────────────┬─────────────────────────────┘
┌──────────────────────── Work layer (Agile control plane) ───────────────────────┐
│ goals → epics → stories/issues · sprints · estimates · WIP limits · review      │
│ stages · completion contracts · budgets (reserve + hard stop) · approvals ·     │
│ run-event log · scheduler (turn / goal / time / event loops)                    │
│ dispatch(WorkItemDispatch) ─────────────┐        ┌──── RunEvent / RunResult     │
└─────────────────────────────────────────┼────────┼──────────────────────────────┘
                     ┌────────── Engine adapters (one contract) ──────────┐
                     │ omni runtime · Claude Code · Codex · Hermes ·       │
                     │ OpenClaw gateway · generic HTTP / process           │
                     └───────────────┬─────────────────────────────────────┘
┌──────────────── Shared services (called by every engine over MCP/HTTP) ──────────────┐
│ Memory (observations, dreaming, skills)  · Knowledge graph (graphify code map)       │
│ Ingestion (MarkItDown behind ingest())   · Research (deep research, compare)          │
│ Verify: tests/evals/arch rules, independent checker, Strix scan, provenance lint      │
│ Discover extras: REA (MCP, static-only), MiroFish rehearsal (separate AGPL service)   │
└──────────────────────────────────────────┬────────────────────────────────────────────┘
┌──────────────── Safety substrate (under everything) ──────────────────────┐
│ sandbox-or-refuse execution · effect audit per tool · untrusted marking ·  │
│ secrets substituted at network layer · smart approvals · egress allowlist │
│ · audit ledger · OTel traces · telemetry off unless opted in              │
└───────────────────────────────────────────────────────────────────────────┘
```

Stack [Inference]: Python 3.12 + FastAPI + Postgres for the work layer and services (the owner's strongest stack, and
DataCrate360's), React + OpenUI for generated views, SQLite allowed for single-user mode. Engines talk over HTTP/MCP,
so the engine's language doesn't matter.

## 4. Contracts (the parts that must not change often)

Proposed by the analysts in each report's T13; harmonised here.

| Contract | Between | Shape (abridged) | Source |
|---|---|---|---|
| `WorkItemDispatch` → `RunEvent*` → `RunResult` | work layer ↔ engines | dispatch: item id, goal, acceptance criteria, budget, scope, allowed tools, handoff summary; events: progress/tool/cost; result: status, artifacts, evidence, cost | Paperclip T13.4 |
| `CompletionCheck(item, result) -> passing \| active \| blocked` | work layer ↔ verifier | only this may set "passing"; records evidence | tutorial Stage 9 `check()` |
| `MemoryPort` | engines ↔ memory | `capture(observation)`, `inject(budget, scope)`, `search(q)`, `timeline`, `get_observations` | claude-mem T13 |
| `ingest(IngestRequest) -> IngestResult` | anything ↔ ingestion | bytes from our own fetcher; `doc_id = sha256`, anchors, errors (`TOO_LARGE`, `TIMEOUT`, `BLOCKED_SOURCE`) | MarkItDown T13 |
| Graph query | engines ↔ knowledge graph | `query(q, budget_tokens, seeds?)`, `path(a,b)`, `affected(paths)`; every edge EXTRACTED/INFERRED + score | graphify T13 |
| `GenUISurface` / `GenUIEvent` | UI ↔ engines | allowed component library → prompt; events back as typed actions; tools allowlisted | OpenUI T13 |
| `security_scan(target, scope_ref, mode, budget, max_turns, diff_base?)` | verifier ↔ Strix | SARIF findings + `run.json`; clean only if completed with coverage | Strix T13 |
| `ReverseEngineeringPort` | discover ↔ REA | effect-class allowlist (default-deny foreign code, network, device, mutation), evidence envelope | REA T13 |
| `rehearse_reaction(...)` | discover ↔ MiroFish service | hard `max_rounds`, `max_llm_calls`, non-confidential seeds | MiroFish T13 |
| `inspect_provenance(path)`, `label_artifact(path, …)` | verify/release ↔ provenance | read-only inspection; labels are added, never stripped | watermarks-remover T13 |

## 5. Build order

Each phase ends with something the owner uses on his own work (DataCrate360 is the first customer). Estimates are
full-time weeks (FTW); multiply by about 2.5 for part-time [Inference].

| Phase | Goal | Contents | FTW |
|---|---|---|---|
| **0. Foundations** | Safe skeleton | Repo harness (AGENTS.md, PROGRESS, feature list, `make check`); safety substrate v1 (sandbox-or-refuse, effect audit, approvals, audit log) | 2 |
| **1. Verified delivery loop (MVP)** | One work item, done with evidence | Work layer v1 (issues, single assignee, atomic checkout, WIP=1, budgets, review stage); `WorkItemDispatch` with **Claude Code + Codex adapters**; independent `CompletionCheck` running the repo's own tests; web board | 4–5 |
| **2. Agile** | A real sprint | Sprints, estimates, column WIP limits, burndown, retro notes; OpenUI board/burndown/review components | 2–3 |
| **3. Learn** | It remembers | Memory port (observations + capped files + session search), skill writing + baseline audit, dreaming-style promotion | 3–4 |
| **4. Discover + ingest** | It reads everything | MarkItDown `ingest()`, graphify code map, deep-research pipeline | 2–3 |
| **5. Verify+** | Ship safely | Strix container job on staging, arch rules, provenance lint + AI labels | 2 |
| **6. Reach** | Use it from anywhere | Hermes/OpenClaw adapters (chat channels), ACP, MCP server, own runtime engine | 3–4 |
| **7. Optional** | Experiments | REA (static-only), MiroFish rehearsal, local-model serving | as needed |

MVP (phases 0–1): **~6–7 FTW ≈ 4 months part-time**. Phases 0–5, a credible v1: **~16–19 FTW ≈ 9–12 months part-time**.

## 6. Honest assessment: can one agent combine all of this?

**Short answer: one *platform*, yes. One merged *agent*, no. And "dominating all agentic tools" is the wrong target.**
Here is why, in order of importance.

1. **Merging the code is not possible in any useful sense.** The projects are Python and TypeScript, ~5M lines in
   OpenClaw alone, with conflicting trust models (OpenClaw trusts the operator, Odysseus sandboxes everything) and two
   AGPL licenses. A merged codebase would be unmaintainable for one person; Hermes, OpenClaw and Paperclip each have
   hundreds of contributors and ship weekly.
2. **Merging the *ideas* is very possible, because the cores are small and they converge.** All three agents
   independently arrived at the same seven principles (agents-compared §4). The building blocks are mostly *patterns*
   (an observer that writes typed observations, a confidence-tagged graph, a dispatch contract, an evidence envelope),
   each a few hundred to a few thousand lines when re-implemented. That's what this plan does.
3. **"One agent that does everything" is an anti-pattern the research itself warns about.** Tools cost attention on
   every call (that's why Hermes and OpenClaw added tool search), and one shared environment widens the blast radius
   (Strix shares one container across all its agents). The design that scales is one control plane, several narrow
   agents, shared memory and an independent checker.
4. **Out-featuring them is a losing game; out-*focusing* them is not.** OpenClaw has a foundation and ~10
   releases a month; Hermes has Nous Research behind it. But **none of the 12 does verified Agile delivery**: no
   sprints, estimates, WIP limits or evidence-gated "done" (Paperclip comes closest and has none of the Agile
   pieces; agents-compared §6). And none knows data work (warehouses, dbt, Airflow, data contracts). That
   intersection, *agents that ship verified increments for data teams*, is empty, and it matches your background,
   your DataCrate360 labs and your users.
5. **Interoperate with the giants instead of fighting them.** Treat Claude Code, Codex, Hermes and OpenClaw as
   engines behind one adapter contract (Paperclip already proves this works with 16 adapters). Their progress then
   becomes your progress.
6. **Constraints are real; plan for them.** Solo, a full-time job, a PhD from September 2026, and no revenue yet
   because of visa timing. That argues for an open-source-first, part-time plan with a small MVP that you use daily on
   DataCrate360 itself. The schedule above assumes that.
7. **The main risks:** upstream churn breaking adapters (pin versions, contract tests); model cost (budgets with hard
   stops from day one); security liability, because agents run code on people's repos (safe defaults are part of the
   product); legal exposure (no watermark removal, REA only static and clean-room, no AGPL code).

**What success looks like in six months [Inference]:** DataCrate360's own backlog runs on omni-agent. Every merged
change passed an independent check. Cycle time and "done with evidence" rate are measured. One outside data team
uses it. That is a far stronger position than a feature list, and it's reachable.
