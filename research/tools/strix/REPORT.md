# Strix: research report (integration profile)

- **Repo:** `usestrix/strix`, commit `a0f8ea2` (2026-10-18), package `strix-agent` 1.7.0, **Apache-2.0** (`pyproject.toml:2-6`), Python ≥3.12
- **Scope note:** two full-depth analysts were stopped by the model provider's content filter while reading the
  pentest knowledge packs. This profile was written by the coordinator **from Strix's own docs, config and code
  layout only**. It deliberately leaves out attack techniques and the contents of `strix/skills/`. It covers what
  omni-agent needs to decide on integration. Depth is lower than the other reports: T2, T9 and T11 are partial
  (see T14).

## Executive summary

Strix is an autonomous **application-security testing agent**. You point it at a code folder, URL, domain or IP that
you are authorized to test. A tree of agents probes it inside a Docker sandbox, and the run writes a Markdown report,
JSON findings and a **SARIF 2.1.0** file. It is built on the **OpenAI Agents SDK + LiteLLM**, so any model works. It has
a headless CI mode with **exit codes (0 clean / 1 error / 2 findings)**, a **dollar budget** and a turn cap, can **scan
only a diff**, and accepts **your own MCP servers** as context. There is a paid managed cloud (app.strix.ai) and
anonymous telemetry is **on by default**. For omni-agent it fits the **Verify** stage as an external, container-run
tool against the team's own staging targets, gated by written scope and a human approval.

## T1. Identity

In plain words: an open-source "AI pentester" from a company that also sells a managed version. You run it locally
with Docker and your own model key.

- `strix-agent` 1.7.0, Apache-2.0, Python ≥3.12 (`pyproject.toml:2-7`) [Verified].
- Built on `openai-agents[litellm]>=0.19.0,<0.20` (`pyproject.toml`, dependencies) [Verified]. README thanks LiteLLM, Caido, Nuclei, Playwright, Bubble Tea [Docs].
- Two ways to run it with the same engine: the self-hosted CLI (free, needs Docker and your LLM key) and the managed cloud app.strix.ai (credits, Enterprise plan for reports) (`AGENTS.md:26-57`) [Docs].
- Repo: 545 files. The largest parts are `strix/interface` (181 files: CLI/TUI), `strix/skills` (80 knowledge packs), `strix/tools` (41) and `tests` (113 files) (`git ls-files`) [Verified].

## T2. Core mechanism (architecture level)

In plain words: the CLI starts a Docker sandbox and a root agent. The root agent can create child agents for parts
of the job. They all use tools inside the same sandbox, record findings, and a reporting step merges duplicate
findings and writes the outputs.

```
strix CLI (-n headless)          ┌──────────── Docker sandbox (one per run) ────────────┐
  │ target, scan-mode, scope,    │ shell · browser · intercepting proxy (Caido) · files │
  │ budget, turns, MCP servers   └───────────────▲──────────────────────────────────────┘
  ▼                                              │ tool calls (shared session)
 runner ──► root agent ──spawn──► child agents ──┘   (agent graph; messages between agents)
  │            (OpenAI Agents SDK + LiteLLM, any model)
  ▼
 report/: findings → dedupe (separate "dedupe" model) → coverage → writer → *.md, vulnerabilities.json, findings.sarif, run.json
```

- Layout as the repo's own guide describes it: `strix/agents` (agent graph + prompts), `strix/tools` (proxy, browser, terminal, scanners), `strix/runtime` (Docker sandbox), `strix/report` (findings, SARIF), `strix/skills` (internal knowledge packs) (`AGENTS.md:68`) [Docs].
- Tool families present as packages: `agents_graph`, `agent_browser`, `shell`, `proxy`, `apply_patch`, `coverage`, `finish`, `load_skill`, `mcp`, `notes`, `reporting`, `threat_model`, `todo`, `view_image`, `wait_for_user`, `web_search` (`strix/tools/*`) [Verified, names only].
- **One sandbox per run, shared by every agent:** a child agent's context is a shallow copy of its parent's (`child_ctx = dict(parent_ctx)`, `strix/core/execution.py:1173-1176`), and the run context carries the sandbox session (`strix/core/runner.py:551`). So all agents in a run work in the same container [Verified]. A misbehaving child therefore affects the whole run's environment [Inference].
- Findings are deduplicated by a separately configurable model with its own headers/body settings (`DEDUPE_LLM_EXTRA_HEADERS`/`DEDUPE_LLM_EXTRA_BODY`, `strix/report/dedupe.py:33-46`) [Verified].
- Report modules: `coverage.py`, `dedupe.py`, `pricing.py`, `runs.py`, `sarif.py`, `state.py`, `usage.py`, `writer.py` (`strix/report/`) [Verified, names].

