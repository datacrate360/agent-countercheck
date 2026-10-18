# Capability map: 12 repos → omni-agent modules

Every capability found in the research, assigned to one omni-agent module with a decision:

- **BUILD**: write it ourselves (small, core, or nothing suitable exists)
- **PORT**: re-implement the *idea* from the report, no code copied (or copied with license + NOTICE where allowed)
- **DEPEND**: use the project as a library
- **SERVICE**: run the unmodified project as a separate process (MCP/HTTP/CLI)
- **SKIP**: leave out

Effort figures are the analysts' estimates (report section T13) or mine [Inference]. "wk" = full-time weeks.

## 1. License plan (decides everything else)

| Project | License | Allowed modes for an MIT omni-agent |
|---|---|---|
| Hermes, OpenClaw, Paperclip, OpenUI, REA, MarkItDown, watermarks-remover | MIT | any (keep the notice) |
| claude-mem, graphify, Strix, OASIS | Apache-2.0 | any (keep LICENSE + NOTICE; mark modified files) |
| **Odysseus, MiroFish** | **AGPL-3.0** | **ideas only (clean-room), or an unmodified separate service.** Never import or copy. |
| PyMuPDF (via markitdown-ocr) | AGPL-3.0 / commercial | don't install the OCR plugin; use another OCR path |
| IDA, Hopper, JEB (via REA) | commercial | keep outside omni-agent's distribution |

## 2. Module map

| omni-agent module | Agile stage | Capability | Source | Decision | Effort |
|---|---|---|---|---|---|
| **core: agent runtime** | all | Bounded loop, parallel tools, guardrails instead of turn caps | Hermes D3, OpenClaw A | BUILD (small, our own) | 2–3 wk |
| | | Provider adapters + text tool-call repair | OpenClaw A (`tool-call-repair`), Odysseus A | PORT | 1 wk |
| | | Fallback blocked after a side effect | OpenClaw A | PORT | 2 d |
| | | Prompt-cache discipline + tool search | Hermes D4–D6 | BUILD | 3 d |
| | | Harness-as-plugin (Claude Code, Codex, Hermes, OpenClaw as engines) | OpenClaw A; Paperclip adapters | BUILD the adapter contract; PORT patterns | 1–2 wk |
| **safety** | all | Sandbox-or-refuse execution | Odysseus B | PORT (clean-room) | 1 wk |
| | | Effect audit on every tool (read-only / network / foreign code / destructive) | REA T2 | PORT | 3–5 d |
| | | Untrusted-content marking + provenance | REA, OpenClaw C | PORT | 3 d |
| | | Secrets substituted at the network layer | OpenClaw B | PORT | 1–2 wk |
| | | Smart approvals (hard blocks + guard model + human) | Hermes D8 | PORT | 1 wk |
| | | Budgets with reservation + hard stop | Paperclip T2 | PORT | 3–5 d |
| **work layer (Agile)** | Plan, Review | Goals → epics → issues, single assignee, atomic checkout, wake merging, review stages, completion contracts, run-event log | Paperclip T2/T13 | SERVICE first (Paperclip over REST/MCP), then PORT a lean version | 1–2 wk, then 4–8 wk |
| | | Sprints, estimates, velocity, column WIP limits, burndown, retros | (gap in every repo) | BUILD | 2–3 wk |
| | | Evidence-gated "done": independent checker, pass-state gating, WIP=1, clean state | tutorial Stage 9, Learn Harness Engineering, Odysseus completion gate | BUILD (already prototyped in Stage 9) | 1 wk |
| | | `/goal` judge + loop types (turn, goal, time, event) | Hermes D11 | PORT | 3 d |
| **memory (Learn)** | Learn → Plan | Capture → observer → typed observations → budgeted injection | claude-mem T2 | SERVICE pilot (1–2 d), then PORT | 2–3 wk |
| | | Dreaming: promote after repeated recall, diary, provenance | OpenClaw C | PORT | 1–2 wk |
| | | Capped human-readable memory files + session search (FTS5) | Hermes D9 | PORT | 3 d |
| | | Skill writing + curator | Hermes D10, OpenClaw C | PORT | 1 wk |
| | | Skill must beat a no-skill baseline before approval | Odysseus A (clean-room) | BUILD from the idea | 1 wk |
| **knowledge graph (Discover/Learn)** | Discover, Plan, Build | Code map, EXTRACTED/INFERRED confidence, blast radius | graphify | SERVICE (MCP/CLI, code-only) + PORT the rubric/cache | 2–4 d + 3–5 d |
| **ingestion** | all | Files → Markdown with page anchors, hardened fetcher, size/time caps | MarkItDown | DEPEND (library, behind `ingest()`) | 1–2 wk |
| **UI** | Plan, Verify, Operate | Generated sprint board, burndown, review form, incident panel | OpenUI | DEPEND (`lang-core` + `react-lang`), telemetry off, our sanitiser | 1–1.5 wk |
| **surfaces** | all | CLI, web app, chat channels, editors (ACP) | Hermes D12, OpenClaw D | BUILD web + CLI; reuse Hermes/OpenClaw as channel engines via adapters | 2 wk |
| **verify: security** | Verify | App-security scan of staging, SARIF ingestion, diff-scoped | Strix | SERVICE (container job) + scope approval + egress allowlist | 1–2 wk |
| **verify: quality** | Verify | Tests, evals, architecture rules (WHAT/WHY/FIX), clean-state check | tutorial §12, Learn Harness Engineering | BUILD (prototyped) | 3 d |
| **discover: research** | Discover | Deep-research pipeline (plan → search → read → synthesise) | Odysseus A (clean-room) | BUILD from the idea | 1–2 wk |
| | | Model compare (blind side-by-side) | Odysseus B (clean-room) | BUILD from the idea | 3 d |
| | | Stakeholder-reaction rehearsal | MiroFish | SERVICE experiment only; clean-room on OASIS if useful | 1 wk experiment |
| | | Competitive / dependency analysis of binaries and apps | REA | SERVICE (MCP), static-only allowlist, clean-room split | 1–2 d + 1–2 wk guardrails |
| **provenance** | Verify, Release | Invisible-Unicode / Trojan-Source lint; C2PA inspection; label AI-made deliverables | watermarks-remover (detection half only) | PORT detection; BUILD labelling | 1–2 d + 1 wk |
| | | Watermark / provenance **removal**, detector evasion | watermarks-remover | **SKIP** (conflicts with EU AI Act Art. 50 and CA SB 942 / AB 853) | — |
| **local models** | all | Hardware fit + local serving | Odysseus (clean-room) | later; start with any OpenAI-compatible endpoint | 1–2 wk (later) |
| **ops** | Operate | OTel export, audit ledger, telemetry opt-in only | OpenClaw D, Hermes | BUILD | 3–5 d |

## 3. What we deliberately do NOT take

- Any code from **Odysseus** or **MiroFish** (AGPL).
- OpenClaw's **in-process plugins with full privileges** and its host-trusted defaults.
- claude-mem's **cloud sync, telemetry and pre-selected paid tier**; Graphify/OpenUI cloud paths and telemetry.
- Strix's **default deep scan mode** and telemetry; REA's tools that run foreign code or touch devices (default-deny).
- Every **removal** feature from watermarks-remover.
