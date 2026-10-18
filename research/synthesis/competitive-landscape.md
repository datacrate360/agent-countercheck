# Competitive landscape: does anyone already do "an AI team that ships verified work"?

Checked 2026-10-18 by web search and by reading the vendors' own pages (sources below). These are commercial
products and frameworks **outside** the 12 repos we read at source level, so the facts here are [Docs]: they come
from vendor pages, not code.

## 1. In plain words

**Yes. As we first described it, omni-agent is not unique.** Several products already take a goal or an issue, split
it, build it with agents, test it, review it and release it. Some of them are large, funded companies. A "team of
role agents" has existed in open source since 2023.

## 2. Who already does what

| Product | Type | What it already does | Source |
|---|---|---|---|
| **Factory "Software Factory"** (Droids) | Commercial, enterprise | Full SDLC stages: triage → code gen → validate → release → document → monitor, with per-stage metrics; routes each task to the best model; "self-improving" | factory.ai/product/software-factory |
| **Devin** (Cognition) | Commercial | Async agents triggered by events and schedules; tests its own changes end-to-end on a cloud VM with recordings as proof; Devin Review fixes review findings until clean; Auto-Triage | cognition.com/blog/testing-development |
| **Linear agents** | Commercial tracker | Assign an issue to Cursor, Copilot, Codex, Devin, Warp, Sentry; bulk-delegate in parallel; "Loops" for recurring agent work; agents appear as users | linear.app/enablement/guide/ai-agents |
| **GitHub Copilot coding agent** | Commercial | Assign an issue → it writes code, opens a PR, runs validation and tests, iterates | docs.github.com (Copilot cloud agent) |
| **Atlassian Rovo Dev** | Commercial | Jira issue → code → PR; reviewer checks code matches requirements | atlassian.com/software/rovo-dev |
| **MetaGPT** | Open source (since 2023) | "Software company as a multi-agent system": PM, architect, engineer, QA roles in a fixed SOP pipeline | github.com/FoundationAgents/MetaGPT |
| **ChatDev** | Open source (since 2023) | Virtual software company of role agents | github.com/OpenBMB/ChatDev |
| **CrewAI** | Open source framework | Role-based agents ("crews") with tasks and delegation | crewai docs/articles |
| **OpenHands** | Open source + cloud | Platform and control plane for running coding agents | openhands.dev |
| **Paperclip** | Open source (MIT) | Company of agents: org chart, goals, issues, budgets, approvals, 16 engine adapters | `research/tools/paperclip/REPORT.md` |

## 3. What is already taken

- "Goal → backlog → agents build → tests → PR → release": **Factory, Devin, Copilot, Rovo Dev.**
- "One board that sends work to many different agents": **Linear** (closed), **Paperclip** (open).
- "A team of role agents": **MetaGPT, ChatDev, CrewAI** (since 2023).
- "Agent proves its work": **Devin** (it tests its own work and records video).

## 4. What is still open [Inference: hypotheses to validate with users]

1. **Independent verification across vendors.** Today each vendor's agent checks its *own* work (Devin tests Devin).
   Nobody offers a neutral referee that checks work from *any* agent (Copilot, Codex, Claude Code, Devin, Hermes,
   OpenClaw) against the team's own acceptance criteria. AI-authored PRs on GitHub are approaching one million
   (arXiv 2607.26819), so the bottleneck is moving from *writing* code to *trusting* it.
2. **A portable "proof of done".** A signed, auditable record per work item: what was asked, who built it (which
   agent/model), what was checked, by whom, with what result and cost. It would be useful for compliance (AI-content
   disclosure duties), audits and incident reviews, and it would work the same whichever vendor did the work.
3. **Open, self-hosted, vendor-neutral.** Factory, Devin, Linear and Copilot are closed SaaS. Paperclip is open but
   has no verification or Agile layer; OpenHands is a coding-agent platform. An open alternative that a team can run
   on its own infrastructure with any model is thin.
4. **Learning from verified outcomes only.** Memory tools (claude-mem, Hermes, OpenClaw) learn from what the agent
   *did*. Learning only from what *passed independent checks* (and from what failed them) is a different, cleaner
   signal.

## 5. Implication for omni-agent

Don't position omni-agent as "another AI software team". That race has funded leaders. Position it as **the open
referee and record of AI work**: any agent builds, omni-agent checks it independently, keeps the proof, and learns
from what passed. The conductor, planning and roles from `agent-roles.md` remain, but they serve that core.
