# T2 Running many AI coding agents at once — market research

Theme: the pain of running Claude Code / Codex / Cursor / Copilot / Devin side by side (context loss, worktrees, merge conflicts, babysitting, handoffs, cost, picking the right agent for each task).
Research date: 2026-10-18. Every quote below was copied from a page or API response I actually retrieved. Quotes may be cut short ("...") but the words are not changed.

## Sources covered

| Platform | What I read | Date range | Notes |
|---|---|---|---|
| Hacker News (Algolia API) | 21 story searches (about 200 story titles); full comment trees of 22 threads (about 1,150 comments); 4 comment searches | 2026-01-02 → 2026-10-14 | Best source. Points and comment counts are given below. |
| Reddit (r/ClaudeCode, r/ClaudeAI, r/cursor, r/codex, r/AI_Agents, r/vibecoding) | About 30 threads found through web search. I could only read the **search-result snippets**, which include the post text and sometimes comments. | 2026-01-28 → 2026-08-18 | **Could not open threads directly.** reddit.com `.json` returned 403, old.reddit returned 302, the browser showed "You've been blocked by network security", and web_extract timed out or returned empty content. So my Reddit quotes come from the snippet text shown in search results, and I can't see full comment trees or most scores. |
| GitHub issues (gh CLI) | 7 searches across anthropics/claude-code and openai/codex, sorted by reactions; read 4 issue bodies | 2026-01-03 → 2026-07-09 | GitHub's search matching is loose, so many top results are general (usage limits, multi-account) rather than about orchestration. |
| Other | GitHub READMEs (gastown, claude-fleet, agent-parallel-merge), Claude/GitHub docs, dev.to posts (from search snippets) | 2026 | Used only as context. |
| r/LocalLLaMA | Searched, but found nothing on-theme worth quoting | — | Treat as not covered. |

## Big picture in one paragraph

Running several coding agents in parallel is now normal for power users, and the **tools layer is badly overcrowded**. I counted more than 40 "Show HN" posts in 2026 for parallel-agent managers: tmux TUIs, kanbans, worktree managers, Conductor, Gas Town, Vibe Kanban, Claude Squad, Omnara, Optio, agent-deck, Agent-Manager, Foremerge, Clash, Wit, Grove, claude-fleet, Handoff, Ctx, and more. Most of them get 2-10 points. The few that get 100-280 points are attached to a known brand (Zed, Omnara/YC, Optio) or to a discussion people want to have. The pain users actually describe is **not** "I can't launch N agents." That part is solved. The pain comes **after** the agents run: human review can't keep up, agents drift into conflicting assumptions, nobody can tell which agent is stuck or which ones are actually done, test environments aren't isolated, and token limits and pricing keep changing. [Inference]

## Top pain points ranked by evidence

