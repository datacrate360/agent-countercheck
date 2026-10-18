# graphify — research report

| Field | Value |
|---|---|
| Repo | Graphify-Labs/graphify (PyPI `graphifyy`, CLI `graphify`, MCP entry `graphify-mcp`) |
| Commit | `86c57be8204673c2a1b9849a3292a7dd3ab373d8`, branch `v8`, 2026-10-16, "chore(release): 0.9.92" (shallow clone, no history) |
| License | Apache-2.0 (current). `LICENSE-MIT` covers contributions made **before** the 0.9.25 relicensing (2026-07-22) |
| Size | 1,079 tracked files; `graphify/` = 233 files / ~86k lines of Python in `graphify/*.py` + `graphify/extractors/*.py`; 431 test files (~6,705 `def test_`) |
| Method | Static reading only. Nothing was installed, built or run. Line numbers are from this commit. CODE WINS over docs. |
| Files read (key) | README.md, ARCHITECTURE.md, BENCHMARKS.md, AGENTS.md, NOTICE, CHANGELOG.md (license entry), pyproject.toml, Dockerfile, .github/workflows/ci.yml + release-graph.yml, graphify/{extract.py, extractors/engine.py, extractors/models.py, extractors/resolution.py, symbol_resolution.py, build.py, validate.py, cluster.py, analyze.py (signatures), export.py, serve.py, benchmark.py, cache.py, watch.py, cli.py, install.py, hooks.py, llm.py, detect.py, security.py, querylog.py, reflect.py, affected.py, prs.py}, graphify/skill.md, graphify/skills/claude/references/extraction-spec.md, worked/karpathy-repos/review.md, docs/how-it-works.md |

## Executive summary

- graphify is a **Python CLI + library, wrapped in an assistant "skill"**. It turns a folder into a NetworkX graph and writes `graphify-out/graph.json`, `GRAPH_REPORT.md` and `graph.html`. Agents then call `graphify query/path/explain` or an MCP server instead of grepping.
- **Code path:** 115 file suffixes go to 55 extractor functions. Most of these use one generic tree-sitter **tree walker** driven by a per-language `LanguageConfig` (sets of node types). There are **no tree-sitter `.scm` queries**. After that come about 30 cross-file resolver passes. This code path is deterministic and makes no LLM calls.
- **INFERRED is decided by rules in the code, not by a model.** A cross-file call that resolves to exactly one candidate becomes `INFERRED` (0.85). If an import edge backs it, it is promoted to `EXTRACTED` (1.0). Function references passed as values become `indirect_call` INFERRED (0.85). Imported names that are used become `uses` INFERRED (0.95). JS/TS calls with no import evidence are dropped.
- **Docs, papers and images** go to the host agent's own model, as parallel subagents in chunks of 20–25 files, using a fixed prompt (`extraction-spec.md`). Headless runs use one of 9 backends instead (Gemini, Kimi, Claude, OpenAI, DeepSeek, Azure, Bedrock, Ollama, `claude -p`).
- **Communities:** Leiden runs only if the optional `[leiden]` extra is installed. **The default install falls back to networkx Louvain** (seed 42). Oversized communities (>25% of nodes) and low-cohesion ones (cohesion <0.05) are split again. Labels come from the highest-degree node in each community, with no LLM.
- **Query** is plain lexical ranking. Each node is scored by exact/prefix/substring match on label, source path, rationale and attributes, weighted by IDF. A gap-ratio rule picks up to 3 seed nodes, then a BFS/DFS (depth 3, hub nodes are not expanded) renders text up to a token budget (default 2000 tokens at about 3 chars/token). The open-source repo contains no embeddings.
- **The "80% fewer tokens" claim is Graphify Cloud marketing.** It does not appear in BENCHMARKS.md and no harness in this repo measures it. The in-repo `graphify benchmark` estimates a ratio (corpus words×1.33 ÷ (chars of the query answer ÷4)) over 5 canned questions. It measures size, not whether the answer is correct. The headline 71.5× comes from one 52-file corpus.
- **Integration:** `graphify install` copies a platform-specific SKILL.md into about 20 assistants and adds always-on instruction blocks (CLAUDE.md, AGENTS.md, …). On Claude Code it also adds **PreToolUse hooks** on Bash|Grep and Read|Glob that inject "MANDATORY: run graphify query" text. The MCP server has 10 tools and 6 resources over stdio or HTTP.
- **For omni-agent:** use it as the **Learn/Discover "code-map" layer**, called over MCP or the CLI behind a thin adapter. Do not vendor ~86k lines of Python. Apache-2.0 is compatible with an MIT host if NOTICE is kept. Port two ideas directly: (1) the EXTRACTED/INFERRED/AMBIGUOUS rubric with discrete scores, and (2) prompt-fingerprinted semantic caching.

---

## T1 — Identity

In plain words: graphify is an open-source tool from a YC S26 startup (Graphify Labs). It builds a knowledge graph of a project so coding agents can look things up instead of re-reading files. It is a Python package, licensed Apache-2.0, very actively developed (version 0.9.92), with a paid "Graphify Cloud" upsell.

