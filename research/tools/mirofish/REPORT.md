# MiroFish — research report

| Field | Value |
|---|---|
| Repo | 666ghj/MiroFish (shallow clone, commit `7657031`, 2026-10-02 +0800) |
| License | AGPL-3.0 (`LICENSE:1-2`, `backend/pyproject.toml:6`, `package.json:20`) |
| Stack | Flask backend (Python 3.11–3.12, uv) + Vue 3 / Vite / D3 frontend; simulation = `camel-oasis==0.2.5` + `camel-ai==0.2.78`; memory graph = **Zep Cloud** (`zep-cloud==3.25.0`) |
| Size | 128 files; ~27.8k Python lines (incl. tests/scripts), ~21k Vue/JS lines |
| Method | Static reading only. Nothing installed, built or run. OASIS README fetched once to explain the engine. |
| Analyst tags | [Verified] = read in code at the cited line; [Docs] = README/docstring/external doc; [Inference] = my reasoning from code |

## Executive summary

- MiroFish is an **orchestration and UI layer around OASIS** (camel-ai's Apache-2.0 LLM social-media simulator) and **Zep Cloud** (a hosted temporal knowledge graph). Most of the "swarm" mechanics come from OASIS. MiroFish's own work is the glue: building a knowledge graph from documents, turning graph entities into personas, having an LLM pick the simulation settings, and a ReACT report writer.
- Pipeline in code: 1 LLM call designs an ontology (exactly 10 entity types) → Zep Cloud pulls entities out of 500-char chunks → **one agent per entity that Zep found** (not "thousands") → 1 LLM call per agent writes a persona of about 2,000 characters → LLM calls write the time, event and per-agent activity config → OASIS runs **Twitter and Reddit simulations in parallel** inside a subprocess → an optional updater writes agent actions back into Zep → a ReportAgent writes a 2–5 section "future prediction report" by searching Zep and "interviewing" the agents.
- Default length: an LLM-chosen `total_simulation_hours` (prompt range 24–168, fallback 72) at 60 min per round, so about 72 rounds per platform. The UI recommends a cap of 40. Each round only activates a random subset of agents (fallback: N/15 to N/5 per hour, scaled by time of day).
- Cost per run (my estimate): about **0.5k–1.5k LLM calls** for roughly 50 entities, plus Zep Cloud extraction for every seed chunk and for every 5 simulated actions. The README itself warns "high consumption, try <40 rounds."
- Several knobs the LLM generates are **never read by the simulator**: stance, sentiment_bias, influence_weight, posts_per_hour, echo_chamber_strength, viral_threshold, recency weights, narrative_direction and scheduled_events. Only activity_level, active_hours, the agents-per-hour range, the peak/off-peak multipliers and the initial posts affect the run.
- **There is no validation, backtesting, calibration or accuracy measurement anywhere in the repo.** The "prediction" is LLM role-play seeded by LLM-written personas and LLM-written opening posts. Treat it as qualitative scenario exploration (how might stakeholders react, what talking points come up), not forecasting.
- Security posture is demo-grade: no auth, CORS `*`, binds to 0.0.0.0, default `SECRET_KEY`, and in-memory task state.
- Fit for omni-agent: an **optional Discover-phase "stakeholder reaction rehearsal"** tool. The best path is a **clean-room re-implementation on OASIS (Apache-2.0)**, or running MiroFish **unmodified as a separate sidecar service** behind an MIT MCP adapter. Never copy MiroFish code into the MIT platform.

## Pipeline diagram

```
 seed files (pdf/md/txt) + "simulation_requirement" (NL)
        │  POST /api/graph/ontology/generate
        ▼
 [LLM #1] OntologyGenerator ── 10 entity types (8 specific + Person + Organization), 6-10 edge types
        │  POST /api/graph/build  (background thread)
        ▼
 Zep Cloud: graph.create → set_ontology → batch.add(500-char chunks, ≤350/batch)
        │      (Zep's own LLM extracts entities/edges = hidden cost)
        ▼
 ZepEntityReader.filter_defined_entities  → N entities with a custom label
        │  POST /api/simulation/create → /prepare
        ▼
 [N LLM calls] OasisProfileGenerator (5 threads)  ← 2 Zep searches/entity for context
        │      → reddit_profiles.json + twitter_profiles.csv
        ▼
 [2 + ceil(N/15) LLM calls] SimulationConfigGenerator → simulation_config.json
        │  POST /api/simulation/start
        ▼
 subprocess: scripts/run_parallel_simulation.py
   asyncio.gather( OASIS Twitter env , OASIS Reddit env )   semaphore=30 each
   round r: pick active agents (time-of-day × activity_level) → LLMAction() each → env.step
        │  actions.jsonl ──► Flask monitor thread ──► ZepGraphMemoryUpdater (5 actions → 1 Zep episode)
        ▼  env stays alive for INTERVIEW commands (file-based IPC)
 POST /api/report/generate
 ReportAgent: plan outline (LLM) → per section ReACT ≤5 iters, ≥3 tool calls
   tools: insight_forge | panorama_search | quick_search | interview_agents (→ OASIS INTERVIEW via IPC)
        ▼
 full_report.md  +  /api/report/chat  +  /api/simulation/interview
```

---

## T1 — Identity

In plain words: MiroFish is an open-source "predict anything" web app from a small Chinese team incubated by Shanda Group. It is a Flask + Vue app that wires two third-party engines together: OASIS for the simulation and Zep Cloud for memory. It is young and has a demo-quality feel.

- Tagline: "A Simple and Universal Swarm Intelligence Engine, Predicting Anything", with claims of "thousands of intelligent agents" and "rehearse the future" — `README.md:7-9`, `README.md:29` [Docs]
- License is AGPL-3.0 for the whole repo (backend and frontend) — `LICENSE:1-2`, `backend/pyproject.toml:6`, `package.json:20` [Verified]
- Stack: Flask 3, OpenAI SDK, `zep-cloud==3.25.0`, `camel-oasis==0.2.5`, `camel-ai==0.2.78`, PyMuPDF — `backend/pyproject.toml:11-34`. Frontend: Vue 3.5, vue-router, axios, d3, Vite 7 — `frontend/package.json` [Verified]
- Version 0.1.0 — `backend/pyproject.toml:3`. Shallow clone at `7657031` (2026-10-02). 128 files [Verified]
- Company and upsell: "strategic support and incubation from Shanda Group"; a hiring call to mirofish@shanda.com; Discord/X/Instagram links; a hosted live demo — `README.md:187-191`, `README.md:45` [Docs]. There is no paid tier in the code, but the app **requires Zep Cloud** (a commercial SaaS, free tier) and rejects a self-hosted Zep URL — `backend/app/config.py:71-72` [Verified]
- Default LLM recommendation is Alibaba DashScope `qwen-plus` — `.env.example:3-6`. The code default is `gpt-4o-mini` at the OpenAI base URL — `backend/app/config.py:29-30` [Verified]

## T2 — Core mechanism (step by step)

In plain words: the user uploads documents and states what they want predicted. An LLM decides what kinds of actors exist, and Zep builds a knowledge graph of the real actors in the text. Each actor becomes a simulated social-media account with an LLM-written persona. OASIS then lets these accounts post, like, comment and follow for N simulated hours on a fake Twitter and a fake Reddit. Finally an LLM "report agent" searches what happened and writes a report framed as a prediction.

1. **Ontology design (1 LLM call).** The system prompt demands exactly 10 entity types (8 specific types plus the fallbacks `Person` and `Organization`), 6–10 edge types, and 1–3 attributes each. Entities must be actors who can speak on social media, not abstract concepts — `backend/app/services/ontology_generator.py:48-191`. Call: `chat_json(temperature=0.3, max_tokens=None, max_attempts=2)` — `ontology_generator.py:235-243`. Text over 50k chars is chunked into 8k pieces and representative excerpts are picked deterministically (no extra LLM calls) — `ontology_generator.py:251-255`, `:302-330` [Verified]
2. **Graph build in Zep Cloud.** A background thread splits the text into chunks (500 chars, 50 overlap by default) — `graph_builder.py:144`, `config.py:41-42`. It then calls `create_graph` → `set_ontology` (custom Zep entity and edge types) → `add_text_batches` through the Zep **Batch API**, sending at most 350 chunks per batch with an idempotency `operation_id` and a sha256 per chunk — `graph_builder.py:148-195`, `:399`, `:458-493`, `:572`. It waits for Zep to finish processing — `graph_builder.py:188`, `:631`. **Entity and relation extraction is done by Zep's own LLM, not MiroFish's** [Verified for the calls; extraction-inside-Zep is Inference]
3. **Entity → agent filter.** Only nodes carrying a custom label (anything other than `Entity`/`Node`) that matches the defined ontology types are kept — `zep_entity_reader.py:221-273`. **Agent count N = number of such entities.** The code never multiplies agents [Verified]
4. **Persona generation (N LLM calls, 5 in parallel).** For each entity, two parallel Zep `graph.search` calls (edges and nodes) collect facts — `oasis_profile_generator.py:348-413`, `:457-528`. One JSON call asks for `bio` (200 chars), `persona` (~2,000 chars covering background, MBTI, posting style, **stance**, catchphrases, "personal memory" of the event), age, gender, MBTI, country, profession and topics. The prompt says to "restore reality as much as possible" — `:721-768`. Organisations get an "official account" persona with age=30 and gender="other" — `:770-815`. Temperature is 0.7, lowered by 0.1 per retry, 3 attempts, then a rule-based fallback — `:567-625`. Individual and group types are hard-coded lists — `:232-241`. Thread pool size is `parallel_count=5` — `:901`, `:1003`, and the API default is also 5 — `api/simulation.py:499` [Verified]
5. **Simulation config (2 + ⌈N/15⌉ LLM calls).** The steps are a time config, an event config, then agent activity configs in batches of 15 — `simulation_config_generator.py:217`, `:276-327`. Time config: the LLM chooses `total_simulation_hours` (24–168) and `minutes_per_round` (30–120, suggested 60), with China (UTC+8) daily rhythm examples. The fallback is 72 h, 60 min, agents per hour N//15 to N//5 — `:545-611`. Event config: the LLM writes `hot_topics`, `narrative_direction` and **the initial posts**, each with a `poster_type` — `:678-705`. `scheduled_events` is always `[]` — `:725` [Verified]
6. **Two platforms = OASIS Twitter + OASIS Reddit, run concurrently.** Flask launches `run_parallel_simulation.py` as a separate process group — `simulation_runner.py:473`, `:518-548`. The script runs both platforms with `asyncio.gather` — `scripts/run_parallel_simulation.py:1585-1588`. Each platform calls `oasis.make(..., semaphore=30)` with its own SQLite DB — `:1155-1160`, `:1346-1351`. Twitter uses the main LLM and Reddit uses the optional "boost" LLM — `:1130`, `:1320`. Action sets: Twitter 6 actions, Reddit 13 actions — `config.py:49-56` [Verified]
7. **Round loop.** Round 0 publishes the LLM-written initial posts as `ManualAction(CREATE_POST)` — `run_parallel_simulation.py:1180-1206`. Then `total_rounds = hours*60/minutes_per_round`, truncated by `--max-rounds` — `:1214-1224`. Each round:
   - `get_active_agents_for_round` draws a target of `uniform(min,max) × multiplier` (peak ×1.5, off-peak from config, else ×1.0)
   - keeps candidates that are inside their `active_hours` and pass `random() < activity_level`
   - samples from them — `:1040-1080`
   - every selected agent gets `LLMAction()` (OASIS asks the LLM to pick and perform an action from its feed) — `:1253-1254`
   - new rows are read from the SQLite `trace` table into `actions.jsonl` — `:1256-1275` [Verified]
8. **Generated but unused parameters.** `stance`, `sentiment_bias`, `influence_weight`, `posts_per_hour`, `comments_per_hour`, `response_delay_*` (per agent) and `recency/popularity/relevance_weight`, `viral_threshold`, `echo_chamber_strength` (per platform) are defined in `simulation_config_generator.py:53-144`. A grep of `backend/scripts/` and `simulation_runner.py` finds **no reader** for any of them. `hot_topics` is only counted for UI display (`api/simulation.py:1287`) [Verified by grep]. Stance therefore only reaches agents indirectly, through persona text [Inference]
9. **"Dynamic temporal memory update."** When `enable_graph_memory_update` is on (the frontend always sends `true` — `frontend/src/components/Step3Simulation.vue:402`), the Flask monitor thread tails `actions.jsonl` — `simulation_runner.py:620-627`, `:787-790`. It turns each action into a sentence like `"[ts] [platform round r] Name: …"` — `zep_graph_memory_updater.py:36-67` — skips `DO_NOTHING` (`:372`) and sends batches of 5 as `graph.add` episodes — `:228`, `:494`. **The simulated agents never read Zep during the run.** The run script has no Zep reference. Agent memory inside the run is OASIS/CAMEL chat memory. The Zep update only feeds the report and chat stage [Verified grep; OASIS memory = Inference]
10. **Post-run environment kept alive.** After the loop the process enters "wait for commands" mode. Supported commands are `interview`, `batch_interview` and `close_env` — `run_parallel_simulation.py:1597-1625`, `simulation_ipc.py:25-29`. An interview is `ManualAction(INTERVIEW, {"prompt": …})` followed by reading the answer back from SQLite — `run_parallel_simulation.py:317-340`, `:517-541` [Verified]
11. **Report generation.** `plan_outline` pulls `get_simulation_context` (graph stats plus related facts) and makes one `chat_json` call to produce 2–5 sections, with a 3-section fallback — `report_agent.py:1176-1258`. Each section runs a ReACT loop with `max_iterations=5` and `min_tool_calls=3`, text `<tool_call>{json}</tool_call>` parsing and a `Final Answer:` sentinel — `:1326-1364`, `:1073-1118`, `:727-742`. The prompt frames simulated behaviour as "the predicted future", says "do not use your own knowledge", requires quoting agents and forbids fabrication — `:628-671` [Verified]
12. **Report tools (Zep plus agents).**
    - `insight_forge`: an LLM splits the question into ≤5 sub-queries, then Zep edge searches (limit 15/20) — `zep_tools.py:943-1014`, `:1090-1121`
    - `panorama_search` — `:1143`
    - `quick_search` — `:1235-1264`
    - Zep search uses `reranker="cross_encoder"` — `:486-491`
    - `interview_agents`: an LLM selects ≤5 agents, an LLM writes the questions, both platforms are interviewed through IPC (timeout 180 s), then an LLM summary — `:1270-1420`, `:1549-1721` [Verified]
13. **How a "prediction" is formed.** Nothing aggregates outcomes statistically. There is no sentiment index, no repeated runs, no confidence. The report is an LLM narrative grounded in retrieved simulation traces and interviews — `report_agent.py:604-611`, `:628-671` [Verified absence by reading the report path; Inference on implication]

## T3 — Architecture and process model

In plain words: there is one Flask server that does everything synchronously or in threads, plus one Python subprocess per simulation that holds the OASIS environments. The two processes talk through files on disk.

- Flask app factory registers 3 blueprints: `/api/graph`, `/api/simulation`, `/api/report` — `backend/app/__init__.py:66-69`, `backend/app/api/__init__.py:7-13` [Verified]
- Long work (graph build, prepare, report) runs in Python threads with an in-memory singleton `TaskManager`, so task state is lost on restart — `graph_builder.py:114`, `models/task.py:56-106` [Verified]
- The simulation runs as `subprocess.Popen([sys.executable, run_parallel_simulation.py, --config …], start_new_session=True)`. Stop is done with `killpg` — `simulation_runner.py:518-548`, `:909-960` [Verified]
- IPC is file based: Flask writes `ipc_commands/<id>.json` and the subprocess polls, then writes `ipc_responses/<id>.json`. Liveness is tracked in `env_status.json` — `simulation_ipc.py:1-7`, `:110-187`, `:270-276` [Verified]
- A monitor thread tails `twitter/actions.jsonl` and `reddit/actions.jsonl` to drive progress and the Zep updater — `simulation_runner.py:620-627`, `:767-790` [Verified]
- Zep client is cached per api_key, reads are retried, and graph lifecycle locks and reader registration prevent deleting a graph that is in use — `utils/zep.py:57-126`, `utils/zep_lifecycle.py:17-47` [Verified]
- Frontend: a 5-step wizard (`Step1GraphBuild` … `Step5Interaction.vue`) plus history and a D3 graph panel — `frontend/src/components/` [Verified]
- Deployment: one container runs `npm run dev`, which starts both servers in **dev mode** (Vite dev server plus Flask `app.run`) — `Dockerfile` (`CMD ["npm","run","dev"]`), `package.json` scripts [Verified]

## T4 — Data model

In plain words: everything lives as files under `backend/uploads/`, using JSON, CSV, JSONL, Markdown and two SQLite databases per simulation. The knowledge graph lives remotely in Zep Cloud.

- Project: `uploads/projects/<id>/project.json`, `files/`, `extracted_text.txt` — `models/project.py:111-136` [Verified]
- Simulation dir (`uploads/simulations/<sim_id>/`):
  - `state.json` — `simulation_manager.py:161`
  - `simulation_config.json` — `:443`
  - `reddit_profiles.json` and `twitter_profiles.csv` (OASIS input formats) — `oasis_profile_generator.py:1114`, `:1190`
  - `twitter_simulation.db` and `reddit_simulation.db` (OASIS SQLite, deleted at each start) — `run_parallel_simulation.py:1151-1153`
  - `twitter/actions.jsonl`, `reddit/actions.jsonl`, `env_status.json`, `ipc_*` [Verified]
- Persona schema `OasisAgentProfile`: user_id, user_name, name, bio, persona, karma, friend/follower/statuses counts (random if the LLM leaves them out), age, gender, mbti, country, profession, interested_topics, source_entity_uuid/type — `oasis_profile_generator.py:80-110`, `:318-334` [Verified]
- Config schema `SimulationParameters`: time_config, agent_configs[], event_config, twitter/reddit PlatformConfig, llm_model, generation_reasoning — `simulation_config_generator.py:53-196` [Verified]
- Report dir: `uploads/reports/<id>/` holds `meta.json`, `outline.json`, `progress.json`, `section_NN.md`, `full_report.md`, `agent_log.jsonl`, `console_log.txt` — `report_agent.py:1950-2002`, `:52-53` [Verified]
- Zep: one graph per project, with custom entity/edge types from the ontology. Seed chunks are stored as episodes with metadata `mirofish_operation_id`, `chunk_index`, `chunk_sha256`. Simulation actions become further episodes — `graph_builder.py:470-484`, `zep_graph_memory_updater.py:444-494` [Verified]

## T5 — Interfaces

In plain words: the only real interface is the REST API (about 60 routes) that the Vue UI uses. There is no CLI apart from the simulation scripts, and no MCP, SDK or skills.

- Graph API: `POST /api/graph/ontology/generate` (multipart: `files`, `simulation_requirement`, `project_name`, `additional_context`) — `api/graph.py:265-300`. Also `POST /build` (`project_id`, `graph_name`, `chunk_size`, `chunk_overlap`) — `:448`, `GET /task/<id>` — `:844`, `GET /data/<graph_id>` — `:879`, and project CRUD at `:132-213` [Verified]
- Simulation API: `create` (`project_id`, `graph_id`, `enable_twitter`, `enable_reddit`) — `api/simulation.py:195`. Then `prepare` (`use_llm_for_profiles`, `parallel_profile_count`) — `:389`, `prepare/status` — `:677`, `start` (`max_rounds`, `enable_graph_memory_update`, `platform`) — `:1501`, `stop` — `:1788`, `run-status`, `actions`, `timeline`, `agent-stats`, `posts`, `comments` — `:1865-2225`, `interview`, `interview/batch`, `interview/all`, `interview/history` — `:2304-2674`, `env-status`, `close-env` — `:2746-2811` [Verified]
- Report API: `generate`, `generate/status`, `GET /<id>`, `by-simulation`, `download`, `chat` (`simulation_id`, `message`, `chat_history`) — `api/report.py:32-688`. Streaming logs at `:877-1054`, and the raw tools `tools/search` and `tools/statistics` — `:1054-1102` [Verified]
- CLI: `scripts/run_parallel_simulation.py --config … [--twitter-only|--reddit-only] [--max-rounds N] [--no-wait]` — `run_parallel_simulation.py:1493-1521`. There are also single-platform scripts `run_twitter_simulation.py` and `run_reddit_simulation.py` [Verified]
- Scripts download endpoint uses an allow-list (no path traversal) — `api/simulation.py:1373-1400` [Verified]

## T6 — How it plugs into agents

In plain words: it does not plug into coding agents at all. It is a standalone app. You could only drive it through its HTTP API.

- No MCP server, no Claude Code/Codex hooks, no skills, no SDK package (`[tool.hatch.build.targets.wheel] packages=["app"]` exists but is not published) — `backend/pyproject.toml:50-51`. A repo-wide search found no MCP [Verified]
- The integration surface for omni-agent would be the REST sequence: `ontology/generate` → `build` → poll `task` → `simulation/create` → `prepare` → poll → `start` → poll `run-status` → `report/generate` → poll → `GET report` / `report/chat` / `simulation/interview`. See the T5 citations [Verified routes; sequence = Inference from the frontend flow]
- Internally, the "agents" are OASIS `SocialAgent`s built from the persona CSV/JSON via `generate_twitter_agent_graph` / `generate_reddit_agent_graph` — `run_parallel_simulation.py:1138-1142`, `:1329-1333` [Verified]

## T7 — LLM usage and cost

In plain words: every stage calls an OpenAI-compatible LLM, and the simulation stage dominates the cost (one call per active agent per round per platform). Zep Cloud also runs its own extraction LLM on every chunk and every batch of 5 actions. Nothing works offline: Zep Cloud is mandatory.

- One provider config: `LLM_API_KEY`, `LLM_BASE_URL`, `LLM_MODEL_NAME`, plus an optional `LLM_BOOST_*` used only for the Reddit simulation — `config.py:28-30`, `run_parallel_simulation.py:984-1037`, `:1320` [Verified]
- The JSON helper strips `<think>` blocks and falls back when `response_format` is unsupported — `utils/llm_client.py:27-70` [Verified]
- `REPORT_AGENT_MAX_TOOL_CALLS`, `…_MAX_REFLECTION_ROUNDS` and `…_TEMPERATURE` in Config are **not read anywhere**. The ReportAgent uses class constants `MAX_TOOL_CALLS_PER_SECTION=5` and `MAX_REFLECTION_ROUNDS=3`, and hard-codes temperature 0.5 and max_tokens 4096 — `config.py:59-61`, `report_agent.py:882-888`, `:1344-1348` [Verified grep]

| Stage | LLM calls (N = filtered entities) | Source |
|---|---|---|
| Ontology | 1 (≤2 attempts) | `ontology_generator.py:235-243` |
| Graph build | 0 MiroFish calls; Zep extraction per 500-char chunk | `graph_builder.py:470-493` |
| Personas | N (≤3 attempts each) + 2N Zep searches | `oasis_profile_generator.py:567-583`, `:411-413` |
| Config | 2 + ⌈N/15⌉ | `simulation_config_generator.py:276-327` |
| Simulation | ≈ Σ rounds × active agents × 2 platforms (≥1 call per LLMAction; OASIS may add tool turns) | `run_parallel_simulation.py:1228-1254` |
| Memory update | 0 LLM; 1 Zep episode per 5 non-idle actions | `zep_graph_memory_updater.py:228`, `:494` |
| Report | 1 outline + per section ≤5 ReACT + tool sub-calls (insight_forge 1; interview ≈ 1 select + 1 questions + ≤5 agents×2 platforms + 1 summary) | `report_agent.py:1216`, `:1326`; `zep_tools.py:1121`, `:1604`, `:1663`, `:1721` |

- **Worked example, N = 50 [Inference]:** ~1 + 50 + 6 ≈ 57 prep calls. Simulation with the fallback config (3–10 agents per hour, about 5 actually active on average after filtering) × 72 rounds × 2 ≈ **700 calls**, or ≈ 400 with the UI's recommended 40-round cap. Report ≈ 30–100 calls. **Total ≈ 0.5k–1k calls.** Each simulation call carries a ~2,000-char persona plus the feed, so input tokens dominate. With `qwen-plus` or `gpt-4o-mini` that is cents to a few dollars per run; frontier models would cost more than ten times as much. The README warns "High consumption, try simulations with fewer than 40 rounds first" — `README.md:120` [Docs]
- Concurrency cap: `semaphore=30` concurrent LLM requests per platform (60 total) — `run_parallel_simulation.py:1159`, `:1350` [Verified]

## T8 — Security, privacy, safety, legal, ethics

In plain words: do not expose it to a network as it is. It sends all your documents to two outside services, by default possibly to a China-hosted LLM endpoint. It deliberately generates realistic personas of real, named people.

- **No authentication** on any route. CORS is `origins:"*"` on `/api/*` — `backend/app/__init__.py:43`. It binds to `0.0.0.0:5001` by default — `run.py:40-45`. `SECRET_KEY` defaults to `'mirofish-secret-key'` — `config.py:21`. Anyone who can reach the port can spend your LLM and Zep credits and read all reports [Verified]
- Docker runs dev servers (`npm run dev`) and publishes ports 3000 and 5001 — `Dockerfile`, `docker-compose.yml` [Verified]
- Data egress: seed documents go to Zep Cloud (stored as episodes) and to the LLM provider. The recommended default endpoint is `dashscope.aliyuncs.com` — `.env.example:5`, `graph_builder.py:470-493` [Verified]. Using it on confidential product or customer documents needs a data-processing review [Inference]
- Upload limits: 50 MB, pdf/md/txt/markdown, with an extension allow-list — `config.py:36-38`, `api/graph.py:122`, `:329` [Verified]
- **Real-person simulation:** the persona prompt asks to "restore real-world facts as much as possible", including stance, catchphrases and "personal memory" of the event for named individuals (students, officials, journalists) — `oasis_profile_generator.py:718`, `:734-768`. The Wuhan University demo simulates a real public-opinion incident — `README.md:68-74`. This carries defamation, impersonation and manipulation risk, and the output can be misread as "what X will say" [Verified + Inference]
- Dual use: the tool is explicitly positioned to test "policies and public relations … at zero risk" — `README.md:38`. The same pipeline can be used to rehearse influence campaigns [Docs/Inference]
- The report prompt bans fabrication and forbids "own knowledge" — `report_agent.py:662-671`. This is a prompt rule, not an enforced check [Verified]
- The tool-result sanitizer strips fake `<tool_result>` blocks that the LLM writes itself — `report_agent.py:1144`, `backend/tests/test_report_tool_result_sanitizer.py` [Verified]

## T9 — Quality: tests, benchmarks, CI

In plain words: there is a reasonable set of unit tests for Zep and JSON robustness, but CI never runs them, and nothing tests whether the predictions are any good.

- 18 backend test files, 138 `def test` functions in total (including 2 root tests that only cover the **star-history script**) — `backend/tests/*.py`, `tests/test_local_star_*.py` [Verified count]
- What is covered: Zep contracts, batching, paging, retries, lifecycle and report/simulation barriers, LLM JSON parsing, ontology normalisation, profile field normalisation and platform profile formats — file names in `backend/tests/` [Verified]
- CI: only `docker-image.yml` (build and push the image on tags) and `update-star-history.yml` (500 lines). **No pytest job, no lint** — `.github/workflows/` [Verified]
- `scripts/validate_zep_cloud_integration.py` (904 lines) is a manual live-Zep smoke test [Verified exists]
- **Benchmarks and validation: none.** A grep for backtest, ground truth, accuracy, calibration or Brier across backend, frontend and READMEs returns nothing [Verified]. The README promises "Financial Prediction, Political News Prediction … coming soon" — `README.md:84` [Docs]
- Repo hygiene: a large share of repo activity is star-history tooling (`scripts/star_history.py` 1,489 lines, workflow, `.github/star-history/history.json`) [Verified]

## T10 — Operations

In plain words: you need Node 18+, Python 3.11–3.12, uv, a Zep Cloud key and an LLM key. Run `npm run setup:all && npm run dev`, or use Docker.

- Install: `npm run setup:all` (npm, then frontend npm, then `uv sync`). Run: `npm run dev` with frontend on :3000 and backend on :5001 — `README.md:130-163`, `package.json` scripts [Docs/Verified]
- Docker: `ghcr.io/666ghj/mirofish:latest`, with `./backend/uploads` mounted — `docker-compose.yml` [Verified]
- Required env: `LLM_API_KEY` and `ZEP_API_KEY`. `ZEP_API_URL` is explicitly rejected (Cloud only) — `config.py:64-72` [Verified]
- Heavy dependencies: camel-ai and camel-oasis pull in a large ML stack (see `backend/uv.lock`, 2,792 lines) [Verified size; contents not audited]
- Runtime footprint: two OASIS environments, each with SQLite and up to 30 concurrent LLM requests, plus Flask threads. They stay alive after the run until `close-env` — `run_parallel_simulation.py:1597-1640` [Verified]
- No install hooks (`preinstall`/`postinstall`) in `package.json` [Verified]

## T11 — What is new vs marketing

In plain words: what is genuinely new is the glue — documents to knowledge graph to personas to OASIS to a grounded report, plus "interviewing" agents after the run. "Predicting anything", "thousands of agents", "high-fidelity" and "God's-eye variable injection" are not supported by the code.

| Claim | Code reality | Tag |
|---|---|---|
| "Thousands of intelligent agents" `README.md:29` | One agent per Zep-extracted entity of the 10 designed types. Typical seed docs give tens to low hundreds, and each round activates a small random subset — `zep_entity_reader.py:221-273`, `run_parallel_simulation.py:1040-1080` | Verified / Inference on typical N |
| "Predicting anything" / "precisely deduce future trajectories" `README.md:7-9,29` | No validation, ensembles or uncertainty. A single stochastic run is narrated by an LLM — T2 step 13, T9 | Verified |
| "Long-term memory" | Agents use OASIS/CAMEL in-run memory. Zep is updated with actions but not read by agents — T2 step 9 | Verified / Inference |
| "Inject variables dynamically from a God's-eye view" `README.md:29` | The IPC supports only interview, batch_interview and close_env. There is no mid-run event injection; `scheduled_events=[]` — `simulation_ipc.py:25-29`, `simulation_config_generator.py:725` | Verified |
| "Dual-platform parallel simulation" | True: OASIS Twitter and Reddit via `asyncio.gather` — `run_parallel_simulation.py:1585` | Verified |
| "Auto-parse prediction requirements" | True: an LLM turns the NL requirement into ontology, time, event and agent configs — T2 steps 1 and 5 | Verified |
| Behavioural knobs (stance, influence, echo chamber, virality) | Generated and shown in the UI, **ignored by the simulator** — T2 step 8 | Verified |
| "GraphRAG" | Zep Cloud's graph plus cross-encoder search; MiroFish implements no GraphRAG itself — `zep_tools.py:486-491` | Verified |
| "Chat with any agent" | True: OASIS INTERVIEW action on the live environment — `run_parallel_simulation.py:317-340` | Verified |

- **What it can credibly do [Inference]:** generate a plausible spread of stakeholder voices and talking points about a seed event; surface arguments, objections and framings you had not considered; produce a qualitative "how might this play out" narrative grounded in a traceable action log; and let you question personas. **What it cannot credibly do:** quantitative forecasts (vote shares, prices, adoption rates), calibrated probabilities, or anything that depends on real population weights. Agents are entities from your document, not a representative sample, and the outcome is largely set by LLM-authored personas (which include stance) and LLM-authored opening posts.

## T12 — Weaknesses, limits, risks

In plain words: the results are hard to trust, hard to reproduce, and tied to a paid cloud service. The engineering is uneven: careful Zep retry code next to unused config knobs and dev-mode deployment.

- **No reproducibility:** `random.uniform`, `random.sample` and persona temperature 0.7 run with no seed control, and there is no multi-run aggregation — `run_parallel_simulation.py:1063-1080`, `oasis_profile_generator.py:581` [Verified]
- **Selection bias:** the agent population equals the entities named in the seed documents. A silent majority and the general public are not modelled unless the LLM invents a `Person` entity [Inference from `zep_entity_reader.py:221-273`]
- **Circularity:** persona prompts request a "stance" and "existing actions and reactions in the event", and the event-config LLM writes the opening posts. The simulation partly replays what the LLM already believes [Verified prompts `oasis_profile_generator.py:747-754`, `simulation_config_generator.py:689-705`; conclusion = Inference]
- **Dead configuration** (stance, influence and similar knobs) gives a false sense of control — T2 step 8 [Verified]
- **China-centric defaults:** UTC+8 daily rhythm, Chinese prompts throughout, `country` returned in Chinese ("中国") — `simulation_config_generator.py:86-111`, `oasis_profile_generator.py:758` [Verified]
- **Vendor lock-in:** Zep Cloud only, self-hosted URLs rejected — `config.py:71-72` [Verified]
- **Ops fragility:** in-memory task registry, file-polling IPC with 60–180 s timeouts, environments kept alive indefinitely, dev-mode servers — `models/task.py:56-71`, `simulation_ipc.py:117-187` [Verified]
- **Cost scales** with rounds × active agents × 2, plus Zep episode volume — T7 [Inference]
- **Report grounding is prompt-enforced only.** The LLM decides what to quote, and no citation check exists — `report_agent.py:643-671` [Verified]
- AGPL-3.0 limits commercial embedding (see T13)

## T13 — How it fits into omni-agent

In plain words: use it, or better a clean re-build of its idea, as an **optional Discover-phase tool that "rehearses" how stakeholders might react** to a feature, launch, pricing change or policy before you build. Always label the output as qualitative and exploratory. Do not copy MiroFish code into the MIT codebase. Either run it untouched as a separate service, or re-implement the pipeline on top of OASIS, which is Apache-2.0.

**Agile role**
- **Discover:** "stakeholder / market reaction rehearsal". Feed a PRD, changelog or announcement draft plus a question ("How will power users and competitors react to removing the free tier?"). Get back representative voices, objections, framings and an interviewable panel.
- **Release / Operate (secondary):** pre-test release-note or incident-comms wording on a persona panel.
- **Learn:** compare a past rehearsal against real feedback. This is where omni-agent could add the **validation loop MiroFish lacks**: log predicted objections and score them against support tickets and reviews.

**Options**

| Option | What | License consequence | Effort |
|---|---|---|---|
| A. Sidecar (recommended to try first) | Run **unmodified** MiroFish in its own container; omni-agent ships an MIT MCP adapter (`rehearse_reaction`) that only calls its REST API | AGPL-3.0 does not extend to a separate program talking at arm's length over HTTP (FSF "aggregate / separate programs" reading) [Inference]. Running it unmodified for yourself triggers nothing. If you **modify** MiroFish and let users interact with it over a network, **§13** requires offering those users the Corresponding Source of the modified MiroFish (only MiroFish, not omni-agent). If you **distribute** the image, you must also pass on AGPL source | ~3–5 days: adapter, polling, budget guard, auth proxy in front |
| B. Clean-room port (recommended long-term) | Re-implement the *ideas* (ontology → entities → personas → OASIS run → grounded ReACT report → interviews) in omni-agent from this report, **without copying code or prompts verbatim** | Ideas and algorithms are not covered by copyright; OASIS is **Apache-2.0** (github.com/camel-ai/oasis), so it is MIT-compatible with NOTICE preserved. Swap Zep Cloud for a local graph (e.g. Graphiti or omni-agent's own memory) [Inference] | ~3–5 weeks for an MVP; the best chance to add seeds, ensembles, a population sampler and a validation log |
| C. Depend as a library (import `app.*`) | Import MiroFish modules into the omni-agent process | Creates a combined work, so omni-agent's distributed or network-served version must be AGPL. **Avoid** | n/a |
| D. Skip | — | — | 0; valid if Discover research stays human-led |

- Recommendation: **A for a 1-week experiment** (behind auth, with dummy or non-confidential seeds and a capped `max_rounds`). **B only if the experiment shows useful signal**, and if so fix the gaps: wire stance and influence into behaviour, add N-run ensembles with seeds, support a population sampler beyond named entities, record cost, and score past rehearsals against real outcomes [Inference]

**Proposed interface contract (MCP tool, MIT adapter)**
```
rehearse_reaction(
  seed_docs: [path|url],            # ≤50MB, pdf/md/txt
  question: str,                    # → simulation_requirement
  platforms: ["twitter","reddit"],  # → enable_twitter/enable_reddit
  max_rounds: int = 20,             # → /start max_rounds (hard cap)
  max_llm_calls: int = 1500,        # adapter-side budget guard (MiroFish has none)
  interview: [str] = [],            # optional follow-up questions to panel
) -> {
  report_md: str, sections: [{title, md}],
  panel: [{agent_id, name, type, bio}],
  actions_log: path (jsonl), interviews: [{agent, q, a, platform}],
  provenance: {mirofish_commit, llm_model, zep_graph_id, rounds_run, agents},
  caveats: "Qualitative scenario exploration; not a calibrated forecast; single stochastic run."
}
```
- Adapter flow: `POST /api/graph/ontology/generate` → `POST /api/graph/build` → poll `/api/graph/task/<id>` → `POST /api/simulation/create` → `/prepare` → poll `/prepare/status` → `/start {max_rounds, enable_graph_memory_update}` → poll `/<id>/run-status` → `POST /api/report/generate` → poll `/generate/status` → `GET /api/report/<id>` → optional `POST /api/simulation/interview/batch` → `POST /api/simulation/close-env`. See the T5 citations [Verified routes]

## T14 — Open questions

In plain words: the main unknowns are how good the output really is, what a run costs in practice, and what OASIS does inside each agent step.

- How many LLM turns does one OASIS `LLMAction` cost in camel-oasis 0.2.5 (one call, or one call plus tool round-trips)? This decides the real cost. Needs reading OASIS `SocialAgent.perform_action_by_llm` or a metered run [Open]
- How much does OASIS's recommendation system (interest- or hot-score-based) shape the outcome, and are its parameters reachable from MiroFish? MiroFish's own platform weights are unused [Open]
- What is the Zep Cloud credit usage per run (seed chunks plus action episodes), and does the free tier cover a 40-round, 50-agent run as the README implies? [Open]
- Would running the same seed several times give stable themes? No evidence exists; a 5-seed ensemble experiment would answer it [Open]
- Is there any published evaluation of MiroFish forecasts (the Wuhan University case or "Dream of the Red Chamber" ending) against reality? None found in the repo [Open]
- Can MiroFish plus Zep handle non-Chinese, non-UTC+8 scenarios well, given the prompts are in Chinese with a language instruction appended? [Open]
- Legal: what is MiroFish's position on simulating identifiable private individuals (GDPR/PIPL)? There is no policy text in the repo [Open]
