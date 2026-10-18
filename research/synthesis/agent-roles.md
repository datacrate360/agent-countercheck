# omni-agent: the work it does and the agents it needs

## 1. In plain words

omni-agent is **an AI team that ships verified work**. You give it a goal. It breaks the goal into small pieces of
work, gives each piece to the agent best suited to it, and calls a piece "done" only when a different agent has
checked the evidence. It remembers what the team learned, so the next goal goes faster.

It is not one super-agent. It is a **team of narrow agents with clear jobs**, like a real team: someone plans,
someone builds, someone checks, someone remembers. Each agent has a small toolbox and a clear boundary, which is
what keeps the whole thing reliable and cheap (the research showed both: big tool lists cost attention on every
call, and shared environments widen the blast radius).

## 2. The work: one goal, seven kinds of work

| Stage | The work | What "done" means | Example |
|---|---|---|---|
| **Discover** | Understand the problem: read the docs, the code, the market | A short brief with sources | "How does our auth work, and what do competitors do?" |
| **Plan** | Turn a goal into a backlog of small items, each with acceptance criteria and an estimate; pick a sprint | Every item fits one session and has a check command | "Add SSO" → 6 stories with tests named |
| **Build** | Do one item: code, config, docs, content | A change plus the evidence the builder claims | A PR with tests |
| **Verify** | Check the claim independently: tests, acceptance criteria, security scan, rules | The checker re-ran the evidence itself | Tests pass, no high-severity findings |
| **Review & release** | Human-visible review, changelog, AI-content labels, release gate | Approved by a person where the stakes need it | Release notes + go/no-go |
| **Operate** | Watch, triage incidents, open fix items | Incident closed with a cause and a follow-up item | Error spike → root cause → backlog item |
| **Learn** | Keep what worked: decisions, gotchas, procedures; retro | Memory and skills updated, and only with proof they help | "Migrations need a lock-timeout" saved as a skill |

The loop never changes: **goal → small items → build one → check it → remember → next.** That loop is the product.

## 3. The agents

Nine roles. Each one has a single job, a narrow toolbox, a clear "can't", and the patterns it inherits from the
research.

| # | Agent | Its one job | Can use | Can't | Done when | Learns from |
|---|---|---|---|---|---|---|
| 1 | **Conductor** | Owns the goal: breaks it into items, sets WIP, dispatches, tracks budget, asks the human at gates | work board, budgets, dispatch, memory (read) | write code, mark anything "passing" | every item is passing or blocked with a reason | Paperclip (dispatch, budgets, review stages), Hermes `/goal`, tutorial Stage 9 |
| 2 | **Researcher** | Answers questions with sources | web search, ingest (MarkItDown), code graph (graphify), memory | change anything | brief with cited sources | Odysseus deep research (clean-room), graphify, MarkItDown |
| 3 | **Planner** | Writes specs and acceptance criteria, splits stories, estimates | docs, code graph, memory | build | every item has a check command and fits one session | Learn Harness Engineering (feature list = behaviour + verification + state) |
| 4 | **Builder** | Does one item in a sandbox | files, shell (sandboxed), tests, its engine's tools | touch other items, mark itself done | change + evidence claim | Claude Code / Codex / Hermes / OpenClaw as engines; Hermes backends; Odysseus sandbox-or-refuse |
| 5 | **Checker** | Independently verifies the builder's claim | re-run tests, read the diff, run rules and scans | see the builder's reasoning, edit code | passing / active / blocked + evidence | tutorial Stage 9 `check()`, Odysseus completion gate, OpenClaw "no fallback after side effects" |
| 6 | **Reviewer** | Reviews against architecture rules and style; prepares the human decision | diff, rules (WHAT/WHY/FIX), memory | merge | approve / request changes with reasons | Learn Harness Engineering L10, Paperclip review stages |
| 7 | **Security** | Scans each increment in staging; checks provenance | Strix (container, scoped), provenance lint, REA (static only) | touch anything outside the written scope | findings as items, or a clean result with coverage | Strix, REA evidence envelope, watermarks-remover (detection only) |
| 8 | **Operator** | Watches running systems, triages incidents, opens fix items | logs, metrics, read-only access | deploy or roll back without approval | incident closed with cause + follow-up item | tutorial incident assistant (§7), OpenClaw audit ledger |
| 9 | **Librarian** | Keeps the team's memory honest: captures observations, promotes what's proven, curates skills, runs the retro | memory store, skills, session search | invent facts; promote untrusted content | memory changes logged and reversible | claude-mem observer, OpenClaw dreaming + provenance, Hermes curator, Odysseus skill-vs-baseline audit |

**And the human:** sets goals, approves scope and risky actions, and makes the final call at release. Every agent
can ask; only the human can approve what's marked high-risk.

## 4. Why this split is the right one

- **The builder never grades itself.** The most common agent failure in the research is declaring victory too early.
  A separate checker that re-runs the evidence is the single biggest reliability gain.
- **Narrow toolboxes.** Each agent sees 5–15 tools, not 200. Fewer tools means better choices and lower cost per call.
- **Each role can run on a different engine.** The builder might be Claude Code today and Codex tomorrow; the checker
  can be a small cheap model plus real test commands. Swapping is configuration.
- **Clear blame.** When something goes wrong, the trace says which agent did what, under which budget and approval.

## 5. What to build first

| Step | Agents | What you can do at the end |
|---|---|---|
| **MVP** | Conductor + Builder (via the Claude Code and Codex adapters) + Checker | Give a goal; get small items built one at a time, each accepted only when its tests pass |
| **v0.2** | + Planner + Reviewer | Real backlogs with acceptance criteria, sprints and review gates |
| **v0.3** | + Librarian | The team remembers decisions and gets faster each sprint |
| **v0.4** | + Researcher | Discovery with sources, from documents and code |
| **v0.5** | + Security + Operator | Safe releases and incident follow-up |

Each step is usable on its own, and omni-agent builds itself from the MVP onward.