### 1. Human review and attention, not compute, is the limit. More agents means more diffs than one person can check.
- **How often:** This is the most frequent theme. It came up in every big HN thread (Zed 279 pts, Kanbots 263 pts, tmux 189 pts, "Agent orchestration for the timid" 128 pts), in 2 "Ask HN" threads about review fatigue, in an Oct 2026 Ars Technica study posted to HN, and in r/ClaudeCode.
- **Quotes:**
  - "the bottleneck moves from generation to review. agents parallelize, humans review sequentially — 8 parallel cards means 8x the diffs to read" (cold_harbor, https://news.ycombinator.com/item?id=48246797, 2026-05-23, Kanbots thread 263 pts)
  - "I tried pushing 3 instances building three different features at the same time in separate worktrees and I was quickly stretched too thin. I ultimately started letting too much slop through" (u/Frequent-Contract925, https://www.reddit.com/r/ClaudeCode/comments/1qplamr/how_many_claude_code_instances_are_you_all_able/, 2026-01-28, 2 pts, 16 comments)
  - "Even when I really tried to do this in parallel I could not meaningfully run more than three sessions." (koliber, https://news.ycombinator.com/item?id=46716601, 2026-01-22)
  - "It overwhelms everyone's ability to keep track of what it's doing. Some people are just no longer keeping track." (SCdF, https://news.ycombinator.com/item?id=46751638, 2026-01-25)
  - "the review process has become a severe bottleneck to me. The agents are producing a massive volume of PR." (Ask HN, https://news.ycombinator.com/item?id=47418016, 2026-03-17)
- **Who has it:** Solo devs running 2-6 sessions, and team leads who receive the agents' PRs.
- **What they use today:** They cap themselves at 2-3 agents, use "another agent" as reviewer (https://news.ycombinator.com/item?id=46557628), use plan/develop/review loops, or simply stop reviewing.
- **Existing products:** Devin Review, Copilot review, Cursor's review UI, and Plannotator / octokraft (small). Users say none of them fix the problem. Most "review" is the same vendor checking its own work. One counter-voice: "I never look at code ... the quality is so good" (stavros, https://news.ycombinator.com/item?id=48241421, 2026-05-22).

### 2. Semantic drift between parallel agents. Worktrees fix file clashes, not decision clashes.
- **How often:** High. HN Zed thread, HN Foremerge (45 pts, Sep 2026), r/ClaudeCode (at least 6 threads on merge safety, swarming, and "agent A breaks agent B"), r/cursor, and many tools (Foremerge, Clash, Grove, Wit, claude-fleet, agent-parallel-merge).
- **Quotes:**
  - "The worktree part is the easy half ... The hard problem is architectural consistency. Agent A renames a type to X. Agent B, in a different worktree, independently renames the same type to Y" (ArielTM, https://news.ycombinator.com/item?id=47870607, 2026-04-23)
  - "Orchestration buys parallelism, not coherence. More agents means more drift between assumptions. Past a point you're just generating merge conflicts with extra steps." (nulone, https://news.ycombinator.com/item?id=46754137, 2026-01-25)
  - "And when you hit \"merge conflicts\", you will realise that Claude just can't maintain enough context to resolve them." (u/Tushar_BitYantriki, 57 pts, https://www.reddit.com/r/ClaudeCode/comments/1rae7sa/5_claude_code_worktree_tips_from_creator_of/, 2026-02-21)
  - "I've been running Claude Code and Codex on the same codebase for about 8 weeks now. Both open PRs on separate branches. Both pass CI individually." (r/ClaudeCode, https://www.reddit.com/r/ClaudeCode/comments/1t9prcp/how_are_you_handling_merge_safety_when_running/, 2026; snippet only)
  - "I ran 3 agent sessions in parallel on one feature and they corrupted our shared decisions in under an hour" (thread title, u/Admirable-Fun2297, https://www.reddit.com/r/cursor/comments/1ws79go/, 2026)
- **Who has it:** Solo devs and small teams running 3+ agents on the same repo.
- **What they use today:** A "shared decision log," a single goal file, splitting scope so tasks don't overlap, merge gates that rebase onto main and rerun CI, and cross-model review ("Codex reviewing Claude's diff, or vice versa", from the 1t9prcp snippet).
- **Existing products:** Foremerge (intent conflicts, open source), Gas Town/Beads, and claude-fleet ("sequential merge gate"). Users are lukewarm: "I haven't really felt they've earned their keep" (gavmor, https://news.ycombinator.com/item?id=49811149, 2026-09-23).

### 3. "Is it actually done, and which agent needs me?" Status, stuck agents and false "success"
- **How often:** Medium-high. This is the pitch of almost every TUI/dashboard launch (Agent-Manager 98 pts / 80 comments, Omnara 147 pts, OCD, Arcane Agents), and it shows up in comments.
- **Quotes:**
  - "I can't tell what state any of them is in without tabbing through every terminal. One of them is usually sitting on a permission prompt I never saw." (yoanwaidev, https://news.ycombinator.com/item?id=49107750, 2026-07-30, 98 pts thread)
  - "Coding agents are pretty good at telling you they finished something they didn't actually finish." (RaysonTech, https://news.ycombinator.com/item?id=49863043, 2026-09-27)
  - "You put a ticket in to \"make random.md document\" and it creates a ton of unit tests and makes \"notrandom.md\" and says \"success\"." (kaydub, https://news.ycombinator.com/item?id=49112561, 2026-07-30)
  - "surfacing/managing session state is only part of the issue, context switching between them is another entirely and feels like the real productivity bottleneck" (Arthurian, https://news.ycombinator.com/item?id=49112018, 2026-07-30)
- **Who has it:** Power users with 3-15 sessions, and enterprise teams running "fire and forget" Jira agents.
- **What they use today:** tmux, OS notifications, Conductor, Agent-Manager, agent-deck, herdr, Omnara (mobile), and GitHub's Agents tab.
- **Existing products:** Dashboards show running vs. waiting. None of them check whether the "done" claim is true. [Inference]

### 4. Environment isolation beyond git: ports, databases, secrets, test data
- **How often:** Medium. HN Zed thread, HN "Git worktrees are not an isolation boundary" (32 pts / 35 comments), sandbox launches (Agent37 162 pts, Runtime 103 pts, Coop 71 pts, Supapool 31 pts).
- **Quotes:**
  - "with worktrees I am not really able to easily copy secrets, etc to run my app, ports conflict, I end up with a bunch of separate dbs and services" (Atotalnoob, https://news.ycombinator.com/item?id=47871666, 2026-04-23)
  - "real blocker for us was test data isolation ... reasoning about which agent broke teh shared migration when three of them touch it got old fast. we just run one agent at a time now" (kippinsula, https://news.ycombinator.com/item?id=47870590, 2026-04-22)
  - "what's a pain is having to compile from scratch every time, and not carrying over gitignored files." (https://news.ycombinator.com/item?id=49110949, 2026-07-30)
- **Who has it:** Developers working on full-stack apps that use Docker or a DB, and teams.
- **What they use today:** `.worktreeinclude`, direnv port hashing, custom hooks that copy `.env`/node_modules, devcontainers, per-branch Postgres schemas, and cloud sandboxes.
- **Existing products:** Sandboxes are becoming a commodity. One commenter: "the sandbox part is getting commoditized" (talkarimli, https://news.ycombinator.com/item?id=50063391, 2026-10-12).

### 5. Cost, quota and vendor pricing whiplash
- **How often:** High in volume, but it's mixed in with general limit complaints. GitHub claude-code #16157 "Instantly hitting usage limits with Max subscription" (695 👍) and #38335 (476 👍). Codex #14593 "Burning tokens very fast" (285 👍). Two HN threads on the `claude -p` / Agent SDK billing changes (May, Jun, Oct 2026).
- **Quotes:**
  - "the parallel agents burn though tokens extremely quickly and hit Max plan limits in under an hour." (ecliptik, https://news.ycombinator.com/item?id=47221592, 2026-03-02)
  - "Even Claude Max x1 if you run 2 agents with Opus in parallel you're going hit limits." (0x457, https://news.ycombinator.com/item?id=47222550, 2026-03-02)
  - "headless usage will become 20-30 times more expensive, and all these high level orchestrator tools/workflows depend on it." (Guillaume86, https://news.ycombinator.com/item?id=48240632, 2026-05-22)
  - "Oh, for goodness sake, can these people just make up their minds? I was just adjusting my workflow to leverage the SDK and `-p`" (Tadpole9181, https://news.ycombinator.com/item?id=49998983, 2026-10-07)
  - "Run one used 22 agents, about 2.9 million subagent tokens, and ran for almost three hours. It failed when one scout stalled six times in a row" (u/Difficult-Sugar-4862, https://www.reddit.com/r/ClaudeAI/comments/1uqwe2s/, 2026-07-08, 4 pts)
- **Who has it:** Everyone running more than 2 agents. Orchestrator builders are hit hardest, because their tools depend on subscription headless access that the vendors keep changing.
- **What they use today:** Multiple $200 plans ("More like 2x$200 plans", https://news.ycombinator.com/item?id=47224504), "multi-harnessing" across Claude/Codex/Cursor to spread limits (r/ClaudeCode 1vjrwhy), cheaper models for workers, and ccusage.
- **Existing products:** None manage a budget across vendors. Endy ("orchestrate multiple coding-agent CLIs to cut LLM spend", 2 pts) is tiny.

### 6. Context loss and handoff between sessions and between vendors
- **How often:** Medium. r/ClaudeCode/r/ClaudeAI (at least 5 threads), HN Ctx "/resume across Claude Code and Codex" (72 pts), Handoff, Ghost, PMB, and the reinstate.dev and session-handoff plugins.
- **Quotes:**
  - "the biggest pain point is starting fresh every session ... It works, but it feels like I'm building infrastructure that should exist." (r/ClaudeCode, https://www.reddit.com/r/ClaudeCode/comments/1qn5tfc/how_do_you_handle_context_loss_between_claude/, 2026)
  - "I have like 15 concurrent sessions I leave up for weeks, 50% Codex 50% Claude Code, even though I know they work better with fresh context." (meowface, https://news.ycombinator.com/item?id=47855179, 2026-04-21, Ctx thread)
  - "Each agent has no shared memory of the decisions the other" (comment snippet, https://www.reddit.com/r/ClaudeCode/comments/1rae7sa/, 2026-02)
- **Who has it:** Solo devs who use more than one vendor.
- **What they use today:** HANDOFF.md files, CLAUDE.md/AGENTS.md, Beads, and hooks. Claude Code still lacks native AGENTS.md support (GitHub #31005, 417 👍), which shows that cross-vendor config friction is real.
- **Existing products:** Many small open-source tools. No standard has emerged.

### 7. Which agent for which task (routing across vendors)
- **How often:** Medium in r/codex and r/ClaudeCode. I found little on HN.
- **Quotes:**
  - "using claude code and codex together ... (claude models for planning, codex for implementing)" (https://www.reddit.com/r/ClaudeCode/comments/1vs1hpz/, 2026-08-18)
  - "In my experience, Codex is far better for any complex backend work and code quality." (u/Stovoy, 32 pts, https://www.reddit.com/r/codex/comments/1sro3f3/claude_or_codex_or_both/, 2026-04-21)
  - "Claude plan - codex review plan - claude review the review of the plan" (u/mrgoditself, 3 pts, https://www.reddit.com/r/ClaudeCode/comments/1t70rwk/run_both_claude_code_and_codex/, 2026-05-08)
  - Codex issue #31814: "This setting makes it impossible to use Terra/Luna subagents." (176 👍, https://github.com/openai/codex/issues/31814, 2026-07-09). Users want per-subagent model choice.
- **What they use today:** Routing by hand based on gut feel. GitHub's Agents tab lets you pick Copilot, Claude or Codex for a task (docs.github.com agent-management page).
- [Inference] People do cross-vendor routing and cross-review by hand, and their choices are anecdotal. Nobody keeps track of which agent actually succeeded on which kind of task.

### 8. Fatigue and skepticism: "I went back to one agent"
- This is real and should be weighed against all of the above.
  - "I personally explicitly avoid parallel agents, since it creates too much cognitive debt" (VortexLain, https://news.ycombinator.com/item?id=47874992, 2026-04-23)
  - "it looks cognitively like being a pilot landing a plane all day long, and not what I signed up for." (medi8r, https://news.ycombinator.com/item?id=47224060, 2026-03-02)
  - "I use coding agent but still don't see the need to manage a swarm of them like this. Is there an actual use case for this or this is just for tokenmaxxing?" (mrbonner, https://news.ycombinator.com/item?id=49111645, 2026-07-30)
  - "anything that made sense gets released a week after they finish building from anthropic/openai/etc." (kaydub, https://news.ycombinator.com/item?id=49112561, 2026-07-30)

## Evidence for and against the hypotheses

### H1: Nobody independently verifies agents' work across vendors
- **FOR:** Users already cross-check by hand, and it seems to work for them: "a separate review agent running a different model on the diff right before merge. Codex reviewing Claude's diff, or vice versa." (snippet, https://www.reddit.com/r/ClaudeCode/comments/1t9prcp/). False "done" claims are a named pain (RaysonTech 49863043; kaydub 49112561). Agents "enshrine" bugs in their own tests (Atotalnoob 47871666). Nobody points to a vendor-neutral verifier as the tool they use.
- **AGAINST:** "Another agent" is the reflex answer and is cheap to do by hand (https://news.ycombinator.com/item?id=46557628). Anthropic's own docs push self-verification as "the single most impactful tip" (support.claude.com power-user tips). Small tools already exist (Nom Army, octokraft, Foremerge). Verification only helps if it cuts *human* review time. Another agent's opinion adds one more thing to read. [Inference]
- **Verdict:** Supported for parallel setups. The specific angle that's missing is verification that **reduces the human review queue**, not one more AI review comment. [Inference]

### H2: A portable "proof of done" record for each AI change
- **FOR:** Users can't tell state or truth across sessions (pain #3). "someone is responsible for what happens, then you arn't going to find that with AI coding bots" (cyanydeez, https://news.ycombinator.com/item?id=50031812, 2026-10-10). An incident write-up describes a push carrying "another session's three unfinished commits" that was deployed before review (agents.stackoverflow.com blueprint snippet). That is a provenance failure.
- **AGAINST:** No one in my sample asked for an "audit record" by name. Solo devs want *less* to read. [Inference] Proof-of-done is valuable if it is a short, machine-checked summary that lets you skip reading the diff. A long log is not useful to them.
- **Verdict:** Weak-to-medium direct evidence in this theme. The strongest form is "evidence that lets me merge without reading every line." The HN "Agentic coding workflows built on Git worktrees and task evidence" post (10 pts, https://news.ycombinator.com/item?id=48614286) is a small signal that others are trying this.

### H3: Open, self-hosted, vendor-neutral alternatives are missing
- **AGAINST (strong):** For *orchestration* specifically, open-source, vendor-neutral tools are everywhere: Gas Town (Claude Code, Copilot, Codex, Gemini), Vibe Kanban, Claude Squad, Kanbots (263 pts, open source), Optio (K8s, open source), Agent-Manager, agent-deck, Foremerge, claude-fleet, Zed parallel agents, and openchamber. "there are a ton of sandboxing providers" (https://news.ycombinator.com/item?id=50060240). Building one more launcher would be entering a crowded space.
- **FOR (narrow):** Vendor lock-in is a pain. Anthropic blocks Max plans in OpenCode (claude-code #17118, 796 👍: "Blocking use of Max plan with Opencode will almost certainly result in me downgrading or cancelling"). Billing changes break third-party orchestrators (pain #5). Teams lack tooling: "Most workflow tools i've seen so far focus on making a single engineer get more out of a claude/codex subscription but not much how teams as a whole can become more productive." (suls, https://news.ycombinator.com/item?id=47228938, 2026-03-02).
- **Verdict:** Open-source *solo* orchestration is crowded. Open-source *team-level* coordination is under-served (shared decision log, who owns what, merge queue across vendors). [Inference]

### H4: Agents should learn only from verified outcomes
- **FOR (indirect):** People route by gut feel (pain #7). Skills and plugins spread wrong information: "Individuals building plugins/skills/mcps that are duplicated or just plain wrong" (kaydub, https://news.ycombinator.com/item?id=49112844). Memory tools persist whatever happened, whether or not it worked. [Inference]
- **AGAINST:** No user in this theme asked for this directly. Beads/Gas Town was praised and then criticised for feature creep ("Then Steve Yegge went off the multi-agent deep end and it became a mess of feature creep", theshrike79, https://news.ycombinator.com/item?id=48398146).
- **Verdict:** Not tested well by this theme. Plausible, but no direct demand.

## New open spaces suggested by the evidence (not in H1-H4)

1. **A review-queue reducer for parallel agents (strength: STRONG).** Rank and group many agent diffs, say which ones were machine-verified, which ones overlap, and which actually need human eyes. The evidence is pain #1, which is everywhere, plus the Oct 2026 study post. Today's tools spawn agents but leave the merge desk to the human. [Inference]
2. **A shared decision and intent ledger across vendors (strength: MEDIUM-STRONG).** One file or service that every agent (Claude/Codex/Cursor) must read before starting and must write to when it makes a decision (renames, contracts, schema). This targets drift (pain #2). Foremerge and Beads touch it, but users are lukewarm and nothing is standard. It fits AGENTS.md-style portability. [Inference]
3. **A cross-vendor budget and quota scheduler (strength: MEDIUM).** Spread work across the subscriptions a user already pays for (Claude Max, ChatGPT Pro, Cursor) and reroute when one hits its limit. "Multi-harnessing due to token limits" is already done by hand. The risk is that vendor terms of service can break it at any time (May/Jun/Oct 2026 Anthropic changes). [Inference]
4. **Full-stack environment isolation for each agent (strength: MEDIUM, but crowded).** Ports, DB branches and secrets for each worktree. Sandboxes are being commoditized. The remaining gap is a "credential broker" layer: "I would pay more for that broker layer than for another 2x cheaper sandbox." (sscaryterry, https://news.ycombinator.com/item?id=50074525, 2026-10-13).
5. **Team-level (not solo) multi-agent coordination (strength: MEDIUM).** Who on the team dispatched which agent, and whose agents collide. suls (47228938) and Runtime's launch (team sandboxes, 103 pts) hint at this. [Inference]
6. **A "which agent wins on what" scoreboard from real outcomes (strength: WEAK-MEDIUM).** This turns anecdotal routing (pain #7) into data, and it connects to H4. [Inference]

## Willingness-to-pay signals

- People already pay $100-$400+/month on subscriptions to run agents in parallel: "More like 2x$200 plans" (https://news.ycombinator.com/item?id=47224504); "I pay for the $200 Claude Code plan just so I never have to think about usage" (https://news.ycombinator.com/item?id=47870560); "I pretty easily hit $2000/mo in API equivalent credits" (https://news.ycombinator.com/item?id=48136336).
- They resist paying for the **wrapper**: Omnara at $20/mo was called "overly expensive when theres an open source end to end encrypted project ... If it was 5$ I would've considered it." (https://news.ycombinator.com/item?id=47034957, 2026-02-16); "for people on $20/mo codex/claude plans 10 sessions is too few and adding $20/mo for omnara doesn't make sense" (https://news.ycombinator.com/item?id=47003215).
- They will pay for security and control layers: "I would pay more for that broker layer than for another 2x cheaper sandbox." (https://news.ycombinator.com/item?id=50074525).
- Cancel threat over lock-in: claude-code #17118 (796 👍).
- [Inference] Money flows to inference and sandboxes. Orchestration UIs are treated as free or open source. A paid tier probably has to sell trust, security or team features, not "run N agents."

## What surprised me

- **How crowded orchestration is.** More than 40 parallel-agent launchers on HN in 10 months, mostly at 2-10 points. One commenter called them "engineering as marketing for running agents in the cloud" (pbjerkeseth, https://news.ycombinator.com/item?id=49110056). Another said vendors ship the same feature a week later. Another orchestrator would be a weak wedge.
- **People cap themselves at 2-3 agents.** This shows up again and again (koliber, Frequent-Contract925, BosunoB, Gas Town review: "2-3 concurrent is the realistic limit", https://www.reddit.com/r/ClaudeCode/comments/1qur3qq/, 2026-02-03). The limit is human, not technical.
- **Worktrees are considered "solved."** The pain moved up to meaning (decisions) and down to environment (ports, DBs, secrets).
- **Vendor billing policy is a platform risk.** Anthropic changed headless/SDK subscription rules in May, paused the change in June, and changed it again on Oct 7, 2026. Orchestrators that wrap `claude -p` broke or nearly broke ("rip conductor :(", https://news.ycombinator.com/item?id=48130781).
- **Cross-vendor review is already folk practice** (Codex reviews Claude and the other way round), but nobody has made it a product.
- **A real split in the community:** some people say they no longer read code at all, while others say that will "get seriously burned" (https://news.ycombinator.com/item?id=48279375). Any verification product has to serve both groups.

## Limits of this research

- I could not open Reddit threads, so Reddit scores and comments are partial (taken from search snippets). Reddit is probably under-weighted compared with HN.
- HN skews toward experienced, skeptical engineers. Cursor and Copilot users (more mainstream) are under-represented.
- GitHub issue search was loose. The reaction counts I cite are real, but most of those issues are about general usage limits, not orchestration as such.
