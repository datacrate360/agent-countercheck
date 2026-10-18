# Where is the open space? Ranked from market research T1–T7

Sources: `research/market/T1`…`T7` (Reddit, Hacker News, GitHub issues, vendor pages; 2026). Spot-checked by the
coordinator on 2026-10-18 via the HN Algolia API and the GitHub API: HN 47911524 (860 pts), 47323017 (659),
49195468 (340), 47633396 (1,099); claude-code #42796 (3,286 reactions); zeroshot 1,994★ MIT; git-ai 2,853★: all match.
Corrections: claude-code #18986 has 0 reactions (an example of the behaviour, not of demand); Odysseus #1711 (PR-triage
proposal) has 11 reactions, and the 66 reactions belong to the separate "WTF is going on here?" issue. Reddit was partly
blocked, so many Reddit vote counts are missing (T3 notes this).

## 1. In plain words

The research **changes our answer**:

- **"An independent AI reviewer" is taken.** CodeRabbit ($1.5B), Greptile, Devin Review, Copilot review, Bugbot, plus
  open source (zeroshot with "the writer cannot approve its own work", open-code-review, pr-agent). And users
  *complain about these reviewers*: too many comments, ~12% precision in one benchmark.
- **"A record of which AI wrote this" is taken.** Cursor's Agent Trace spec and git-ai.
- **"Run many agents in parallel" is crowded.** More than 40 open tools, most of them ignored.
- **What nobody has: proof that the work actually happened and actually works, checked against reality by something the
  agent can't edit, with what it cost, in one record a human can approve in seconds.**

The pain behind that is loud and everywhere: agents say "done, tests pass" when they aren't, in code *and* in
business systems. Review queues explode (40 → 110 PRs a week, same 4 approvers). Bills nobody can explain ("8
developers burned $2K in a single day and no one really knows how"). Vendors silently change models (claude-code #42796,
3,286 reactions).

## 2. Ranked open spaces

| # | Open space | Evidence | Crowded? | Strength |
|---|---|---|---|---|
| **1** | **Evidence-based "done" checker that runs things, not opinions**: re-runs tests it controls (the agent can't edit or skip them), reads back the real system (DB row, CRM record, refund, file), and reports *claim vs reality* | T1, T4, T5, T6: "it said done but wasn't" recurs across code and ops; every fix today is a homemade hook; users trust deterministic checks over AI comments (T7) | No product named by users; zeroshot is closest (code only, AI reviewers) | **Strong** |
| **2** | **Cost per verified outcome**: join spend with pass/fail per task, agent, model and vendor ("$ per verified done task") | T6, T4: dashboards show totals, not per-task; Paperclip shows $0 for subscription runs; Uber/Microsoft budget threads | Observability (Langfuse etc.) does tokens, not outcomes | **Strong** |
| **3** | **Review-queue shrinker**: one-screen "approve in 30 s" summary per change: what was asked, evidence, risk, cost; only risky items reach a human | T1, T2: review, not compute, is the limit; people cap themselves at 2–3 agents | AI reviewers *add* comments; nobody removes work | **Strong** |
| **4** | **Your own regression watch**: replay *your* tasks before you accept a model/agent upgrade, and catch silent vendor changes | T4, T6: ~994 openclaw issue titles say "regression"; #42796; cache 1h→5m (#46829) | Evals are for builders of models, not users of agents | **Medium-strong** |
| **5** | **Effect-gated approvals**: ask a human only for real effects (prod write, money, external send), not every command | T3: humans miss 1 in 3 threats in approval prompts; YOLO mode everywhere; "agent deleted our production database" | Agent firewalls exist but none has won | **Medium** |
| **6** | **Plain-language outcome inbox for non-technical owners** (Slack/Teams/WhatsApp): what ran, cost, did it work, approve button | T5 | Thin | **Medium** |
| **7** | **Signed record of what was checked and who approved the exact code** (beyond who wrote it) | T7, T1, T3: compliance pull (Amazon sign-off, EU AI Act Art. 12, FINRA), but developers resist; launches got 2–9 HN points | Early specs only | **Medium**, buyer is compliance, not devs |

**Not open (drop or keep as plumbing):** another AI code reviewer; a solo multi-agent launcher; a generic "AI employee";
"open and self-hosted" as a selling point by itself (table stakes); "bring your own model" (expected, not a
differentiator).

## 3. What this means for omni-agent [Inference]

Merge #1–#3 into one product:

> **omni-agent checks whether AI work actually happened and actually works, for any agent, and tells you what it cost.**
> Each task gets a *receipt*: the claim, the evidence omni-agent gathered itself (tests it ran in a sandbox the agent
> can't touch, read-backs from the real system), pass/fail, cost, and a one-screen approve/reject.

Why this can win:
- **It sits on top of every agent** (Claude Code, Codex, Copilot, Devin, Hermes, OpenClaw, n8n flows), so the
  giants' growth is its growth.
- **It reduces work** for the human (fewer things to review) instead of adding comments, which is the complaint against
  today's reviewers.
- **It works beyond code**: "did the CRM really update?" is the same check pattern as "did the test really pass?".
- **Receipts add up to data nobody else has**: verified-success rate and cost per agent, model and task type. That
  becomes the regression watch (#4) and a neutral benchmark of real-world agent quality.

Risks the evidence shows:
- **A bad checker is rejected fast** (work sent back 15 times; "Proof Loop" Show HN got 2 points). Precision matters
  more than coverage: report only what was *proven*, and say "unverified" honestly.
- **Developers like to build their own** (DIY hooks). The open-source core must be easier than a homemade hook.
- **Vendor platform risk**: Anthropic changed subscription rules three times in 2026. Depend on outputs (git, APIs,
  logs), not on vendor internals.
- **Who pays**: individual devs rarely do. The likely payers are team leads with cost pain (#2) and companies with
  compliance needs (#7).

## 4. Next step: validate before building

1. Write 3 one-paragraph pitches (#1 receipts, #2 cost per verified task, #3 review shrinker) and test them with 10–15
   people who run agents daily (the HN/Reddit posters in T1–T6 are exactly them).
2. Build the smallest real demo: a CLI `omni verify` that takes an agent's claimed "done" on a repo, re-runs the
   project's checks in a clean sandbox, diffs claim vs reality, and prints a receipt with cost. Post it as a Show HN
   with a real "caught it lying" example.
