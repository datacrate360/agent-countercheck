# T7 Referee / Proof-of-Done Landscape — market research

Theme: who is already building the "referee" (an independent checker of AI agents' work) and the "proof of done" (an audit record per AI-made change), and how people react to it. The question: are H1 and H2 open, or already crowded?

Research date: 2026-10-18. Every quote below was copied from a page or API response I actually retrieved. Sentences marked [Inference] are my own reasoning.

---

## Sources covered

| Platform | What I read | Date range | Notes |
|---|---|---|---|
| Hacker News (Algolia API) | 16 story searches, 6 comment searches; full comment trees of 15 threads (~720 comments), plus 7 single comments | 2026-01-17 → 2026-10-17 | Story points and comment counts are shown. Algolia does **not** return points for single comments, so HN comment quotes have no score. |
| GitHub (gh CLI) | Repo metadata for 17 projects; top issues by reactions for 5 repos; READMEs of git-ai and zeroshot | Repos created 2023-07 → 2026-08, all pushed 2026 | Read-only. |
| Reddit | 4 threads read in full or in part (r/webdev, r/codereview x2, r/devops, r/ClaudeAI) | 2026-04-11 → 2026-09-11 | Direct `.json` with curl was **blocked** (HTML challenge). The browser was **blocked** ("You've been blocked by network security"). old.reddit sometimes needed a login. Some threads worked through web_extract. For 2 threads the post date was not visible. |
| Vendor blogs / specs | Greptile "AI code review bubble", CodeRabbit Series C, Cognition Devin Review, InfoQ on Agent Trace, OpenFab spec, agentattest README, Zylos, crashoverride | 2026-01 → 2026-10 | Vendor claims are marketing, not proof. |
| Product Hunt | Code-review-tools category page | Updated 2026-10-15 | Only the category page loaded. I could not read individual launch comment threads. |

Not reached: X/Twitter, Discord, LinkedIn, G2 reviews. Reddit coverage is thin because of blocking.

---

## Landscape map: who already builds the referee / proof of done

**A. Paid, independent AI PR reviewers (many, well funded)**
- CodeRabbit raised "$143 million in a Series C funding round at a $1.5 billion valuation" and calls itself "the industry-leading independent AI code review layer" (2026-08-12, https://www.coderabbit.ai/blog/introducing-agentic-change-management). It is now growing into "Agentic Change Management".
- Greptile's whole pitch is independence: "We strongly believe that the review agent should be different from the coding agent. … an auditor doesn't prepare the books, a fox doesn't guard the henhouse" (2026-01-24, https://www.greptile.com/blog/ai-code-review-bubble). The HN thread got 351 points and 249 comments (https://news.ycombinator.com/item?id=46766961). Greptile also shipped TREX, "An AI code reviewer that runs your code" (60 pts, 2026-06-17, https://news.ycombinator.com/item?id=48571851).
- Greptile names the crowd itself: "OpenAI, Anthropic, Cursor, Augment, now Cognition, and even Linear … pure-play code review agents like Greptile (that's us!), CodeRabbit, Macroscope, and a litter of fledgling YC startups" (same blog).
- Other names seen in 2026 threads: Cursor Bugbot, Copilot review, Devin Review (free), Qodo, Graphite, Augment, Unblocked, cubic, Ellipsis, CodeAnt, Kilo, Vetoo, Revix, Ito, Octopus, Rovo/Bitbucket.

**B. Open-source / self-hosted reviewers (also crowded)**
- alibaba/open-code-review: **47,575 stars**, created 2026-05-18; HN launch 284 pts / 73 comments (https://news.ycombinator.com/item?id=48406358).
- The-PR-Agent/pr-agent: 13,434 stars (the original open-source PR reviewer).
- vercel-labs/openreview 1,693; kodustech/kodus-ai 1,498; miracodeai/mira 369; Juror-AI/juror 123 ("Cheaper and better Greptile alternative"); puemos/lareview 181; adamsreview 244.

**C. Cross-vendor "writer ≠ checker" orchestration (H1 exactly)**
- the-open-engine/zeroshot: **1,994 stars**, MIT. Its README says "The agent that writes the code cannot approve its own work. The engine enforces the graph's review gates and retry limits." It runs "Claude Code, Codex, or Copilot as the worker and reviewers, with a separate harness and provider available for each agent" (https://github.com/the-open-engine/zeroshot).
- Smaller ones: dsifry/goodtogo (65 stars, deterministic "PR is ready" check), StrangeDaysTech/straymark, adversarial-review skills, MartinLoop.

**D. AI authorship / provenance records (H2, attribution half)**
- git-ai-project/git-ai: **2,853 stars**. "every line of AI code is linked to the **agent, model, and prompts** that generated it". Its standard is supported by Claude Code, Codex, Cursor, Copilot, OpenCode (README).
- Agent Trace (Cursor RFC, with Cognition): a "vendor-neutral format for recording AI contributions alongside human authorship". InfoQ notes it "does not attempt to assess code quality" (2026-02-04, https://www.infoq.com/news/2026/02/agent-trace-cursor/).
- in-toto / SLSA for agent code: OpenFab "generation predicate" (records model, prompt hash, "the acceptance contract", human sign-offs; https://open-fab.ai/attestation/generation/v0.1), AuroraAeon/agentattest (1 star, created 2026-04-28), crashoverride guides.

**E. Agent action audit trails / black boxes (many tiny repos)**
- safedep/gryph 179 stars; Trusted-Autonomy (8 stars, "Audit trail + independent validation of what it did"); many 1–5 star "agent-audit-trail" repos (hash-chained JSONL, EU AI Act). HN launches here get 2–9 points.

[Inference] The "referee" layer is the most crowded part of the AI-coding market in 2026. The "provenance" layer has a well-backed standard (Agent Trace + git-ai). The **join** — one record that says *who wrote it + who/what checked it + what evidence was run + who approved, signed, across vendors* — has only tiny or spec-stage projects (OpenFab, agentattest).

---

## Top pain points ranked by evidence

### 1. AI reviewers are noisy; people stop trusting them (very frequent)
Appears in almost every review thread I read (HN bubble thread, Ask HN tools, GPT-6 review thread, Vetoo launch, r/codereview x2).
- "the signal to noise ratio is poor. It's really hard to get it not to tell you 20 highly speculative reasons why the code is problematic along with the one critical error." — zmmmmm, 2026-01-26, https://news.ycombinator.com/item?id=46771757
- "We have Code Rabbit at work, and it's made PRs unreadable." — the__alchemist, 2026-01-26, https://news.ycombinator.com/item?id=46772357
- "I've tried Greptile and it's pretty much pure noise. I ran it for 3 PRs and then gave up." — sebra, 2026-01-27, https://news.ycombinator.com/item?id=46777079
- "It is so verbose that sometimes it generated larger comments than the PR diff itself (no joke). Also it missed a lot of critical issues, so in the end devs treated it as noise and didn't really trust it." — u/koxboi, 2 pts, 2026-09-11, https://www.reddit.com/r/codereview/comments/1vce5h9/greptile_coderabbit_or_other/
- "We tried coderabbit, qodo, and they all produce an amount of noise that end up being useless." — oscarestudillom, 2026-09-09, https://news.ycombinator.com/item?id=49621594
- A benchmark run on alibaba's tool: "very good recall (~74% …) - not so good precision (~12%, e.g. lots of false positives)" — eranation, 2026-06-05, https://news.ycombinator.com/item?id=48408272

Who: team leads and devs at small-to-mid companies. Workaround: tune config, switch vendor, or build their own. Solved? Vendors claim it (Augment says "2 to 3x more likely to get fixed", https://news.ycombinator.com/item?id=46776697), but users keep switching. Some praise Bugbot and Unblocked for low noise.

### 2. Human review is the bottleneck now; AI made PRs bigger and more numerous (very frequent)
- Cognition: "code review—not code generation—is now the bottleneck to shipping great products." (2026-01-21, https://cognition.ai/blog/devin-review)
- "My company has a few devs and largish codebases and we're running out of capacity to review PRs based on the rate of code we're pushing now." — u/xFawtface2x, 2026-08-01, https://www.reddit.com/r/codereview/comments/1vce5h9/greptile_coderabbit_or_other/
- "I think the major problem is that llms have made the cost of code cheaper but the cost of engineering has stayed the same." — u/Orlandocollins, 450 pts, 2026-04-11, https://www.reddit.com/r/webdev/comments/1sin68g/i_audited_6_months_of_prs_after_my_team_went/ (post: 1,658 pts, 393 comments)
- "So you merged 340 PRs into main without having a human review them? Yikes" — u/RedPandaExplorer, 828 pts, same thread.
- "For work I often have to review multi thousand file changes in PR mostly generated by AI" — HN Ask, 2026-06-15, https://news.ycombinator.com/item?id=48538778

Who: everyone from solo devs to 40-person teams. Today: Devin Review / Stage style "make it easier to read" UIs, chaptering, more AI review. Reception of UI-only tools is mixed — Stage (130 pts launch) was reported as sunsetting three months later (https://news.ycombinator.com/item?id=48996894).

### 3. "Any model can review; why pay a wrapper?" — commoditization (frequent)
- "This whole space is at best an open-source project, not a (multiple!) whole company." — roncesvalles, 2026-01-27, https://news.ycombinator.com/item?id=46774886
- "i would rather spend a week vibe copying this than onboarding a tool i have to pay for and am at the mercy of whoever made it. Its just over for selling saas tools like this. For agents i also need this local not on someones cloud." — jFriedensreich, 2026-04-17, https://news.ycombinator.com/item?id=47807516
- "Am I right to conclude all these AI platforms that do things like code review are just increasingly irrelevant wrappers around the top models?" — bko, 2026-04-30, https://news.ycombinator.com/item?id=47968182
- "Rolled our own with OpenCode, seems to work quite well and meets the goal of being vendor agnostic :)" — r_p4rk, 2026-06-18, https://news.ycombinator.com/item?id=48592498
- "Built my own using Claude Code; inside a gitlab job we call Claude Code headless." — stpedgwdgfhgdd, 2026-06-19, https://news.ycombinator.com/item?id=48595210

Who: experienced devs, platform teams. Today: DIY with Claude Code / Codex / OpenCode in CI. [Inference] This is good for an open-source project and bad for a paid one.

### 4. Pricing anger and lock-in (medium)
- HN post "Greptile's New Pricing Is Predatory" (17 pts, 2026-04-30, https://news.ycombinator.com/item?id=47966075).
- "I recently moved off Cursor's BugBot because it's no longer a flat $40, and I feel a little lost trying to find a viable alternative because there are so many and the pricing kind of sucks for all of them." — causal, 2026-06-05, https://news.ycombinator.com/item?id=48407110
- "they sell you code review and then they try to upsell you to their ecosystem with agents, platform etc." — u/koxboi, 2026-09-11, Reddit link above.
- "For our sugary, CodeRabbit would have charged >$700 a month" — u/Own-Warthog8272, 2026-08-01, same Reddit thread.

### 5. Agents say "done" when they are not (medium)
- gtg launch: the agent would "declare victory while threads were still unresolved. The core problem: no deterministic way for an agent to know a PR is ready to merge." — dsifry, 45 pts, 2026-01-17, https://news.ycombinator.com/item?id=46656759
- Reply on the same idea: "I don't understand how this provides anything above using GitHub status checks and branch protections to require conversations to be resolved before merging." — nyc1983, https://news.ycombinator.com/item?id=46664278
- Blog title: "Agent-written tests can't verify agent-written code" (2026-03-02, https://news.ycombinator.com/item?id=47221866, 3 pts).
- Dan Luu, "How well do agents use test/verification techniques?" got 191 pts / 71 comments (2026-09-08, https://news.ycombinator.com/item?id=49605246).

### 6. Compliance asks "prove AI code was reviewed" (low frequency, high stakes)
- "Recently our compliance team asked: 'Can you prove all AI-generated code was properly reviewed?' We had no answer." — u/Ok_Stretch_6623, r/devops (date not visible; 25 comments), https://old.reddit.com/r/devops/comments/1u81nd0/how_are_you_tracking_aigenerated_code_in_your/
- Pushback in the same thread: "they don't actually need to know which lines an llm wrote. they need to prove every change was reviewed by a human who took responsibility. that control already exists if your prs require an approval"
- "Problem: No unified way to prove why an agent made a decision. Each vendor logs their piece, but the full decision chain is fragmented." — Ask HN, 4 pts, 2026-01-22, https://news.ycombinator.com/item?id=46714180

Who: fintech/healthcare teams, regulated enterprises. Today: PR approval rules plus git blame. Few use attestation.

### 7. Reviewers lack the "why" (intent) behind the change (medium)
- "When I review a change, it's almost never just about the actual code changes, but reviewing it in the context of what was initially asked" — embedding-shape, 2026-04-17, https://news.ycombinator.com/item?id=47806409
- "All of the examples simply describe what the code is doing. I need a tool that explains the intent and context behind a change." — SkyPuncher, 2026-04-17, https://news.ycombinator.com/item?id=47807104
- straymark (open source) runs "agents from three different models to perform the audits … (comparing intent against implementation)" — montfort, 2026-06-24, https://news.ycombinator.com/item?id=48662499

---

## Evidence FOR and AGAINST the hypotheses

### H1 — Nobody independently verifies AI agents' work across vendors
**Verdict: FALSE as stated. The space is crowded.**

AGAINST (it already exists):
- CodeRabbit (US$1.5B valuation) markets itself as the "independent AI code review layer" and asks "Who would independently verify all of that code?" (https://www.coderabbit.ai/blog/introducing-agentic-change-management).
- Greptile is built on "Independence" and has "never shipped codegen features" (https://www.greptile.com/blog/ai-code-review-bubble).
- zeroshot (1,994 stars, open source, self-hostable) enforces "the agent that writes the code cannot approve its own work" across Claude Code / Codex / Copilot.
- Users already do cross-vendor review by hand: "For Claude Code, I think the standard is Codex + Gemini. Why these two? Because it 'covers' the blind spots the others would miss by themselves." — nxy, 2026-06-19, https://news.ycombinator.com/item?id=48594257. Also "I also built a skill I call `/meta-review` that asks Codex, Cursor, and Gemini to review the code (I use Claude Code)." — atestu, 2026-06-05, https://news.ycombinator.com/item?id=48407013

AGAINST (people doubt "independence" matters):
- "Independence is ridiculous - the underlying llm models are too similar on their training days and methodologies to be anything like independent." — sdenton4, 2026-01-26, https://news.ycombinator.com/item?id=46769939
- "I disagree that independence is a competitive advantage. … I don't see why a company like Cursor can't create full independence between their coding and review products but still bundle them together" — rushingcreek, https://news.ycombinator.com/item?id=46771085
- "so they graciously accept that 'AI' agents generate slop … so why would they be any better at reviewing?" — blibble, 2026-01-21, https://news.ycombinator.com/item?id=46712003

FOR (a narrower version is still real):
- Users say no single reviewer is enough and trust is low (pain 1). The remaining gap is not "independent review" but **reviews people trust**: high precision, evidence-backed (ran the code, ran tests), and **deterministic** gates. Supporting: Greptile TREX "runs your code"; deet: "The ability to run code in a sandbox … plugging a big hole in the feedback loop" (https://news.ycombinator.com/item?id=48574626); sdenton4 suggests "automatically building and checking deterministic models of code … These types of checks really are model independent".
- HN comment on audits: "You need different incentives (a reviewer whose goal isn't to finish the work, but to find problems) and/or different blind spots" — supern0va, 2026-10-07, https://news.ycombinator.com/item?id=49996813

### H2 — Teams need a portable "proof of done" / audit record per AI change
**Verdict: PARTLY OPEN. Attribution is taken; verified-outcome records are not.**

AGAINST (already being built / not needed):
- Attribution standard exists with wide backing: Agent Trace (Cursor RFC with Cognition; InfoQ) and git-ai (2,853 stars, many agents).
- in-toto/SLSA predicates for AI code exist as early specs: OpenFab, agentattest, crashoverride guides.
- Many devs say "code is code": "Asking to prove it the AI code is reviewed is the same as asking if non-AI code is reviewed. You point them at the PR process." — r/devops thread above.
- Audit-trail launches on HN get almost no traction (2–9 points: Air black box, AATF spec, Viatoris, Gryph, Markovian). [Inference] Weak pull from developers; any demand comes from compliance buyers, not devs.

FOR:
- Agent Trace explicitly **does not** judge quality: it "does not attempt to assess code quality" (InfoQ). git-ai records who/what/prompt, not whether it was verified. So "what checks ran, what passed, who approved which exact tree" is still missing from the main standards. [Inference]
- A real compliance question with no answer: "Can you prove all AI-generated code was properly reviewed? We had no answer." (r/devops).
- Zylos (2026-07-31) describes the gap: review approves one SHA but a different artifact ships; it suggests to "bind the approval event to a specific tree hash — not a PR number" (https://zylos.ai/zh/research/2026-07-31-review-to-deploy-artifact-equivalence-agent-authored-code). It also cites CISA/NSA/Five Eyes guidance (May 2026) to treat coding agents as untrusted by default.
- Fragmentation across vendors: "Each vendor logs their piece, but the full decision chain is fragmented." (https://news.ycombinator.com/item?id=46714180). Also: "OpenAI encrypts Codex agent instructions, blocking local audit trail" (2026-07-17, https://news.ycombinator.com/item?id=48943426).

### H3 — Open, self-hosted, vendor-neutral alternatives are missing
Only the review part touches my theme. **For review: FALSE.** pr-agent (13k stars), alibaba/open-code-review (47k), kodus (1.5k, "Bring your own LLM"), vercel openreview (1.7k), mira, juror, zeroshot all exist. Demand for local/BYOK is real (jFriedensreich, r_p4rk quotes above; joshribakoff: "I dislike the idea of coupling my workflow to saas platforms like github or code rabbit", https://news.ycombinator.com/item?id=46661775).

### H4 — Agents should learn only from verified outcomes
Little direct evidence in this theme. Related:
- "I would love to see PR comments … be automatically synced back to the context coding agents have … There's no reason nowadays for an engineer or a team of engineers to make the same code quality mistake twice." — gracealwan, 2026-04-17, https://news.ycombinator.com/item?id=47799555
- sscarduzio built "pr-war-stories": "distilling knowledge from pr reviews back into Bugbot fine tuning and CLAUDE.md" (https://news.ycombinator.com/item?id=47799945).
- Vendors already sell this: CodeRabbit "learns from your team"; a VentureBeat article on Qodo 2.1, whose URL reads "qodo-2-1-solves-your-coding-agents-amnesia-problem" (posted to HN 2026-02-18, https://news.ycombinator.com/item?id=47059823). Nobody I read asked for "only verified" outcomes. [Inference] "Learn from accepted review feedback" is wanted; the "verified only" filter is my guess, not something users asked for.

---

## NEW open spaces suggested by the evidence

1. **Precision-first, evidence-backed verdicts ("show me it ran") rather than more comments** — Strength: **strong**. Noise is the top complaint across all sources. Users trust things that run code or are deterministic. Vetoo, Revix, TREX and Ito are all chasing this, so it is contested but not won. [Inference] An open referee whose output is a short pass/fail with attached evidence (tests run, repro, screenshots) fits better than another commenter bot.

2. **The joined record: authorship + verification evidence + approval bound to the exact tree, cross-vendor, signed** — Strength: **medium**. Agent Trace/git-ai cover authorship. SLSA/in-toto cover build. Nobody popular joins them to "what was checked and by whom." Demand comes mostly from compliance, not devs. Only specs exist (OpenFab, agentattest with 1 star). Risk: the "code is code" pushback, and GitHub could ship it natively.

3. **Deterministic "done" gates for agents (merge readiness as a red/green check)** — Strength: **medium**. gtg got 45 pts; pain 5 shows agents declaring victory early. Pushback: branch protection already does part of it. [Inference] Fits as a small feature of a referee, not a product by itself.

4. **Intent-vs-implementation review (did the change do what the ticket asked?)** — Strength: **medium**. Repeated asks for "intent and context". Linear/CodeRabbit are moving toward tickets, so it may close soon.

5. **Review-burden triage: decide what a human must read** — Strength: **weak-medium**. The bottleneck is human attention; CodeRabbit's new pitch is to "prioritize where developer attention and time generate the most value". Stage's sunset shows UI-only versions are hard to sell.

---

## Willingness-to-pay signals

- People do pay: "I installed CodeRabbit … pretty happy with the results, especially considering the low price ($15/user/mo I think)." — The_Fox, https://news.ycombinator.com/item?id=46771089
- "We've been using Coderabbit, great deal ($30/mo/dev flat) and finds a lot." — atestu, https://news.ycombinator.com/item?id=48407013
- "The ROI here is so high that I don't mind using the strongest model available for the actual code review." — plmpsu, https://news.ycombinator.com/item?id=48321333
- "I'd like to use a paid tool, but they don't support our workflow / tools. I made my own" — u/stolsson, 2026-08-01, r/codereview link above.
- Market money: CodeRabbit $143M Series C at $1.5B; 40-person team evaluating tools (https://news.ycombinator.com/item?id=48587808).
- Against paying: "Y'all are a bit nuts if you want 50% more per month than Claude Pro for this." — malcolmgreaves on Stage, https://news.ycombinator.com/item?id=47807461; "I don't see the point of paying for yet another CI integration doing LLM code review." — gherkinnn, https://news.ycombinator.com/item?id=46770484
- No one I read said they pay for AI provenance / attestation. [Inference] WTP for H2 is unproven; it would come from compliance budgets.

---

## What surprised me

- **Independence is already the marketing line of the two biggest pure players.** Greptile and CodeRabbit both use "independent" in their pitch. H1's core message is not new.
- **alibaba/open-code-review has 47,575 stars in five months.** Open-source review is not a gap; it is a land grab by big companies.
- **zeroshot already is "writer cannot approve its own work, any vendor, open source, self-host"** — very close to omni-agent's referee idea, with ~2k stars.
- **Many HN readers reject "independence" on technical grounds** (models share training data). Different-model review is seen as helpful but not truly independent. Deterministic checks are seen as truly independent.
- **Audit-trail projects are numerous but nobody upvotes them.** Dozens of repos, almost all 1–10 stars, HN launches at 2–9 points.
- **Cognition positioned Devin Review as a reading aid, not a judge** ("AI reviewing AI does lead to a cycle of ouroboros slop" — Cognition employee, https://news.ycombinator.com/item?id=46712183), and gave it away free.
- A Stage-inspired product shut down within ~3 months of a 130-point HN launch.

---

## Bottom line for omni-agent [Inference]

- H1 (independent cross-vendor verifier) is **crowded**: funded SaaS (CodeRabbit, Greptile), free vendor tools (Devin Review, Copilot, Bugbot), and open source (zeroshot, pr-agent, open-code-review). Don't lead with "independent review".
- H2 (proof of done) is **half-crowded**: authorship records are taken (Agent Trace, git-ai). A record of **what was verified, with evidence, approved against the exact tree, across vendors** is still open, but dev demand is weak and compliance demand is early.
- The best opening is to **combine them**: an open referee whose output is not comments but a signed, low-noise verdict with run evidence, built on Agent Trace / in-toto instead of a new format.
