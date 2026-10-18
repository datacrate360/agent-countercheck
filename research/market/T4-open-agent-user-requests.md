# T4 — What users of the big open agents complain about and ask for — market research

Research date: 2026-10-18. Analyst theme T4. Sources were read-only (no login, no posting, no reactions).

## Sources covered

| Platform | What was pulled | Count | Date range |
|---|---|---|---|
| GitHub issues (via `gh`) | Top 40 issues by reactions, all states, per repo: NousResearch/hermes-agent, openclaw/openclaw, paperclipai/paperclip, odysseus-dev/odysseus, OpenHands/OpenHands | 200 issues ranked | 2024-03 → 2026-10 |
| GitHub issues | 16 keyword searches per repo (verify, review, approval, audit, rollback, trust, proof, "said done", fake, claims...) sorted by reactions | ~270 unique hits | 2024 → 2026-10 |
| GitHub issues | Full body + top comments read for 36 issues | 36 | 2025-01 → 2026-09 |
| GitHub search counts | Full-text phrase counts ("fake tool", "says it did", "marked done", "regression" in title) | 21 counts | all time |
| Hacker News (Algolia API) | Story search for each product since 2026-01-01; comment search for 5 pain phrases; full read of 4 threads | ~50 stories, ~50 comments, 4 threads | 2026-01 → 2026-10 |
| Reddit | 6 searches; 10 threads tried | 5 threads read | 2026-03 → 2026-10 |

Repo size context (from GitHub API today): openclaw 392k stars / 9.4k open issues; hermes-agent 255k stars / ~50k open issues+PRs; paperclip 104k stars / 6.1k open; odysseus 94k stars / 1.5k open; OpenHands 91k stars / 1.1k open.