## T3. Process model

In plain words: one Python process on your machine controls a Docker container where the work happens.

- Runtime package: `docker_client.py`, `docker_connection.py`, `session_manager.py`, `caido_bootstrap.py`, `backends.py` (`strix/runtime/`) [Verified, names]. Docker must be running (`AGENTS.md:35`) [Docs].
- Scans take from minutes (`quick`) to hours (`deep`); the guide says to run them in the background (`AGENTS.md:35`) [Docs].

## T4. Data model and outputs

In plain words: every run gets its own folder with a human report, machine-readable findings and run metadata.

- `strix_runs/<run-name>/`: `penetration_test_report.md`, `vulnerabilities/*.md`, `vulnerabilities.json`, `findings.sarif` (SARIF 2.1.0), `run.json` (`AGENTS.md:37`) [Docs].
- `run.json` records `status` and `llm_usage.cost` vs the budget. The guide warns that exit code 0 "only covers what was analyzed", so check `run.json` before calling a run clean (`AGENTS.md:36`) [Docs].
- Settings persist to `cli-config.json`. Dict-valued settings (`LLM_EXTRA_HEADERS`, `LLM_EXTRA_BODY`) are saved as JSON text and decoded on load (upstream commit message for #1495) [Docs].

## T5. Interfaces

In plain words: a CLI made for automation (flags, exit codes, files), a cloud CLI/REST API, and agent skills.

- Headless: `strix -n -t <target> --scan-mode quick|standard|deep` (default `deep`) (`docs/usage/cli.mdx:47`) [Docs].
- Guard rails as flags: `--max-budget <usd>` (`:77`), `--max-turns` (default **500**, `:117`), `--fail-on <severity>` (`:65`), `--scope-mode` (default `auto`; `diff` with `--diff-base origin/main` scans only a change) (`:51,152`) [Docs].
- Exit codes: `0` clean, `1` fatal, `2` vulnerabilities found (`AGENTS.md:36`) [Docs].
- Models: `STRIX_LLM` (any LiteLLM id) + `LLM_API_KEY`; extra request fields via `LLM_EXTRA_BODY` (`AGENTS.md:31-32`; `docs/llm-providers/`) [Docs].
- Your own MCP servers: `~/.strix/mcp-servers.json` (stdio or http with bearer token), e.g. a DB schema server, hosting info, issue tracker (so accepted risks aren't re-reported), logs (`docs/integrations/mcp.mdx`) [Docs].
- Agent skills for coding agents: `npx skills add usestrix/strix` (run, fix-and-verify, CI scanning, managed platform) (`AGENTS.md:10-24`) [Docs].
- Cloud: `strix cloud <resource> <verb>` mirrors the REST API (OpenAPI at docs.app.strix.ai); JSON when not a TTY; source uploads are reviewed with a dry run and approved by SHA-256 (`AGENTS.md:41-57`) [Docs].

## T6. Plugging into agents

In plain words: other agents drive Strix like a CI tool: run it headless, then read the files it writes.

- Documented for coding agents (`docs/integrations/coding-agents.mdx`) and GitHub Actions (`docs/integrations/github-actions.mdx`: install, set `STRIX_LLM`/`LLM_API_KEY` secrets, `strix -n -t ./ --scan-mode quick`) [Docs].
- No MCP *server* for Strix itself was found in the files listed; integration is CLI + files (or the cloud REST API) [Inference from layout].

## T7. LLM usage

In plain words: you bring any model through LiteLLM; the cost is many model calls per scan, capped by your budget.

- LiteLLM routing through the OpenAI Agents SDK; separate optional dedupe model (`strix/report/dedupe.py:33-46`) [Verified].
- `--max-budget` in dollars and `run.json` `llm_usage.cost` make cost observable and capped (`AGENTS.md:33,36`) [Docs]. Typical spend per mode was not measured here (T14).

## T8. Safety, scope, legal, telemetry

In plain words: it really attacks the target you give it, so only point it at systems you own or have written
permission to test. Its main controls are scope settings, budgets and a container, and telemetry is on until you
turn it off.

- **Authorized use only:** the README warning says unauthorized testing is illegal and the user is responsible (README "Authorized use only") [Docs]; the agent guide repeats "Only scan targets the user is authorized to test" (`AGENTS.md:60`) [Docs].
- Scope controls: target flag, `--scope-mode` (auto/diff), a dedicated scope prompt template (`strix/agents/prompts/scope.jinja`) [Verified, file exists]. Whether out-of-scope hosts are blocked **at the network level** (not just by prompt) was not verified (T14). Treat scope as advisory until proven.
- Isolation: work runs in a Docker container, but **one container is shared by all agents in a run** (T2) [Verified].
- Cloud source uploads exclude git ignores, hidden files, `.git`, symlinks, dependency/build output, secret-like filenames and nested archives, and need an explicit SHA-256 approval (`AGENTS.md:52-54`) [Docs].
- **Telemetry on by default:** `enabled: bool = Field(default=True, alias="STRIX_TELEMETRY")` (`strix/config/settings.py:165`) [Verified]. PostHog + Scarf. Claimed contents: error class, OS, version, scan mode, model name, skills used, vulnerability counts by severity/CWE. Never targets, code, findings or prompts (`strix/telemetry/README.md`) [Docs]. Opt out with `STRIX_TELEMETRY=0`.

## T9. Quality

- 113 test files (`git ls-files tests`) [Verified]; CI runs ruff, mypy, bandit, pytest, Go TUI and viewer checks on every PR (upstream commit "ci: run ruff, mypy, bandit, pytest, Go TUI and viewer checks on every pull request") [Docs]. Benchmark claims in `benchmarks/` were **not** reviewed (T14).

## T10. Operations

- Install: `curl -sSL https://strix.ai/install | bash` (pipe-to-shell; pin and review in CI) (`AGENTS.md:30`) [Docs]. Needs Docker. Go/Bubble Tea TUI ships alongside the Python CLI (upstream PR #941) [Docs].

## T11. Real vs marketing

- Real and useful for a platform: CI exit codes, SARIF output, budget and turn caps, diff-scoped scans, dedupe of findings, MCP context servers [Docs/Verified as cited].
- Not checked here: detection quality and false-positive rate. The benchmarks were not reviewed.

## T12. Weaknesses and risks

- Shared container for all agents in a run (T2).
- Telemetry on by default (T8).
- Scope enforcement may be prompt-level (T14).
- Pipe-to-shell install; managed-cloud upsell.
- Default `--scan-mode` is `deep` (hours, highest cost); CI should always pass `quick` + `--max-budget` [Inference].

## T13. How it fits into omni-agent

In plain words: run Strix as a sealed, external step in the **Verify** stage, only against your own staging
environment, only after a human approves the scope, and feed its SARIF into the same findings view as tests.

- **Agile role:** Verify. Every increment gets a security pass before release; findings become backlog items.
- **Integration choice:** run as an **external container job** (Apache-2.0 is compatible with MIT; no code copied).
  Don't embed it in-process.
- **Guardrails omni-agent adds:**
  1. a written `scope.yaml` per project (allowed hosts/paths), approved by a human, checked before every run;
  2. network egress allowlist at the container/host level (not only the prompt);
  3. always `-n --scan-mode quick --max-budget <n> --max-turns <n>`, `--scope-mode diff` on PRs;
  4. `STRIX_TELEMETRY=0`;
  5. only staging targets the team owns, never production or third parties;
  6. results ingested from `findings.sarif` + `run.json`. A run counts as "clean" only if `status` completed and
     coverage is acceptable (the guide's own warning).
- **Interface contract (proposal):**
  ```
  security_scan(target, scope_ref, mode="quick", budget_usd, max_turns, diff_base?) -> {
      status, cost_usd, exit_code, findings: SarifResult[], report_path, coverage_note }
  ```
  Each finding becomes a work item (severity → priority) and the fix PR re-runs the scan as its verification command.
- **Effort:** 2–4 days for the container job + SARIF ingestion; 3–5 days for scope approval + egress allowlist.

## T14. Open questions (not checked)

- Whether out-of-scope requests are blocked by code/network, or only by the scope prompt.
- Benchmark methodology and detection/false-positive rates (`benchmarks/`).
- Container resource limits and image size.
- Whether the coverage check blocks `finish` or only reports.
- Full agent-loop stop conditions beyond `--max-turns` and `--max-budget`.