- Package `graphifyy` 0.9.92. Requires Python >=3.10. Console scripts are `graphify = graphify.__main__:main` and `graphify-mcp = graphify.serve:_main`. `pyproject.toml:9-10`, `pyproject.toml` `[project.scripts]` [Verified]
- Core deps: `networkx`, `numpy`, `rapidfuzz`, `tree-sitter>=0.25,<0.26`, plus about 25 bundled grammar wheels (python, js, ts, go, rust, java, groovy, c, cpp, ruby, c#, kotlin, scala, php, swift, lua, zig, powershell, elixir, objc, julia, verilog, fortran, bash, json). More than 30 optional extras (`mcp`, `leiden`, `pdf`, `video`, `office`, per-LLM backends, `sql`, `terraform`, `ocaml`, …). `pyproject.toml` dependencies / `[project.optional-dependencies]` [Verified]
- License: `license = "Apache-2.0"`, `license-files = ["LICENSE","LICENSE-MIT","NOTICE"]`. NOTICE says: "Portions … contributed under the MIT License prior to the relicensing … remain available under those terms". CHANGELOG 0.9.25 (2026-07-22) records the switch from MIT to Apache-2.0, which added a patent grant and a patent-retaliation clause. `NOTICE:4-8`, `CHANGELOG.md:928-930` [Verified]
- "graphify" and the logo are claimed as marks of Graphify Labs. Community projects must state they are not affiliated. `README.md:489` [Docs]
- Upsell: Graphify Cloud (app.graphify.com, 14-day trial) is advertised with "80% fewer tokens", "formal verification", cross-repo graphs, and Sentry/Jira connectors. None of these features exist in this repo. `README.md:31,41,126,448-459` [Docs]. The CLI prints a one-time Cloud call-to-action and writes a `.cloud-cta-shown` marker file. `graphify/cli.py:130-142` [Verified]
- Maturity: there is a test-count gate and a doc-drift test (`tests/test_architecture_doc.py` imports every symbol named in ARCHITECTURE). Issue numbers above #4400 appear in comments, which signals heavy churn and a large user base. `ARCHITECTURE.md:15`, `graphify/cache.py:40` [Verified/Inference]

## T2 — Core mechanism (step by step)

In plain words: first it sorts files into code, docs, papers, images and video. Code is parsed locally with tree-sitter into functions, classes and imports. A second set of passes then links names across files and labels each link as directly read (EXTRACTED) or deduced (INFERRED). Non-code files are read by an LLM using a fixed JSON prompt. Everything is merged into one graph, clustered into communities, and written out.

```
 detect(root)                       graphify/detect.py  (CODE/DOC/PAPER/IMAGE/VIDEO ext sets :48-53, sensitive-file skip :133,:287)
   │  .graphify_detect.json
   ├──────────── code ───────────────────────────┐            ┌──────── docs / papers / images ─────────┐
   ▼                                             │            ▼                                          │
 extract(paths, root=, cache_root=)  extract.py:9136          check_semantic_cache(prompt_file=SPEC)  cache.py:1375
   Phase 1: per-file AST cache hit?  :9276-9286               chunk 20-25 files (1 image/chunk)   skill.md:266
   Phase 2: _extract_parallel / _sequential :9292-9323        N parallel general-purpose subagents  skill.md:268-292
     _get_extractor(path) :8791 → _DISPATCH :8540-8655          prompt = references/extraction-spec.md (verbatim)
       → extract_<lang> → _extract_generic(LanguageConfig)      OR headless llm.extract_corpus_parallel  llm.py:2692
          engine.py:5739 (tree-sitter Parser, recursive walk)     (<untrusted_source> wrapping, llm.py:598-630)
          in-file call-graph pass engine.py:8354                save_semantic_cache  cache.py:1521
          raw_calls[] saved for cross-file pass                 .graphify_semantic.json (_origin="semantic")
   Cross-file passes (all languages):                                     │
     _disambiguate_colliding_node_ids :10079                              │
     _resolve_cross_file_imports (py "uses" INFERRED .95) :10133          │
     _rewire_unique_stub_nodes :10154                                     │
     shared raw-call resolution :10231-10724 (INFERRED .85 / EXTRACTED)   │
     run_language_resolvers (member calls per lang) :10296                │
     relativize ids/source_file, stamp _origin="ast" :10792-10970         │
   .graphify_ast.json                                                     │
   └──────────────────────► merge (AST first, semantic dedup by id) skill.md:374-418
                                     ▼
                     build_from_json(extraction, directed=False)  build.py:1147  (nx.Graph, 1 edge/pair :1313)
                                     ▼
                     cluster(G)  cluster.py:233  (Leiden→Louvain fallback, split >25% / cohesion<0.05)
                                     ▼
          analyze.god_nodes / surprising_connections / suggest_questions / find_import_cycles  analyze.py:112,151,475,720
                                     ▼
          report.generate → GRAPH_REPORT.md ; export.to_json → graph.json (schema_version=1) export.py:287,476 ; to_html
                                     ▼
          query/path/explain (CLI)  ·  MCP stdio/HTTP serve.py:2129  ·  PreToolUse hook-guard cli.py:2844
```

1. **Detect.** Files are classified by extension. Code = 100+ suffixes. Docs = `.md .mdx .qmd .txt .rst .html .yaml .yml .j2 …`. Paper = `.pdf`. Image = `.png .jpg .gif .webp .svg`. Video/audio go to faster-whisper. Credential-like paths (`id_rsa`, `.env` non-templates, `secrets/` dirs) are skipped silently. The tool warns when the corpus is under 50k words ("you may not need a graph") or over 500k words / 500 files (cost). `graphify/detect.py:48-57,119-151,287` [Verified]
2. **Extractor dispatch.** `_get_extractor` routes MCP config files and package manifests by filename first. It then applies `.graphifyrc` `language.<ext>=` remaps. It sniffs `.h` (ObjC vs C++ vs C) and `.m` (ObjC vs MATLAB, where MATLAB gets no extractor). Extensionless files are routed by shebang. Otherwise it uses `_DISPATCH`: 115 suffixes mapped to 55 `extract_*` functions. `graphify/extract.py:8540-8655,8791-8840` [Verified]
3. **AST walk, not tree-sitter queries.** `LanguageConfig` declares `class_types`, `function_types`, `import_types`, `call_types`, accessor fields, `function_boundary_types`, an `import_handler` and an `extra_walk_fn`. Example: Python = `class_definition` / `function_definition` / `import_statement|import_from_statement` / `call` / `attribute`. `_extract_generic` loads the grammar via `importlib`, parses with `tree_sitter.Parser`, and walks the tree recursively. A repo-wide grep found no `Query(`/`.captures(`. `graphify/extractors/models.py:14-57`, `graphify/extract.py:1328-1340`, `graphify/extractors/engine.py:5739-5786` [Verified]. Many languages have their own module (`extractors/rust.py`, `go.py`, `sql.py`, `markdown.py`, `cobol.py`, `robot.py` …). Some of these are not tree-sitter based (Pascal falls back to regex; Robot uses `robot.api`). `pyproject.toml` comments on pascal/robot extras [Verified]
4. **Node IDs.** IDs are deterministic: `{repo-relative path with segments joined by _}_{symbol}`, lowercased, `[a-z0-9_]`. The semantic prompt must reproduce the same ID so that AST and LLM nodes merge. Collisions get a path-based salt in `_disambiguate_colliding_node_ids`. `skills/claude/references/extraction-spec.md:61`, `graphify/extractors/resolution.py:1060`, `graphify/extract.py:10079` [Verified]
5. **In-file call graph.** After the walk, `label_to_nid` maps (case-sensitive plus a case-insensitive map for PHP) bind callee names to local definitions as `calls` edges. A function named as a value (a callback argument or a dispatch-table entry) becomes `indirect_call`, `confidence: INFERRED`, `confidence_score: 0.85`. The comment explains why: "that it is ever INVOKED is the inference". Calls that cannot be resolved are kept as `raw_calls` for the cross-file pass. `graphify/extractors/engine.py:8354-8396,8530-8556` [Verified]
6. **Cross-file Python import resolution.** Pass 1 builds a per-stem map from entity names to IDs. Pass 2 adds an edge from each importing-file symbol that references an imported name, as `uses` INFERRED with score 0.95 ("named cross-file reference"). Example: `DigestAuth --uses--> Response`. `graphify/extractors/resolution.py:2911-2930,3133-3148` [Verified]
7. **Shared cross-file raw-call resolution.** This is how most INFERRED `calls` edges are produced. It runs in all languages. `extract.py:10231-10724`:
   - It builds `global_label_to_nids`. Rationale nodes are skipped, and on incremental runs `resolution_context_nodes` from unchanged files are added to the index. `:10231-10260` [Verified]
   - Ambiguous bare names (`log`, `execute`) are skipped unless shared tie-breakers leave exactly one candidate (prefer non-test code, then path proximity). This protects god-node rankings from spurious hubs. `graphify/symbol_resolution.py:328-399` (same logic) [Verified]
   - Source-less targets (external stubs) never receive name-only `calls` edges. `:10644-10649` [Verified]
   - **JS/TS direct calls without import evidence are dropped**, because JS modules have no implicit cross-module scope (#1659). C/C++/Ruby keep single-candidate resolution. `:10688-10699` [Verified]
   - **Promotion rule:** if an import edge goes from the caller's file to the callee symbol or its file, the edge is `EXTRACTED`/1.0. Otherwise it is `INFERRED`/0.85. `:10700-10724` [Verified]
   - The Python import-guided resolver emits `EXTRACTED` `calls` with `metadata.resolver = "python_import_guided"`. `graphify/symbol_resolution.py:239-325` [Verified]
8. **Language-specific member-call resolvers.** Receiver-type tables, then `obj.method()` bound per language: Python, TS, C++, C#, PHP, Java, ObjC, Kotlin, Swift, Go, Rust. Each emits `EXTRACTED` when the type is qualified and `INFERRED` otherwise. `graphify/extract.py:4682,5307,5473,5789,6468` (`"confidence": "EXTRACTED" if type_qualified else "INFERRED"`), `graphify/resolver_registry.py:59` [Verified]
9. **Rationale nodes.** Python `# NOTE:`/`# WHY:` comments and docstrings become `rationale` nodes linked with `rationale_for`. `graphify/extract.py:1770` (`_extract_python_rationale`), `:2340` (JS) [Verified]
10. **Semantic pass (docs/papers/images).** The skill chunks uncached files into groups of 20–25, grouped by directory, one image per chunk. It requires `subagent_type="general-purpose"` because Explore agents cannot write the chunk files. All agents are dispatched in one message. Each writes `.graphify_chunk_NN.json`. If more than half the chunks fail, the run aborts. `graphify/skill.md:207-302` [Verified]
11. **The semantic prompt** (`extraction-spec.md`, passed verbatim to subagents):
    - EXTRACTED = explicit (import/call/citation/"see §3.2"); INFERRED = reasonable inference; AMBIGUOUS = flag, do not omit.
    - Rationale goes into a `rationale` attribute on the node, not into separate nodes.
    - `file_type` ∈ {code, document, paper, image, rationale, concept}.
    - Cross-language `calls` are forbidden.
    - Image guidance: "use vision … do not just OCR".
    - Add `semantically_similar_to` (INFERRED 0.6–0.95) only when the link is non-obvious; at most 3 hyperedges per chunk.
    - INFERRED scores must come from the **discrete** set {0.95, 0.85, 0.75, 0.65, 0.55}, never 0.5. AMBIGUOUS = 0.1–0.3.
    - Source: `graphify/skills/claude/references/extraction-spec.md:12-64` [Verified]
    - The headless version is `_EXTRACTION_SYSTEM`, with a stricter `DEEP_MODE` suffix and a SECURITY paragraph about `<untrusted_source>` blocks. `graphify/llm.py:502-558` [Verified]
12. **Merge.** AST nodes come first. Semantic nodes are deduplicated by ID, and the semantic side is stamped `_origin: "semantic"` so that `graphify update` does not delete them. `graphify/skill.md:374-418`, `graphify/build.py:45` (`_is_ast_tier`) [Verified]
13. **Build.** `build_from_json` produces `nx.Graph()` by default, or `nx.DiGraph()` with `--directed`. These are **simple** graphs, so only one edge survives per node pair. The survivor is chosen by confidence rank (EXTRACTED 3 > INFERRED 2 > AMBIGUOUS 1), then by whether it has a `source_location`, then by score. Specific relations are never downgraded to the generic ones `references|uses|mentions`. `graphify/build.py:161,172,1313,1755-1790` [Verified]
14. **Cluster, analyze, report, export.** See T4 and below. `graphify/cluster.py:233`, `graphify/analyze.py:112-720`, `graphify/export.py:287` [Verified]

**Community detection (detail):**
- `_partition` copies the graph with nodes and edges sorted canonically, so results do not depend on hash order (measured as 70 vs 69 communities under different PYTHONHASHSEED values before this fix). It then tries `graspologic_native.leiden` directly (skipping a 7–19 s graspologic import), then `graspologic.partition.leiden(random_seed=42, trials=1)`, then **`nx.community.louvain_communities(seed=42, threshold=1e-4, max_level=10)`**. `graphify/cluster.py:23-60,106-177` [Verified]
- `cluster()` scales AMBIGUOUS edge weights by 0.5 (`GRAPHIFY_AMBIGUOUS_SCALE`). It can exclude hubs above a degree percentile and re-attach them by majority vote of neighbors. Isolates become single-node communities. Communities larger than max(10, 25% of nodes) get a second partition pass, as do communities of 50 or more nodes with cohesion <0.05. IDs are re-indexed by size, descending. `graphify/cluster.py:179-182,233-392` [Verified]
- `cohesion_score` = intra-community edges ÷ n(n−1)/2, excluding self-loops. `graphify/cluster.py:412-427` [Verified]
- Labels: `label_communities_by_hub` uses the label of the highest-degree member. An LLM or host-agent naming pass (skill Step 5) may override these. Community IDs are re-mapped to the previous run's IDs for stability. `graphify/cluster.py:185-209,434`, `graphify/cli.py:2421-2466`, `graphify/skill.md:509` [Verified]

**Query scoping and ranking (detail):**
- `_query_graph_text(question, mode=bfs|dfs, depth=3, token_budget=2000, context_filters, seeds)`. Explicit `seeds` let an external ranker (the comment mentions embeddings or grep) choose the start nodes. `graphify/serve.py:1413-1472` [Verified]
- `_score_query`, one pass per node over trigram-prefiltered candidates:
  - Full-query exact match on label/ID: `1000×10×idf_max`. Prefix match: `100×10`.
  - Per term: exact 1000, prefix 100, substring 1, source path 0.5, rationale 0.75, attributes 0.75, each × IDF.
  - Exact and prefix tiers are scaled by the square of the fraction of query terms matched.
  - Source: `graphify/serve.py:334-346,575-760` [Verified]
- `_pick_seeds(max_k=3, gap_ratio=0.2)`: candidates are dropped once their score falls below 20% of the top score. At least one seed per query term is guaranteed. Relational verbs ("calls", "uses") are dropped from that guarantee. Seeds are deduplicated by label. `graphify/serve.py:787-955,1455-1465` [Verified]
- `_bfs`: nodes with degree ≥ max(50, p99 degree) are never expanded as transit unless they are seeds. Induced edges are completed afterwards. `graphify/serve.py:1062-1090` [Verified]
- `_subgraph_to_text`: seeds come first, then other nodes sorted by (hop distance, −degree, id). Output is cut at `token_budget×3` chars. `graphify/serve.py:1120-1160` [Verified]
- Name lookups for `explain`/`path` **refuse ambiguous names** (exit code 3) instead of guessing. `README.md:405`, `graphify/serve.py:1733-1840` [Verified/Docs]

## T3 — Architecture and process model

In plain words: it is a set of single-purpose Python modules chained into a pipeline, and they pass plain dicts between them. There is no daemon unless you ask for one. The agent drives it by running shell snippets from the skill file, and optional pieces (watcher, MCP server, git hooks) run as separate processes.

- Pipeline: `detect → extract → build → cluster → analyze → report → export`. Modules exchange dicts and NetworkX graphs. Some extractor state is module-global (scan-root caches are cleared each run). `ARCHITECTURE.md:7-11`, `graphify/extract.py:9186-9200` [Verified]
- The **skill is the orchestrator** when running inside an assistant. `skill.md` holds bash and `python -c` blocks that run steps 0–9. It writes sidecar files (`.graphify_python`, `.graphify_detect.json`, `.graphify_ast.json`, `.graphify_semantic.json`, `.graphify_extract.json`, `.graphify_root`) into `graphify-out/`. `graphify/skill.md:65-120,178-205,374-418,584-614` [Verified]
- Headless: `graphify extract` (`cli.py:3642`) does the same work in-process with the LLM backends in `llm.py`. `graphify update` re-extracts code only, through `watch._rebuild_code` (no LLM), and holds a per-repo lock. `graphify/cli.py:2779-2838`, `graphify/watch.py:188,1677` [Verified]
- Parallelism: `ProcessPoolExecutor` for AST extraction when uncached files exceed a threshold (`GRAPHIFY_MAX_WORKERS`). Assistant subagents handle the semantic side. `graphify/extract.py:8949,9162-9165` [Verified]
- Long-lived processes, all optional:
  - `graphify watch` (watchdog, 3 s debounce): code changes trigger an AST rebuild; doc changes only set a flag saying an LLM pass is needed. `graphify/watch.py:2700-2746` [Verified]
  - git hooks: post-commit, post-checkout, a merge driver for `graph.json`, and rewrite hooks, all launched detached. `graphify/hooks.py:8-15,384,425,980` [Verified]
  - MCP server over stdio or Streamable HTTP, with hot reload of graph.json keyed on mtime and size. `graphify/serve.py:185,2129-2135,2743,2901` [Verified]

## T4 — Data model, storage, schemas

In plain words: everything lives in a `graphify-out/` folder. The main file is `graph.json`, NetworkX "node-link" JSON with a schema version of 1. Nodes are code symbols, files, concepts and documents. Edges carry a relation name, a confidence tag, a numeric score and the source line.

- **Extraction contract** (validated before build): node requires `id, label, file_type, source_file`. Edge requires `source, target, relation, confidence, source_file`. `file_type` ∈ {code, document, paper, image, rationale, concept}. `confidence` ∈ {EXTRACTED, INFERRED, AMBIGUOUS}. `graphify/validate.py:4-7` [Verified]
- Optional node fields: `source_location` ("L42"), `source_url`, `captured_at`, `author`, `contributor`, `rationale`, `_origin` (ast|semantic), `metadata`, plus markers such as `_callable`/`_callable_class`. Optional edge fields: `confidence_score`, `weight`, `context` (call/argument/constructor…), `type_only`, `metadata.resolver`. `extraction-spec.md:64`, `graphify/llm.py:528-531`, `graphify/symbol_resolution.py:300-322` [Verified]
- **Relations** emitted by AST code (counted from `add_edge` and literal sites): references, contains, calls, defines, method, imports, imports_from, inherits, case_of, implements, extends, uses, triggers, reads_from, navigates, rationale_for, embeds, indirect_call, includes, re_exports, instantiates, mixes_in, dynamic_import, depends_on, crate_depends_on, listened_by, bound_to, … The semantic vocabulary adds cites, conceptually_related_to, shares_data_with, semantically_similar_to. Hyperedges use participate_in|implement|form. Repo-wide grep; `extraction-spec.md:64`, `graphify/affected.py` DEFAULT_AFFECTED_RELATIONS [Verified]
- **graph.json** = `networkx.node_link_data(G, edges="links")`:
  - Each node additionally gets `community`, `community_name` and `norm_label`.
  - Top level: `hyperedges`, and `built_at_commit` (git HEAD of the target repo).
  - `graph.schema_version = GRAPH_SCHEMA_VERSION = 1` and `graph.graphify_version`.
  - Nodes and links are sorted with canonical key order, and the file is written atomically.
  - Source: `graphify/export.py:176,287,370-383,440-487` [Verified]
- Edge direction: the file says `"directed": false`, but **link order encodes direction**. Older files store the direction in `_src`/`_tgt`. A naive `node_link_graph` load loses direction; use `paths.load_node_link_graph`. `ARCHITECTURE.md:67-71`, `graphify/export.py:415-425` [Verified/Docs]
- **Shrink guard:** `to_json` refuses to overwrite an existing graph.json with a smaller graph unless `force=True`. This protects against partial or failed runs. `graphify/export.py:290-330` [Verified]
- Simple-graph limitation: parallel relations between the same pair collapse to one edge (T2 step 13). `diagnostics.py` exists to assess "MultiDiGraph readiness". `graphify/build.py:1313`, `graphify/diagnostics.py:1` [Verified]
- **Caches** (`graphify-out/cache/`):
  - `ast/v{extractor}-s{schema=17}-g{grammar sha256}/`: stale dirs are cleaned up, and keys = sha256(content + relative path), with a stat (size, mtime_ns) fast path.
  - `semantic/p{prompt_fp}/` and `semantic-deep/`: entries are keyed by a 12-hex sha256 of the extraction prompt, so a changed prompt invalidates only its own entries. These are deliberately *not* versioned by graphify release, because re-extraction costs LLM calls.
  - Markdown hashes exclude YAML frontmatter.
  - Source: `graphify/cache.py:40,48-77,123-162,511-600,1015-1046` [Verified]
- Other artifacts: `manifest.json` (for `--update`), `GRAPH_REPORT.md`, `graph.html`, an optional `wiki/` (one article per community), an Obsidian vault, `graph.svg`, GraphML, Cypher (Neo4j/FalkorDB), callflow HTML, and `memory/` + a lessons file from `save-result`/`reflect`. `ARCHITECTURE.md:24-27`, `graphify/detect.py:37`, `graphify/reflect.py:1-15` [Verified]

## T5 — Interfaces (exact names)

In plain words: there is a CLI with about 40 subcommands, a `/graphify` slash-command skill, an MCP server with 10 tools, and a Python API. The everyday commands are `query`, `path`, `explain` and `update`.

- **Slash skill:** `/graphify [path|github-url…] [--mode deep] [--update] [--directed] [--cluster-only] [--resolution N] [--exclude-hubs P] [--no-viz] [--svg] [--graphml] [--neo4j|--neo4j-push URI] [--falkordb|--falkordb-push URI] [--mcp] [--watch] [--wiki] [--obsidian]`, plus `/graphify add <url> [--author] [--contributor]` and `/graphify query|path|explain`. On Codex the prefix is `$graphify`. `graphify/skill.md:10-44`, `README.md:257,340-366` [Verified]
- **CLI subcommands** (dispatcher `graphify/cli.py`): `install`, `uninstall`, `status`, `query`, `path`, `explain`, `affected`, `god-nodes`, `save-result`, `reflect`, `diagnose`, `add`, `watch`, `label`/`cluster-only`, `update [--force] [--no-cluster]`, `hook install|…`, `hook-check`, `hook-guard <search|read> [--strict]`, `check-update`, `tree`, `merge-driver`, `merge-graphs`, `clone`, `export <html|obsidian|wiki|svg|graphml|falkordb|callflow-html>`, `benchmark`, `global add|remove|list|path`, `extract`, `cache-check`, `merge-chunks`, `merge-semantic`, `prs [N] [--triage] [--conflicts] [--worktrees]`, `provider list|show|add|remove`, plus `<platform> install` variants. `graphify/cli.py:1394-5339` (cmd== sites) [Verified]
- Query exit codes: 0 = answer, 3 = ambiguous name, 4 = no match. `README.md:405` [Docs]
- **MCP tools** (`graphify-mcp` or `python -m graphify.serve <graph.json> [--transport http --host --port --api-key]`):
  - `query_graph(question, mode, depth≤6, token_budget=2000, context_filter[])`
  - `get_node(label|node_id)`
  - `get_neighbors(label, relation_filter, token_budget)`
  - `get_community(community_id)`
  - `god_nodes(top_n, exclude_hubs_percentile)`
  - `graph_stats()`
  - `shortest_path(source, target, max_hops=8, undirected)`
  - `list_prs`, `get_pr_impact(pr_number)`, `triage_prs`
  - Every tool also accepts an optional `project_path` (multi-project).
  - Source: `graphify/serve.py:2202-2355,2359-2380` [Verified]
- **MCP resources:** `graphify://report`, `://stats`, `://god-nodes`, `://surprises`, `://audit`, `://questions`. `graphify/serve.py:2615-2620` [Verified]
- **Python API:** `detect(root)`, `extract(paths, *, root, cache_root, parallel, max_workers, resolution_context_nodes, resolution_context_edges)`, `collect_files`, `build_from_json(extraction, directed, root, dedup)`, `build`, `cluster(G, resolution, exclude_hubs_percentile, ambiguous_scale)`, `score_all`, `god_nodes`, `surprising_connections`, `suggest_questions`, `find_import_cycles`, `graph_diff`, `report.generate`, `export.to_json/to_html/to_obsidian/to_svg/to_graphml/to_canvas/to_cypher`, `serve(graph_path)`, `serve_http`, `watch`, `run_benchmark`. `ARCHITECTURE.md:17-34` (drift-tested), `graphify/extract.py:9136-9146` [Verified]
- Config: `.graphifyrc` (`language.<ext>=`, `viz_node_limit`), `.graphifyignore`, `.gitignore` honored, many `GRAPHIFY_*` env vars (BACKEND, MAX_WORKERS, AMBIGUOUS_SCALE, HOOK_STRICT, QUERY_LOG*, MAX_GRAPH_BYTES, API_KEY …). `README.md:372-393,420`, grep [Verified/Docs]

## T6 — How it plugs into agents

In plain words: `graphify install` writes a skill file and an "always use the graph" instruction into each assistant's config folder. On Claude Code it also installs hooks that fire before every search or file read and tell the model to run `graphify query` first. MCP is a separate, optional route.

- `_PLATFORM_CONFIG` has 21 targets: claude, codex, opencode, kilo, aider, copilot, claw (OpenClaw), droid, trae, trae-cn, **hermes** (`.hermes/skills/graphify/SKILL.md`, reusing claw's bundle), kiro, pi, codebuddy, antigravity(-windows), windows, kimi, amp, agents (generic `.agents/skills`), devin. Separate commands cover cursor, vscode and gemini. `graphify/install.py:607-745`, `README.md:225-249` [Verified]
- The skill bodies are 14+ near-duplicate `skill-*.md` files of about 740 lines each, with `skills/<host>/references/*.md` sidecars loaded on demand (progressive disclosure). They are generated and checked by `tools/skillgen` (CI checks: `--check`, `--audit-coverage`, `--schema-singleton`, `--monolith-roundtrip`, `--always-on-roundtrip`). `pyproject.toml` package-data, `.github/workflows/ci.yml:35-47` [Verified]
- Always-on blocks: `graphify/always_on/{claude-md, agents-md, gemini-md, kiro-steering, vscode-instructions, antigravity-rules}.md`. This repo's own `AGENTS.md` is an example: "After modifying code files … run `graphify update .`". `AGENTS.md:1-8` [Verified]
- **Claude Code / CodeBuddy hooks:** PreToolUse matchers `Bash|Grep` → `graphify hook-guard search` and `Read|Glob` → `graphify hook-guard read [--strict]`, timeout 10 s. The hook emits `additionalContext`: "MANDATORY: graphify-out/graph.json exists. You MUST run `graphify query …` before grepping/reading". If the file changed since the build, a softer STALE message is used. Strict mode **denies the first raw Read per session** via `permissionDecision: deny`. The hook always exits 0 (fails open). `graphify/install.py:445-486`, `graphify/cli.py:18-70,2844-2855,1092-1116` [Verified]
- Codex `.codex/hooks.json` gets only a no-op `hook-check`, because Codex Desktop rejects additionalContext. Gemini gets a `BeforeTool` hook in `.gemini/settings.json`. Pi has no PreToolUse, so a native TS extension (`graphify/pi/index.ts`) calls `hook-guard`. `graphify/install.py:1101-1130,1841-1896`, `README.md:255` [Verified/Docs]
- MCP is **not** auto-registered (the antigravity install only prints a config snippet). The README states that `graphify install` "does not establish an MCP connection". `graphify/install.py:1378-1383`, `README.md:313-324` [Verified]

## T7 — LLM usage, cost, offline

In plain words: building a graph from code needs no model and works offline. Docs, PDFs and images need a model: by default the assistant you are already chatting with, or an API key when run headless. Ollama gives a fully local option. Costs scale with the amount of non-code content, not with code size.

- Code-only corpora: no LLM at all, and the skill explicitly forbids asking for a key. `GRAPHIFY_BACKEND=none` disables every backend, including the `claude -p` labeling fallback. `graphify/skill.md:165-172`, `README.md:417-420` [Verified/Docs]
- Inside an assistant, the semantic work is done by the host's subagents. **Token usage is billed to the user's assistant session**, and the chunk JSON records placeholder zeros that the skill asks the agent to back-fill from Agent `usage`. `graphify/skill.md:304` [Verified]
- Inconsistency: the skill says it reads only `GEMINI_API_KEY`/`GOOGLE_API_KEY` and "does not read ANTHROPIC_API_KEY…". Headless `graphify extract` auto-detects in this order: gemini → kimi → claude → openai → deepseek → azure → bedrock → ollama. `graphify/skill.md:165,170`, `graphify/llm.py:3398-3410` [Verified]
- Backend table (default model, $ per 1M tokens in/out):
  - claude `claude-sonnet-4-6` 3/15
  - kimi `kimi-k2.6` 0.74/4.66 (README warns: China-hosted)
  - gemini `gemini-3-flash-preview` 0.50/3.00
  - openai `gpt-4.1-mini` 0.40/1.60
  - deepseek `deepseek-v4-flash` 0.44/1.32
  - azure `gpt-4o` 2.5/10
  - bedrock claude-3.5-sonnet 3/15
  - ollama `qwen2.5-coder:7b` 0
  - `claude-cli` (subscription) 0
  - Source: `graphify/llm.py:125-240`, `estimate_cost` `:3255-3260` [Verified]
- Headless extraction has token-packed chunks, adaptive retry on context overflow or timeout, partial-file markers, and "hollow response" detection. `graphify/llm.py:2285,2444,1287-1340` [Verified]
- Other LLM uses: an optional `--dedup-llm` entity-merge step (the base dedup is MinHash/LSH + Jaro-Winkler, no LLM), community naming, and `graphify prs --triage`, which defaults to `claude-opus-4-7` with fallbacks to other keys, then `claude` CLI, then ollama. `graphify/dedup.py:1-4`, `graphify/prs.py:592-630`, `README.md:419` [Verified]
- Video/audio: local faster-whisper; `yt-dlp` for URLs. `pyproject.toml` `video` extra, `README.md:418` [Docs]

## T8 — Security, privacy, legal

In plain words: code stays on your machine. Docs go to whichever model does the semantic pass. The headless path defends well against prompt injection, but the in-assistant path relies only on the prompt itself. The HTTP server has no auth by default, and the Docker image binds to all interfaces.

- `security.py`:
  - `validate_url` allows only http/https, blocks private, link-local and metadata IPs, and resolves hosts before connecting.
  - `_NoFileRedirectHandler` stops redirects to file://.
  - Fetch caps: 50 MB binary / 10 MB text.
  - Graph-file cap: 512 MiB (`GRAPHIFY_MAX_GRAPH_BYTES`).
  - `validate_graph_path` must resolve inside graphify-out.
  - `sanitize_label` strips control characters and caps labels at 256 chars; metadata is capped (512-char values, 50 list items, 100 attributes).
  - Source: `graphify/security.py:21-32,83-160,243-315,357,391-417` [Verified]
- Secret hygiene: sensitive-path patterns, credential-store dirs and secret-prone data extensions are skipped at detect time. `graphify/detect.py:114-215,287` [Verified]
- Office files: zip-bomb caps (50 MiB raw, 512 MiB decompressed, compression ratio 200). `graphify/detect.py:64-77` [Verified]
- **Prompt injection, headless path:** each file is wrapped in `<untrusted_source path sha256>`. Any `<|…|>` chat-template tokens, `<<SYS>>`, `[INST]` and forged closing tags are defused with a zero-width space, and the system prompt says to treat the block as data. `graphify/llm.py:510-520,598-630` [Verified]
- **Prompt injection, skill/subagent path:** neither `skill.md` nor `extraction-spec.md` contains "untrusted" (grep count 0). Subagents read raw files with full Write/Bash tools (`general-purpose`). A hostile doc in the corpus could therefore try to steer a tool-capable subagent. `graphify/skill.md:272`, `graphify/skills/claude/references/extraction-spec.md` [Verified gap; exploitability = Inference]
- HTTP MCP server: no auth unless `--api-key`/`GRAPHIFY_API_KEY` is set. It warns on `0.0.0.0` without a key. The **Dockerfile's default CMD binds `0.0.0.0:8080` with no key**. `graphify/serve.py:2901-2955`, `Dockerfile:22-25` [Verified]
- Query logging: the README says every query is logged to `~/.cache/graphify-queries.log` by default. **The code makes logging opt-in** (`GRAPHIFY_QUERY_LOG` or `GRAPHIFY_QUERY_LOG_ENABLE=1`, #1797). The code is authoritative; the README is stale. `README.md:422` vs `graphify/querylog.py:15-30` [Verified]
- No telemetry found in code. The only outbound URLs are LLM backends, `add <url>` fetches, `gh` for PR features and a printed Cloud CTA. `README.md:421`, grep for urlopen/requests [Verified]
- Agent-steering hooks: the PreToolUse "MANDATORY" text and the strict-mode deny change agent behavior in every session for that project. This is intrusive, but deny fires at most once per session and the hook fails open. `graphify/cli.py:18-70` [Verified]
- Supply chain: CI runs `bandit -r graphify -ll` and `pip-audit --strict`. Extras are floored above named CVEs (starlette). `.github/workflows/ci.yml:125-129`, `pyproject.toml` mcp extra comment [Verified]
- Legal: Apache-2.0 with a patent grant and patent retaliation; trademark restrictions on the name "graphify". `LICENSE`, `README.md:489` [Verified/Docs]

## T9 — Quality: tests, benchmarks, CI

In plain words: there are thousands of unit tests and solid CI. The published benchmarks use the vendor's own harness, and that harness is not in this repo. The "80% fewer tokens" headline is not backed by anything here.

- Tests: 431 test files and about 6,705 `def test_` (lexical count; not run), one test file per module. Fixtures cover each language. `tests/bench_extract.py` and `tests/bench_query_scoring.py` are micro-benchmarks. The suite avoids network access. `ARCHITECTURE.md:100-114`, `git ls-files tests` [Verified]
- CI (`ci.yml`, ubuntu-latest only):
  - skillgen drift checks
  - pytest on Python 3.10 / 3.12 / 3.13 / 3.14 with `uv sync --all-extras --frozen`
  - Pi `bun test`
  - bandit and pip-audit
  - `release-graph.yml` builds graphify's own graph (AST-only) as a release asset
  - Source: `.github/workflows/ci.yml:17-129`, `.github/workflows/release-graph.yml` [Verified]
- **Checking the "80%" claim:**
  - "Cut agent token spend by 80%" / "80% fewer tokens" appears only next to **Graphify Cloud** marketing. `README.md:31,126,452` [Verified]
  - BENCHMARKS.md never mentions 80% or token-spend reduction. Its token-related figures: ERPNext "about 140K tokens per query", and context-stuffing "costs roughly 20x the tokens for lower coverage". `BENCHMARKS.md:145-150` [Verified]
  - So the 80% figure has no methodology in this repo. [Verified absence]
- The in-repo token metric is `graphify benchmark`:
  - corpus tokens = `total_words × 100/75` from detect output. If that is missing, it is invented as `nodes × 50` words.
  - query tokens = `len(answer_text)//4` for each of 5 canned questions ("how does authentication work", …), using BFS depth 3 with budget 2000. Questions with no match are dropped.
  - ratio = corpus ÷ mean query tokens.
  - Source: `graphify/benchmark.py:10,32-52,55-102`, `graphify/cli.py:3559-3574` [Verified]
- What that metric actually measures:
  - It is **bounded by construction**: the answer is capped near 2000×3 chars ≈ 1,500 "tokens" at 4 chars/token, so any corpus over about 7.5k tokens shows more than 5× "reduction".
  - It compares against **stuffing the whole corpus into context**, a straw-man baseline, not against an agent that greps and reads a few files.
  - It never checks whether the answer was correct.
  - The query renderer budgets at 3 chars/token while the benchmark counts 4 chars/token, which flatters the ratio by about 25%.
  - Source: `graphify/serve.py:1126`, `graphify/benchmark.py:10` [Verified + Inference]
- The headline 71.5× (`docs/how-it-works.md:57`) comes from one 52-file mixed corpus (123,488 vs about 1,726 tokens). The same corpus code-only gives 8.8×. httpx (6 files) gives about 1×. `worked/karpathy-repos/review.md:13-27`, `worked/httpx/README.md:39` [Verified]
- BENCHMARKS.md results (LOCOMO, LongMemEval, ERPNext):
  - They come from "graphify's own harness" (`memory/runner.py`, `crosstool/run.py`), and **those directories are not in this repo**.
  - The best rows use "hybrid retrieval", a shared BGE-m3 embedder and a SurrealDB engine. None of these exist in the OSS package, which contradicts the README's "No embeddings" if read as describing what the benchmarks ran.
  - The ERPNext code result has n=6 questions (82.0% vs 70.8%).
  - The judge is the same model family as the reader (Kimi K2.6), validated against a second judge (κ=0.81).
  - Source: `BENCHMARKS.md:37-56,85-93,143-150,195-203`, `ls memory crosstool` → absent [Verified]
- The INFERRED precision evidence is a single dated manual spot check: 12 out of 12 cross-file edges correct on FastAPI, production code only. The doc admits test-tree `calls` are noisier. `BENCHMARKS.md:152-165` [Docs]

## T10 — Operations

In plain words: install with `uv tool install graphifyy`, then run `graphify install` once. The base install is heavy (about 25 grammar wheels). Leiden, MCP, PDF and video are opt-in extras.

- Install: `uv tool install graphifyy` / `pipx install graphifyy` / `uvx graphifyy install`. Project-scoped install: `graphify install --project` writes `.claude/skills/graphify/SKILL.md` and prints a `git add` hint. `README.md:50-55,194-217` [Docs]
- Footprint: networkx, numpy, rapidfuzz and about 25 tree-sitter grammars in the base install. Leiden needs `graspologic` (pre-3.13) or `graspologic-native`, which pulls in numba/umap; the code bypasses that import. `pyproject.toml`, `graphify/cluster.py:23-36` [Verified]
- Grammar ABI pinning: `tree-sitter>=0.25,<0.26`, because 0.23/0.24 cannot load ABI 15 grammars. `pyproject.toml` comment [Verified]
- Hooks pin the absolute Python interpreter at install time (GUI git clients often have a minimal PATH). Re-run the install after changing Python or venv. `graphify/hooks.py:15-25`, `README.md:255` [Verified/Docs]
- Docker: `python:3.12-slim`, entrypoint `python -m graphify.serve /data/graph.json --transport http --host 0.0.0.0 --port 8080`. `Dockerfile:10-25` [Verified]
- Scale guards: graph-file cap of 512 MiB; trigram index for query; p99 hub threshold; ProcessPool; corpus-size warnings. `graphify/security.py:32`, `graphify/serve.py:470-560,1062`, `graphify/detect.py:55-57` [Verified]
- Incremental:
  - `--update` and `graphify update` use `detect_incremental` with the manifest and content hashes.
  - Only changed code is re-parsed, but resolution indexes include **read-only context nodes from unchanged files**, so a changed caller still binds to an unchanged callee.
  - Semantic files are stamped only if they produced output.
  - Source: `graphify/detect.py:2842`, `graphify/extract.py:9166-9180`, `graphify/watch.py:2121-2125`, `graphify/skill.md:593-614` [Verified]

## T11 — What is actually new vs marketing

In plain words: the extraction and cross-file linking are seriously engineered, and the confidence tagging is a good idea, but neither is a research breakthrough. Several headline claims are stretched or describe the paid product.

| Claim | Verdict | Evidence |
|---|---|---|
| "Code parsed with tree-sitter AST, deterministic, no LLM" | **True** for the code path. Determinism work is real: sorted partition input, seed 42, canonical JSON ordering | `cluster.py:106-140`, `export.py:440-445` [Verified] |
| "~40 languages" | Plausible: 115 suffixes, 55 extractors, about 25 bundled grammars plus optional ones; some are not tree-sitter (regex Pascal fallback, robot.api) | `extract.py:8540-8655`, `pyproject.toml` [Verified] |
| "Every edge tagged EXTRACTED or INFERRED" | True, and there is a third tag, AMBIGUOUS. For code, the tag comes from a **rule** (import evidence → EXTRACTED). For docs, the **LLM chooses** it | `extract.py:10700-10708`, `extraction-spec.md:13-15` [Verified] |
| "Not a vector index. No embeddings" | True for the OSS package. **False for the benchmark configurations** (BGE-m3 hybrid, SurrealDB) | `BENCHMARKS.md:88-93,105` [Verified] |
| "Communities (Leiden)" | Only with the `[leiden]` extra; the default is **Louvain** | `cluster.py:166-177` [Verified] |
| "LLM-free labels" | Default labels are hub names; the skill then asks the agent to rename communities | `cluster.py:185`, `skill.md:509` [Verified] |
| "Cut agent token spend by 80%" | **Cloud marketing, no in-repo methodology** | T9 [Verified absence] |
| "Formal verification" | Cloud only; no such code in the repo | grep [Verified absence] |
| "0 LLM credits graph build" | True for code; docs/images use your assistant's tokens | `skill.md:163` [Verified] |
| "Query logging to ~/.cache by default" (README) | Code says opt-in | `querylog.py:15-30` [Verified] |

Genuinely useful ideas:
1. A **discrete confidence-score rubric** (0.95/0.85/0.75/0.65/0.55), adopted because models collapsed continuous ranges into two values. `extraction-spec.md:47-59`
2. **Import-evidence promotion** from INFERRED to EXTRACTED, and refusing to guess on ambiguous names (god-node guard).
3. **Prompt-fingerprinted semantic cache.** `cache.py:142`
4. **Read-only resolution context** for incremental rebuilds.
5. **Hook-based steering** of agents toward a cheaper retrieval tool, with a "stale" softening.
6. **Shrink guard** on writes.
7. `reflect`: time-decayed "useful / dead_end / corrected" lessons over saved answers. `reflect.py:1-15`

[Verified]

## T12 — Weaknesses, limits, risks

In plain words: it is big and changes fast. It only finds things by name, not by meaning, and it relies on prompt-driven agent steps that can break. A few defaults (open HTTP server, no injection fencing in the skill path) need care.

- **Lexical retrieval only.** `query` finds seeds by label, path and attribute string matching. A conceptual question with no matching identifier ("how is billing reconciled?") seeds poorly or returns "No matching nodes". The `seeds` hook exists so an external ranker can be plugged in. `graphify/serve.py:575-760,1425-1430` [Verified + Inference]
- **Simple graph, one edge per pair**, with specific relations winning: `calls` and `references` between the same pair collapse. Edge direction is encoded by link order in an "undirected" file, which is an easy trap for consumers. `graphify/build.py:1313,1755-1790`, `ARCHITECTURE.md:67-71` [Verified]
- **Name-based cross-file resolution is not type-checked** outside the per-language receiver resolvers. Correctness rests on heuristics (unique candidate, non-test preference, path proximity). The project itself says the INFERRED tag is "most meaningful on production cross-file resolution". `BENCHMARKS.md:162-165`, `graphify/symbol_resolution.py:328-345` [Verified]
- **Monolithic files and churn:** `extract.py` 11,085 lines, `engine.py` 10,313 (one function `_extract_generic` of about 4,400 lines), `cli.py` 5,391. AST cache schema is at v17. Comments cite hundreds of regressions (#1392, #2843, #4449…). Vendoring it means owning that. `wc -l`, `graphify/cache.py:40` [Verified]
- **Skill-orchestrated pipeline is fragile:** correctness depends on the agent substituting `INPUT_PATH`/`SPEC_PATH`/`IS_DIRECTED` correctly, using `general-purpose` subagents, and back-filling token usage. The skill text is full of MANDATORY guardrails written after failures. `graphify/skill.md:219-302,422` [Verified]
- Semantic node IDs must exactly match AST ID rules or "ghost duplicates" appear. LLM output may drift. `extraction-spec.md:61` [Verified]
- Security gaps: no injection fencing on the subagent path; HTTP server unauthenticated by default and the Docker image on 0.0.0.0 (T8). [Verified]
- Steering hooks may conflict with omni-agent's own tool policy, because they inject "MANDATORY" text into every Bash/Grep/Read/Glob call. [Inference]
- Benchmarks are vendor-run with the harness absent, a small n for code (6), and a token metric bounded by construction (T9). [Verified]
- Leiden is optional, so community quality and IDs differ between environments with and without `[leiden]`. [Inference]

## T13 — How to incorporate into omni-agent

In plain words: treat graphify as an external "code map" service. Omni-agent builds or refreshes the graph, then its agents ask focused questions through MCP or the CLI instead of reading whole repos. Do not copy its code. Copy two or three of its ideas. The Apache license is fine for an MIT project as long as the notice is kept.

**Agile lifecycle role**
- **Discover:** onboarding a repo. `GRAPH_REPORT.md` (god nodes, communities, surprising connections, suggested questions) gives a first architecture brief. `path`/`explain` support reverse-engineering work.
- **Plan:** communities become candidate epics or ownership areas. `affected` (blast radius over calls/imports/inherits…) helps size stories. `prs --conflicts` flags PRs that touch the same communities, which signals merge-order risk. `graphify/affected.py`, `graphify/prs.py:1-15` [Verified]
- **Build:** scoped `query_graph` (2k-token budget) as the coder agent's first retrieval step, before grep or read.
- **Verify / Review:** `get_pr_impact` and `affected` for test selection and review focus; `find_import_cycles` as a lint gate. `graphify/analyze.py:720` [Verified]
- **Learn:** graph.json as long-lived structural memory. `save-result` + `reflect` give a lessons artifact (preferred sources, dead ends, corrections) that matches omni-agent's Learn phase. `graphify/reflect.py:1-15` [Verified]

**Mode recommendation: DEPEND over MCP/CLI (external process), plus PORT 3 ideas, SKIP the skill/hook installer.**
- Why not vendor: ~86k lines of fast-moving Python, an AST cache schema bumped 17 times, and node-ID rules that would have to be replicated exactly. [Verified/Inference]
- Why not `graphify install` into omni-agent's agents: its hooks and "MANDATORY" always-on text would compete with omni-agent's own routing. Call the tools explicitly instead. [Inference]
- Code-only mode (`graphify update .` / `extract --code-only`, `GRAPHIFY_BACKEND=none`) gives a **zero-cost, offline, deterministic** baseline. Run the doc/image semantic pass through omni-agent's own model router using `llm.py`'s `<untrusted_source>` fencing, not the subagent path. `graphify/cli.py:2779`, `README.md:417-420`, `graphify/llm.py:598-630` [Verified]
- Ideas to port into omni-agent's own memory/KG layer:
  1. The EXTRACTED/INFERRED/AMBIGUOUS + discrete score rubric on every fact (provenance).
  2. Prompt-fingerprinted caching of LLM extractions.
  3. Shrink-guarded atomic writes, plus refusing ambiguous lookups (exit code 3) instead of guessing.

[Verified]

**License compatibility:** Apache-2.0 → MIT host: fine for depending on it or calling it as a process. If any source is copied, keep the Apache license text and NOTICE, mark modified files, and do not use the "graphify" marks to imply endorsement. Apache's patent-termination clause applies to the copied code. Pre-0.9.25 code is also available under MIT (`LICENSE-MIT`), but mixing provenance is not worth it. `NOTICE:1-8`, `README.md:489` [Verified + Inference]

**Effort (estimate):**
- MCP/CLI adapter + build/refresh job: **2–4 days**.
- Semantic pass through omni-agent's router using graphify's `extract_corpus_parallel` or the prompt file: **+2–3 days**.
- Porting the confidence rubric and cache idea into omni-agent's KG: **3–5 days**.
- Embedding-based seeding through `seeds=`: **+2 days**.

[Inference]

**Proposed interface contract (omni-agent side):**

```yaml
tool: code_graph            # omni-agent capability, backed by graphify
transport: mcp-stdio        # `graphify-mcp <repo>/graphify-out/graph.json`  (fallback: CLI + exit codes 0/3/4)
lifecycle:
  build:   {cmd: "graphify update <repo> [--no-cluster]", llm: none, on: [repo_onboard, post_merge, git post-commit hook]}
  enrich:  {cmd: "graphify extract <repo> --backend <router>", llm: omni-router, on: [docs_changed], budget_usd: cap}
  health:  {check: "graph.schema_version == 1 && built_at_commit == HEAD"}   # else mark STALE
operations:
  query(question, depth<=6, token_budget=2000, seeds?: [node_id|path])   -> text subgraph   # MCP query_graph
  explain(label) -> node + neighbors | AMBIGUOUS(candidates)                                  # get_node / get_neighbors
  path(a, b, max_hops=8, undirected=false) -> hops                                            # shortest_path
  impact(files|pr_number) -> {communities, nodes_touched}                                      # get_pr_impact / `graphify affected`
  overview() -> GRAPH_REPORT.md                                                                # resource graphify://report
provenance_contract:
  every edge: {relation, confidence: EXTRACTED|INFERRED|AMBIGUOUS, confidence_score, source_file, source_location}
  omni-agent rule: INFERRED/AMBIGUOUS edges are hints, never evidence for a Verify-phase gate
security:
  http transport only with --api-key; never bind 0.0.0.0 (override Dockerfile CMD)
  semantic pass only via fenced headless path (llm.py <untrusted_source>), not tool-capable subagents
  do not run `graphify install` hooks in omni-agent-managed workspaces
```

## T14 — Open questions

In plain words: the main unknowns are how good its cross-file links are on our kind of repos, what the "80%" number actually measures, and whether the semantic pass is worth its cost compared with plain code-only graphs.

1. What methodology, workload and baseline produce Cloud's "80% fewer tokens"? Nothing in this repo explains it. Ask the vendor, or measure it in omni-agent with a real agent-vs-agent A/B (tokens **and** task success). [Open]
2. INFERRED precision and recall on Python/TS data-engineering repos (dbt, Airflow, FastAPI services) beyond the 12-edge FastAPI spot check. Plan: sample about 100 INFERRED edges per repo. [Open]
3. How stable are community IDs and labels across runs with and without `[leiden]`, and across incremental updates (`remap_communities_to_previous`)? This matters if communities become epic or ownership keys. `graphify/cluster.py:434` [Open]
4. Is the default undirected simple graph (one edge per pair) acceptable for impact analysis, or should omni-agent always build with `--directed`? Does `affected` already account for this? [Open]
5. Will the MCP server's hot reload plus a per-repo lock behave under concurrent omni-agent workers and `graphify update`? `graphify/serve.py:185`, `graphify/watch.py:188` [Open]
6. Cost and quality of the semantic pass for docs/PDFs compared with omni-agent's own document ingestion (e.g., MarkItDown). Possible plan: feed graphify only code plus Markdown. [Open]
7. Can the BENCHMARKS harness (`memory/`, `crosstool/`) be obtained to reproduce the results? The SurrealDB/hybrid rows suggest a non-OSS engine. [Open]
8. With 1,000+ issues and an AST cache schema bumped 17 times, what upgrade/pinning policy should omni-agent use (pin a minor version, re-run on upgrade)? [Open]