**Inaccessible / limits:**
- Reddit `.json` via curl was blocked (HTML wall) for all 10 threads. `web_extract` worked for 5 threads; 2 returned 403 (r/aisolobusinesses Paperclip thread, r/openclaw "OpenClaw vs Paperclip" thread); r/AI_Agents "How are you monitoring what your OpenClaw agents actually do" hit a "Prove your humanity" wall. Some extracts did not show the post date; those are marked "date not shown".
- Discord: not reachable without login. Not used.
- OpenHands: the old `All-Hands-AI/OpenHands` path returned nothing; the repo now lives at `OpenHands/OpenHands`. Its most-reacted issues are mostly 2024–2025; its 2026 issues have very few reactions (max 20). So OpenHands is a weak signal for 2026.
- GitHub reaction counts below are total reactions (all emoji) unless marked 👍. Phrase counts from GitHub search are rough (full-text match, can include PRs' linked text and noise).

---

## Top unmet needs ranked by reactions (evidence)

Ranking method [Inference]: I grouped the 200 top-reacted issues into themes and summed reactions of the issues in each theme, then checked HN/Reddit for the same theme. Rank = reactions first, then how often it appears across repos and platforms.

| Rank | Theme | Approx. reactions on top issues (sum) | Repos where it shows | Also on HN/Reddit? |
|---|---|---|---|---|
| 1 | Use my own model / subscription / local LLM (provider access, OAuth, bans) | ~700+ | all 5 | Yes, the biggest HN threads of the year (1349, 1099, 802 points) |
| 2 | Stability: installs and upgrades that keep breaking | ~400+ | openclaw, hermes, paperclip, odysseus | Yes, very often |
| 3 | Desktop/chat channels/UX (apps, themes, messaging platforms) | ~600 (but feature asks, not pain) | hermes, openclaw, paperclip | Some |
| 4 | Multi-agent orchestration + interop (A2A, ACP, roles, adapters) | ~250 | hermes, paperclip, OpenHands | Yes |
| 5 | Cost/token visibility and budgets | ~150 on GitHub; large on Reddit/HN | paperclip, openclaw, hermes | Yes, very often |
| 6 | Permissions, approvals, human-in-the-loop, multi-user | ~200 | openclaw, hermes, paperclip, odysseus | Yes |
| 7 | Memory, shared knowledge, backup of agent state | ~150 | hermes, paperclip, openclaw | Some |
| 8 | Maintainers drowning in AI-made PRs / "AI slop" in the agent's own codebase | ~200 | odysseus, openclaw, paperclip | Yes |
| 9 | "Agent says it did it but didn't" / fake tool calls / false "done" | ~45 reactions, but high phrase counts | all 4 active repos | Yes |
| 10 | Remote brain, local hands (split runtime) | ~140 | hermes | Few |
| 11 | Silent / undisclosed behavior (hidden routing, hidden installs, forced attribution) | ~45 | hermes, paperclip | Some |

Below, each pain point in detail. Quotes are copied from pages I retrieved.

### 1. "Let me use the model I already pay for" (providers, subscriptions, local models)
- **Pain:** users cannot plug in their own subscription, local model, or cheaper provider; vendors ban or bill third-party harness use.
- **How often:** the single biggest cluster. Paperclip's #1 issue (Ollama, 79 reactions), hermes #3 (Claude subscription OAuth, 57), plus Mistral (35), OpenRouter (35), Copilot CLI adapter (44), Antigravity (31), Vertex (28), Azure (23), and more.
- **Quotes:**
  - "Claude-subscribed users effectively pay twice (subscription + per-token API)." — hermes #25267, 2026-05-13, 57 reactions. https://github.com/NousResearch/hermes-agent/issues/25267
  - "Feature Request: Support for Local LLMs via Ollama" — paperclip #187, 2026-03-07, 79 reactions (68 👍). https://github.com/paperclipai/paperclip/issues/187
  - HN story "Tell HN: Anthropic no longer allowing Claude Code subscriptions to use OpenClaw" — 2026-04-03, 1099 points, 827 comments. https://news.ycombinator.com/item?id=47633396
  - HN story "Google restricting Google AI Pro/Ultra subscribers for using OpenClaw" — 2026-02-22, 802 points. https://news.ycombinator.com/item?id=47115805
  - "I had it working great ... on the Max ($200/mo) plan. Then they intentionally broke it even though my usage was completely within their stated/published usage limits" — HN @sneak, 2026-04-15. https://news.ycombinator.com/item?id=47785371
- **Who:** solo devs and hobbyists on subscriptions; small teams on enterprise seats.
- **Workarounds:** model routers (LiteLLM, Bifrost), Ollama, OpenRouter, switching harness.
- **Who solves it:** each harness adds providers one by one. Nobody solves the vendor-policy risk. [Inference] This is a commodity race, not an open space, but omni-agent must be fully model-neutral to be credible.

### 2. Stability: "every update breaks it"
- **Pain:** installs fail, upgrades break tools, channels and config.
- **How often:** very high. GitHub search: 994 openclaw issues and 247 hermes issues with "regression" in the title. OpenClaw froze new features for a "Stabilisation Mode".
- **Quotes:**
  - "we're getting a PR roughly every two minutes. ... no human team can review at that pace and maintain quality." — OpenClaw maintainers, #5799, 2026-02-01, 75 reactions. https://github.com/openclaw/openclaw/issues/5799
  - "I ran the bootstrap 3 times and the AI just talks and never edits a single file. Ever. It just says it did and it doesn't." — openclaw #57898, 2026-03-30, 11 reactions. https://github.com/openclaw/openclaw/issues/57898
  - "it was so fragile and unknowable and 15% broken at every moment." — HN @browningstreet, 2026-04-15. https://news.ycombinator.com/item?id=47786210
  - "I just think the trust layer needs to be as strong as the agent layer: boring, explicit, operator-grade upgrade checklists and compatibility gates" — r/openclaw "OpenClaw or BrokenClaw?" comment (date not shown in extract). https://www.reddit.com/r/openclaw/comments/1uftow7/openclaw_or_brokenclaw
  - "Stability was the moat the whole time. It always is." — r/better_claw post (date not shown). https://www.reddit.com/r/better_claw/comments/1v2i9qd/what_happened_to_openclaw_the_answer_isnt
- **Who:** everyone; worst for non-coders and people running agents 24/7.
- **Workarounds:** pinning old versions, running in Docker, switching to Hermes ("Dont use openclaw, use hermes." — HN @incomingpain, 2026-04-16, https://news.ycombinator.com/item?id=47791703).
- **Who solves it:** Hermes is praised as more stable by several HN users, but hermes also has 247 "regression" titles.

### 3. Desktop, chat channels, UX
- Highest raw reactions: openclaw "Linux/Windows Clawdbot Apps" (95), hermes "Desktop Client-Only Installation" (74), hermes dashboard themes (67), openclaw Feishu (75), DingTalk (59), paperclip "Chat with agents" (49).
- [Inference] Real demand, but it is table stakes, not an open space for a verification-focused product. Note: Paperclip users want chat even though Paperclip says "Not a chatbot. Agents have jobs, not chat windows." (comment on paperclip #49, 11 reactions) https://github.com/paperclipai/paperclip/issues/49

### 4. Multi-agent orchestration and interop
- **Pain:** one agent is not enough; people want roles, handoffs, and calling other vendors' agents.
- **Evidence:** hermes A2A protocol (53 reactions, #514), multi-agent architecture (38, #344), delegate_task agent profiles (32, #9459), generalized ACP client to drive other CLIs (27, #5257); paperclip adapters for Copilot CLI (44), Antigravity (31); OpenHands "Multiple LLM working in sync" (6).
- **Quotes:**
  - "Today, Hermes is one agent that can spawn throwaway child agents ... That's delegation, not multi-agent." — hermes #344, 2026-03-04, 38 reactions. https://github.com/NousResearch/hermes-agent/issues/344
  - "I was running 14 different instances ... I burned through a lot of tokens and didn't end up with much to show for it." — HN @nathan_f77, 2026-04-16. https://news.ycombinator.com/item?id=47787728
  - "whenever I give it a longer project, the system often gets stuck halfway through execution" — r/openclaw, 2026-04-15, 9 points. https://www.reddit.com/r/openclaw/comments/1sltjrt/what_actually_makes_ai_agents_reliable_on_long/
  - "Reliable long runs usually come from reducing autonomy, not increasing it. ... force every handoff through review and stop points" — same thread, u/agentXchain_dev, 3 points.
- **Who:** power users and small teams running fleets.
- **Who solves it:** Paperclip (org-chart of agents), Hermes Kanban + profiles, OpenClaw Lobster workflows. Users say they still stall and burn tokens.

### 5. Cost and token visibility
- **Pain:** agents burn tokens silently (heartbeats, crons, big prompts); subscription runs show $0 so budgets do not work.
- **Evidence:** paperclip #339 (16), #1756 token budgets (11), #498 subscription vs API cost (11), #212 cost dashboard shows $0 (5); hermes lazy tool schema loading (21), headroom compression (17); openclaw RTK compress tool output (21), DeepSeek cache broken after upgrade (14 and 8).
- **Quotes:**
  - "a single `codex_local` agent consumed **19.8M input tokens** in one hour ... Because `cost_cents = 0`, there was: No budget enforcement" — paperclip #339, 2026-03-08, 16 reactions. https://github.com/paperclipai/paperclip/issues/339
  - "the heartbeat fires every few seconds and eats your tokens alive. I was literally watching money disappear while nothing was happening." — r/openclaw "Spent more on Paperclip heartbeats than on rent" (date not shown). https://www.reddit.com/r/openclaw/comments/1seolaj/spent_more_on_paperclip_heartbeats_than_on_rent
  - "$200 in 9 days. Then I turned it off." — u/microChasm, r/openclaw, 2026-03-17, 6 points. https://www.reddit.com/r/openclaw/comments/1rvsbyr/how_much_are_you_spending_on_tokens_with_openclaw/
  - "Someone posted this week about a $350 bill over 2 days. ... a cron job was firing every minute instead of every hour" — r/openclaw PSA, 2026-04-26, 5 points. https://www.reddit.com/r/openclaw/comments/1svx74o/psa_your_openclaw_is_probably_burning_tokens_on/
  - "I kept failing to answer a simple question: what did they actually do last night, and what did it cost me?" — Show HN: Preloop, 2026-07-28. https://news.ycombinator.com/item?id=49083042
- **Who:** solo devs on API keys; teams on subscription or enterprise seats.
- **Workarounds:** cheap model for heartbeats, context sidecars, external dashboards (Grafana for Hermes, Show HN 28 points, https://news.ycombinator.com/item?id=49318128).
- **Who solves it:** Preloop (self-hosted control plane), Grafana integration; partial in Paperclip budgets.

### 6. Permissions, approvals, human-in-the-loop, multi-user
- **Pain:** agents run destructive or outbound actions without asking; no roles for families/teams; no clean "human task" queue.
- **Evidence:** openclaw RBAC (28, #8081), execution guardrails (5, #6823), allow-always exec policy (9); hermes gateway permission tiers (24, #527), multi-tenant (16, #34352); paperclip SSO (17), replace an agent with a human (15, #188), agents should create issues for humans when blocked (12, #923), pause whole company (9), permission grants API (8), pipeline-step approvals (4, #762); odysseus SSO (18), destructive-op safeguards (2, #2870).
- **Quotes:**
  - "This has caused real incidents: Agent deleted OAuth credentials trying to \"fix\" an auth issue" — openclaw #6823, 2026-02-02. https://github.com/openclaw/openclaw/issues/6823
  - "it decided without my approval to automatically clean certain directories like ~/Downloads removing 100GB of files from it." — odysseus #2870, 2026-06-05. https://github.com/odysseus-dev/odysseus/issues/2870
  - "The human (\"The Board\") has no clear queue of things they need to act on." — paperclip #923, 2026-03-15, 12 reactions. https://github.com/paperclipai/paperclip/issues/923
  - "We're considering building this as a plugin ourselves since we can't wait." — comment on paperclip #762 (approvals), 2026-04-05. https://github.com/paperclipai/paperclip/issues/762
  - "it only has readonly access to both, as I dont trust it to do stuff for me or on my behalf." — HN @rarisma, 2026-04-15. https://news.ycombinator.com/item?id=47784780
- **Who:** teams (~50-person deployment described in #8081 comments), families, small startups.
- **Who solves it:** many third-party plugins appear in the comments (Pinchy, openclaw-governance, Cordum, Hermes Swarm Map). [Inference] Fragmented: every harness grows its own approval layer.

### 7. Memory, shared knowledge, backup
- hermes mempalace (29), backup & version control of agent data (40, #12238), cloud sync (23); paperclip company-level knowledge layer (26, #1858); openclaw Qdrant memory (16), memory flush on reset.
- "A single disk failure wipes months of agent learning with no recovery path. Equally, there is no way to see how an agent's skills and memory evolved — or roll back a single skill that started behaving incorrectly." — hermes #12238, 2026-04-18, 40 reactions. https://github.com/NousResearch/hermes-agent/issues/12238
- "Each agent operates in isolation without shared understanding of the company's mission" — paperclip #1858, 2026-03-26, 26 reactions. https://github.com/paperclipai/paperclip/issues/1858
- Hermes shipped `hermes backup` (per comment on #12238). Rollback of a single bad skill is still the open part.

### 8. Maintainers flooded with AI-made PRs ("AI slop")
- **Pain:** the agent projects themselves are built by agents; maintainers cannot review the flood; users lose trust in the code.
- **Evidence:** odysseus "WTF is going on here?" (66 reactions, 54 comments), "Don't abuse Github issues" (62), "AI slop in general" (39), PR-triage proposal (11); openclaw Stabilisation Mode (75); paperclip "This thing is another AI DISASTER" (13).
- **Quotes:**
  - "800 merged pull requests of LLM slop, yet barely any objective improvement. 350 still pending." — odysseus #1866, 2026-06-03, 66 reactions. https://github.com/odysseus-dev/odysseus/issues/1866
  - "If someone lets his AI post stuff that he doesn't understand, what value is he to the project?" — same issue.
  - "PRs arrive faster than one person can read them ... collapses **408 PRs → ~279 real decisions** ... **Sign every verdict** with a local key, so the whole triage is replayable and tamper-evident." — odysseus #1711 (Charon proposal), 2026-06-03, 11 reactions. https://github.com/odysseus-dev/odysseus/issues/1711
  - "The repository says 5k+ issues and 5k+ prs. There's no way any human being knows what's going on in the codebase." — HN @unbolted3032, 2026-04-16. https://news.ycombinator.com/item?id=47787433
- **Who:** OSS maintainers and contributors; users judging trustworthiness.
- **Who solves it:** OpenClaw uses bots ("clawsweeper", "Closing this as implemented after Codex review"); hermes uses AI triage comments ("This was generated by AI during triage."); Charon proposal. [Inference] AI is reviewing AI inside one project, with no shared standard.

### 9. "It says it did it, but it didn't" (fake tool calls, false done)
- **Pain:** the agent reports work that never happened; runs get marked "succeeded" while still incomplete; "done" status loops.
- **How often:** low reactions per issue, but frequent. GitHub phrase counts: "fake tool" — openclaw 91, hermes 78; "says it did" — openclaw 28, hermes 12; "marked done" — ~34 each in openclaw, hermes, paperclip.
- **Quotes:**
  - "It just says it did and it doesn't." — openclaw #57898, 2026-03-30, 11 reactions. https://github.com/openclaw/openclaw/issues/57898
  - "I even setup an audit process to verify authenticity through logging and submission files--it admitted that what it said it did wasn't actually done. ... It's fictional at this point." — comment on hermes #8993, 2026-04-24, 6 reactions. https://github.com/NousResearch/hermes-agent/issues/8993
  - "In roughly 1 out of 2 chats ... AI models fake tool calls instead of invoking them" and a commenter: "**Failures**: 27 VS **Successes**: 7" — odysseus #4939, 2026-06-27, 6 reactions. https://github.com/odysseus-dev/odysseus/issues/4939
  - "a heartbeat run can be marked `succeeded` even though the parent OpenCode session explicitly says it is still waiting on background subagent results." — paperclip #889, 2026-03-14. https://github.com/paperclipai/paperclip/issues/889
  - "chat logs tell what the openclaw said it did, but not what it actually did." — Show HN: CameraClaw, 2026-03-17. https://news.ycombinator.com/item?id=47418007
  - "the user approves the task, but nothing happens, and child/continuation work can remain stuck even though the parent appears complete" — hermes #31717, 2026-05-24. https://github.com/NousResearch/hermes-agent/issues/31717
- **Who:** everyone who runs agents unattended; worse with cheap/local models.
- **Workarounds:** reading session JSONL logs, screen recording sandboxes (CameraClaw), self-built audit logs, stronger models.
- **Who solves it:** OpenHands ships a trained critic model (#8963, 11 reactions) — but a user asked how to opt out of it. Hermes closed "Acceptance Criteria & Independent Judge" (#356) with "Achieved this with /goal". [Inference] Fixes are per-harness and run by the same vendor.

### 10. Remote brain, local hands
- hermes "Support remote Hermes agent with local tool execution" (43, #18715), desktop client-only (74 + 27 + 22), multi-backend terminal (17).
- "Running a separate local Hermes instance loses the remote instance's accumulated skills and memory. Running everything on the remote Hermes host loses access to the local workspace" — hermes #18715, 2026-05-02. https://github.com/NousResearch/hermes-agent/issues/18715
- [Inference] People want the agent's identity/memory separate from where it executes. Not core to verification, but a design hint.

### 11. Silent or undisclosed behavior
- "web_search/web_extract silently routes to Parallel.ai without user opt-in" — hermes #45058, 2026-06-12, 29 reactions; top comment "This is a huge concern." (17 reactions). https://github.com/NousResearch/hermes-agent/issues/45058
- "Paperclip silently installs itself into a **hidden directory**" — paperclip #3154, 2026-04-08. https://github.com/paperclipai/paperclip/issues/3154
- Paperclip hard-codes "`Co-Authored-By: Paperclip <noreply@paperclip.ing>`" in every agent commit — #4677, 9 reactions. https://github.com/paperclipai/paperclip/issues/4677
- [Inference] Users want a neutral record of what their agent stack actually did and sent, not the vendor's word.

---

## Evidence FOR and AGAINST the hypotheses

### H1 — nobody independently verifies AI agents' work across vendors
**FOR**
- "Done" is unreliable in all four active open agents (pain #9). Users verify by hand: reading JSONL, building audit logs, screen recording.
- The idea of uncorrelated verification is explicitly asked: "implementation and verification must be uncorrelated — the agent that does the work should not be its sole verifier." — hermes #356, 2026-03-04 (3 reactions). https://github.com/NousResearch/hermes-agent/issues/356
- People run mixed fleets: "I run a small fleet of agents: Claude Code, Codex, Hermes, a couple of custom ones." (Preloop, HN). Paperclip's top asks are adapters for many vendors' CLIs. So a cross-vendor layer has a real home.
- Maintainers need cross-contributor verification of AI PRs (pain #8).

**AGAINST**
- Direct asks for "independent verification" get few reactions (hermes #356 had 3; OpenHands critic 11). The top-reacted asks are providers, stability, UX.
- Vendors are closing the gap inside their own tool: Hermes `/goal`, OpenHands critic model, OpenClaw "Codex review" bot. Users may accept "good enough" in-vendor checks.
- Much of the fake-done pain is blamed on weak/local models; users' fix is "use a better model" or a router (HN 49159530).
- [Inference] H1 is real as a pain but not yet named by users as a product category. Users describe symptoms ("it says it did"), not "cross-vendor verifier".

### H2 — teams need a portable "proof of done" / audit record per AI change
**FOR**
- Charon proposal: "Sign every verdict with a local key, so the whole triage is replayable and tamper-evident." (odysseus #1711, 11 reactions).
- Audit trail is mentioned inside RBAC asks (openclaw #8081: "Audit logging for admin actions"), and Pinchy ships an "HMAC-signed audit trail".
- "Chat logs tell what the openclaw said it did, but not what it actually did." (CameraClaw) — people want evidence, not narration.
- Paperclip "done" status bugs show the record of completion itself is not trustworthy (#3980, #889).
- A 2026-03-02 HN comment in the thread "If AI writes code, should the session be part of the commit?" argues the useful part to keep is "what approaches were tried and rejected". https://news.ycombinator.com/item?id=47215896

**AGAINST**
- No top-40 issue in any repo asks for a "proof of done" artifact. Reaction counts for audit-specific asks are small.
- Some users push back on agent attribution in commits (paperclip #4677: opt-out of Co-Authored-By). A proof record must be opt-in and not pollute git history.

### H3 — open, self-hosted, vendor-neutral alternatives to Factory/Devin/Linear agents are missing
**FOR**
- Strongest signal in this theme: vendor bans (Anthropic, Google) and "pay twice" complaints are the top HN stories of 2026 about these tools. Users are angry at lock-in.
- Self-hosting and local models are top asks (paperclip Ollama 79; OpenHands Gitea/Forgejo 46, podman 24; odysseus SSO).

**AGAINST**
- The open, self-hosted space is crowded, not empty: OpenClaw, Hermes, Paperclip, Odysseus, OpenHands each have 90k–390k stars, plus NanoClaw, ZeroClaw, nanobot, OpenTalon, Pinchy, Preloop.
- [Inference] The gap is not "an open alternative exists"; it is "a stable, trustworthy one". Users switch harness every few months (OpenClaw → Hermes) because of breakage.

### H4 — agents should learn only from verified outcomes
**FOR**
- Hermes users want rollback of "a single skill that started behaving incorrectly" (#12238, 40 reactions) — evidence that self-learned skills go bad.
- "Hermes Agent can now /learn from anything" (HN, 6 points) and Microsoft SkillOpt request (14) show self-learning is a selling point, so bad learning is a coming risk.
- A routing tool pitch on hermes #157: "Every run captures telemetry and outcome success, learns which model+tool path actually works" — outcome-based learning is already being sold.

**AGAINST**
- No user issue directly asks "only learn from verified outcomes". Evidence is indirect. Weak.

---

## New open spaces suggested by the evidence (not in H1–H4)

1. **Upgrade safety gate for agent stacks** — strength: **strong**. Why: 994 "regression" titles in openclaw, 247 in hermes; a top r/openclaw comment asks for "operator-grade upgrade checklists and compatibility gates". [Inference] A neutral tool that replays a user's own saved tasks against a new version (or new model) and says "safe to upgrade / these 3 workflows broke" would hit the #2 pain and reuse the verification engine.
2. **Cross-harness cost + activity ledger ("what did my agents do last night and what did it cost")** — strength: **strong**. Why: many Reddit/HN posts about surprise bills; Paperclip budgets broken for subscription runs; Preloop and Grafana integrations appear in mid-2026, meaning demand is real but solutions are new. Fits H2 (one record holds both evidence and cost).
3. **Human task queue / "agent is blocked, here is what I need from you"** — strength: **medium**. Why: paperclip #923 (12), #188 (15), #762; users of fleets complain about being the "messenger pigeon". A neutral inbox across agents for approvals and unblocks.
4. **Maintainer-side triage of AI PRs (dedupe, verify, sign)** — strength: **medium**. Why: odysseus #1866 (66), openclaw #5799 (75); maintainers of the agent projects themselves are the first victims. Good early-adopter wedge for a verifier.
5. **Honest-failure / "did the tool really run" detector** — strength: **medium**. Why: fake tool calls in ~half of chats on local models (odysseus #4939); hermes #45058 complaint "Now they route silently instead of failing honestly." A cheap check comparing claimed actions vs. real tool events.
6. **Agent state version control (skills/memory diff + rollback)** — strength: **medium-weak**. Why: hermes #12238 (40); backup is shipped, rollback of single skills is not.

---

## Willingness-to-pay signals
- "It was providing me tons of value - easily $2-3k/mo. I hate it but I caved, decided I would pay the extra usage charges, and prepaid $1k" — HN @sneak, 2026-04-15. https://news.ycombinator.com/item?id=47785371
- "Claude max 200. ... It's a LOT of tokens, like $2500/mo worth of api" — u/No_Mango7658, r/openclaw, 2026-03-17. https://www.reddit.com/r/openclaw/comments/1rvsbyr/how_much_are_you_spending_on_tokens_with_openclaw/
- "Spending about $2 a day and it feels like having the best employee I've ever had." — u/qaz135wsx, same thread.
- "I have a tool that sits on top of Open Claw that allows me to track the cost(tokens) for each task." — u/ryanb082, same thread (people build/buy cost tooling).
- Paid setup services appear inside issues: "Hermes Agent Setup Service ... Price: Only 2500 BDT (local) / 25 USD" — comment on hermes #8993. [Inference] People pay to get these stacks working.
- A ~50-person engineering team built its own multi-user layer for OpenClaw (comment on openclaw #8081) — teams invest engineering time, a proxy for budget.
- Users pay twice today (subscription + API) to keep agents running (hermes #25267).
- Weak/negative: "If it's free, you are the product." (paperclip #3154 comment) — suspicion of hidden monetization; trust must be earned.

---

## What surprised me
- The loudest anger is not about agent quality but about the **agent projects' own code** being AI-built and unreviewable (odysseus #1866 with 66 reactions; OpenClaw freezing features). The verification problem hits maintainers first.
- "It says it did it" shows up with **many mentions but few reactions** — people report it as a bug, not as a feature request. [Inference] Users do not yet know a product category could fix it; demand is latent, not named.
- Vendors already respond in-house: Hermes closed the "independent judge" request saying `/goal` covers it; OpenHands built a critic model; OpenClaw closes issues "after Codex review". In-vendor self-checking is becoming normal — which strengthens H1's "cross-vendor/uncorrelated" angle but weakens "nobody verifies".
- Platform risk dominates: the biggest HN threads of 2026 for these tools are about Anthropic/Google cutting off subscriptions, not features.
- Switching cost is low: several HN users moved OpenClaw → Hermes within weeks. [Inference] A neutral layer that survives harness switching (records, checks, costs) is more durable than another harness.
- OpenHands has little 2026 community activity on GitHub issues compared to the "personal agent" projects.
