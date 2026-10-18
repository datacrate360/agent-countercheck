# T5 Agents beyond coding (non-developers, mixed teams, Agile/PM): market research

Retrieved: 2026-10-18. Analyst theme: T5. Paragraphs marked [Inference] are my own reasoning. Everything else is copied from pages I actually retrieved.

## Sources covered

| Platform | What I read | Date range | Notes |
|---|---|---|---|
| Reddit | 31 threads attempted, 28 usable (r/AI_Agents 11, r/ProductManagement 5, r/smallbusiness 3, r/projectmanagement 2, r/agile 2, r/scrum 1, r/SaaS 3, r/Entrepreneur 1, r/Solopreneur 1, r/aiagents 1) | 2026-01-31 → 2026-10 | Reddit blocks direct access from this machine. `.json` gave HTTP 403, old.reddit gave a 302, and the browser showed "blocked by network security". All Reddit text came through `web_extract`, which often returns only the post plus the top ~20 comments, and sometimes cuts the text short. Some extracts had no absolute date, only "Nmo ago" relative to 2026-10-18. Those are shown as "~Mon 2026". 1 thread failed (r/projectmanagement "Adopting AI", SSL timeout). 3 extracts were almost empty (r/SaaS OpenClaw trust, r/projectmanagement MCPs, r/PM "Has Claude/Codex helped"). I did not quote from those. |
| Hacker News | ~25 Algolia story searches (2026+ only), 13 full comment trees read (≈2,000 comments), 6 comment-level searches | 2026-01-12 → 2026-10-16 | Worked well. Points and comment counts below are from Algolia. |
| GitHub | paperclipai/paperclip (103,934 stars). Top 20 issues by reactions read, 2 issue bodies read | 2026-03 → 2026-05 | `gh search issues` returned errors (likely a rate limit), so I used `gh issue list` instead. |

Totals: ~28 Reddit threads, 13 HN threads plus ~20 standalone HN comments, and 20 GitHub issues.

---

## Top pain points (ranked by evidence)

### P1. Agent says "done" but the real business system says otherwise (CRM, refunds, posts, files)
- **How often:** Very frequent. There were 6 separate r/AI_Agents/r/aiagents threads in Jul–Oct 2026 just on this, plus HN comments and the webmaster case study. This was the most repeated concrete pain in my sample.
- **Quotes:**
  - "A clean run just means the thing stopped running. that's it. doesn't mean the work happened." (r/AI_Agents, 2026-07-26, 4 pts / 33 comments) https://www.reddit.com/r/AI_Agents/comments/1v7bhm2/your_agent_says_done_you_go_check_and_nothing/
  - "If you're doing codegen you got it easy tbh, rerun the test or diff the file and you know. but the stuff that touches real business systems is the problem." (same thread)
  - "never let the model grade its own homework." (r/AI_Agents, "our agent kept saying 'done, CRM updated' when it wasn't", date not visible in extract) https://www.reddit.com/r/AI_Agents/comments/1wtvtcf/our_agent_kept_saying_done_crm_updated_when_it/
  - "it even reported a hash for the finished file that neither machine ever produced." / "mostly-true is what success normally looks like." (r/AI_Agents, 130-agent setup, date not visible) https://www.reddit.com/r/AI_Agents/comments/1w4wy06/caught_one_of_my_agents_reporting_work_it_never/
  - "Perplexity told me it was finished constantly, with only 10 records of 100 done..." (r/aiagents, 2026-08-10, 12 pts) https://www.reddit.com/r/aiagents/comments/1vk75xp/has_an_ai_agent_ever_told_you_done_when_it/
- **Who has it:** Solo builders and agencies who ship agents to clients for ops, sales, support, and marketing. Also founders running "agent companies".
- **Workaround today:** Hand-built "read-back" checks against the system of record, receipts per tool call, and "the summary is never proof" rules. One commenter: "we ended up building a deterministic verification layer that checks the crm status after the agent claims success." (CRM thread above). A webmaster builder's rule: "Standing rule now: when the agent says it's done, open the file." https://www.reddit.com/r/AI_Agents/comments/1wqh0s0/i_set_up_an_ai_webmaster_my_nontechnical_client/
- **Who solves it today:** Nobody, according to these users. Several posters are building their own tools ("I'm building something in this area", "trying to figure out whether people actually want a tool for this or just the pattern"). Observability tools are dismissed: "observability is just the trace, which is the agent narrating itself" (1v7bhm2).

### P2. Non-developers can't get general agents (OpenClaw-style "AI employees") working reliably
- **How often:** Very frequent on HN ("Ask HN: Who is using OpenClaw?" 342 pts / 383 comments, 2026-04-15) and in r/AI_Agents, r/smallbusiness, and r/Entrepreneur.
- **Quotes:**
  - "for us non-Coders and non-Software Engineer types, it's a waste of time." (r/AI_Agents, 2026-03-28, 12 pts / 42 comments) https://www.reddit.com/r/AI_Agents/comments/1s5rsl6/i_broke_up_with_my_ai_agent/
  - "They make experienced developers faster, not turn non-developers into developers." (same thread, 4 pts)
  - "I don't feel like a tool that is ready for a non-developer audience." (HN lizardking, 2026-04-15) https://news.ycombinator.com/item?id=47783940
  - "The potential of the tool is massive. The reality is that it is frustrating and unreliable." (HN BeetleB, same thread)
  - "It's not really accurate to paint it as something that learns about your workflow and is ready to go out of the box. It does require some initial set up in my experience. It's technical." (r/smallbusiness, 2026-02-17, 4 pts) https://www.reddit.com/r/smallbusiness/comments/1r6r9fg/has_anyone_actually_used_ai_agents_to_automate/
- **Who:** Small business owners, solo founders, and hobbyists. Even 30-year engineers said setup was "worse than a root canal" (HN carimura, 47783940).
- **Workaround:** Moving back to Claude Code/Codex plus cron, or to Hermes/NanoClaw. Also narrowing to "morning brief" and read-only use: "it only has readonly access to both, as I dont trust it to do stuff for me or on my behalf." (HN rarisma, 47783940).
- **Existing products:** OpenClaw (judged "fragile", "15% broken at every moment", by HN browningstreet), Claude Cowork (easier, but seen as risky), Hermes (one user: "the first one where it didn't feel like I was constantly needing to fix it", HN superfrank, 47783940).

### P3. Narrow, boring, back-office jobs work. Customer-facing and "autonomous employee" jobs fail
- **How often:** Frequent. Consultants and owners across r/smallbusiness, r/Solopreneur, r/Entrepreneur, r/AI_Agents, and r/SaaS all describe the same pattern.
- **Quotes:**
  - "The ones who started with 'write our newsletter' spent three months arguing with AI output and gave up." (r/Solopreneur consultant, 20+ SMB deployments, 2026-03-13) https://www.reddit.com/r/Solopreneur/comments/1rsojbs/ive_helped_20_small_businesses_deploy_ai_agents/
  - "AI sales agents talking directly to customers (there is a loss of trust, especially in B2B/high ticket clients)" (r/Entrepreneur, 2 pts) https://www.reddit.com/r/Entrepreneur/comments/1qvy2fm/automation_with_ai_what_did_you_try_what_worked
  - "The 'fully autonomous employee' demos never survive contact with real work." / "Reversibility matters more than confidence." (r/AI_Agents, 2026-07-07) https://www.reddit.com/r/AI_Agents/comments/1upuzes/whats_your_actual_use_case_for_ai_agents_right/
  - "The businesses that get burned are the ones that over-automate and assume the AI handles exceptions well. It usually doesn't." (r/smallbusiness, date not visible) https://www.reddit.com/r/smallbusiness/comments/1wthadj/what_ive_seen_ai_agents_actually_do_well_and_not/
  - "Your own authentic voice answering a customer query directly is one of the few advantages a small business has over the big guys." (HN cube00, 2026-09-15) https://news.ycombinator.com/item?id=49715611
- **What works per users:** After-hours phone intake, invoice/AP matching, lead routing, appointment reminders, meeting summaries, drafted (not sent) replies, and daily briefs.
- **Who:** SMB owners (<$5M revenue) and automation agencies.

### P4. Trust needs approvals + audit trail + "why did it do that", more than intelligence
- **How often:** Frequent in r/SaaS and r/AI_Agents. Echoed on HN.
- **Quotes:**
  - "in week two of production it confidently issued a refund to someone who wasn't eligible" … "no approval step, no audit trail, just... done." (r/SaaS, dorongal1) https://www.reddit.com/r/SaaS/comments/1s3sa1v/i_think_the_biggest_problem_with_ai_agents_is/
  - "what they can't tolerate is not being able to explain WHY it did something after the fact. Auditability often has to come before autonomy in any real production deployment." (same thread)
  - "The painful 70% is approvals, audit trails, human checkpoints, and damage control." (same thread)
  - "An acceptance of a work result should not be inferred as 'the model said it is done', but be recorded as a human approval." (HN dongkeren, 2026-08-18) https://news.ycombinator.com/item?id=49347839
- **Who:** Teams putting agents into support, sales ops, and CRM.
- **Workaround:** "human checkpoints": the agent drafts, a human approves side effects, and every action is logged with a natural-language reason (dorongal1).

### P5. Non-technical stakeholders need a plain "did it work / what did it cost / what do I do next" view, not traces or chat
- **How often:** Medium. Seen in one rich thread with several builders confirming, the webmaster case, and the cost threads.
- **Quotes:**
  - "they want three things: what ran, what it cost, did it actually work." (r/AI_Agents, date not visible) https://www.reddit.com/r/AI_Agents/comments/1uw7xhj/built_a_statusupdate_layer_for_ai_agents_you_ship/
  - "'No error thrown' and 'task completed' can both be true while the actual output was wrong in a way that matters to the client." (same thread)
  - "Every agent app i've shipped to a client ends up in the hands of sales, support or marketing folks and none of them want a transcript." (same thread)
  - "i've had clients literally not use agents because they couldn't predict the bill." (same thread)
  - "it can charge something like $32 for one task and then not charge anything for the next five or six tasks." (r/AI_Agents, Duckbill user) https://www.reddit.com/r/AI_Agents/comments/1v2sl9f/whats_the_real_cost_of_everyday_ai_agents/
  - "If you only track provider spend, nobody knows what one useful outcome actually costs." (HN reggz, 2026-03-19) https://news.ycombinator.com/item?id=47426577
- **What works:** "the agent writes a row, they get a queue with statuses + an approve button", delivered inside Slack, Teams, or WhatsApp (1uw7xhj).

### P6. PMs are adopting coding agents (Claude Code / Cowork) for non-code work, but they hit setup, context, verification, and security walls
- **How often:** High engagement. "How I use Claude Code as a Product Manager" has 701 pts / 178 comments (2026-04-19). The "Claude Code vs Cowork" thread has 125 pts / 71 comments.
- **Quotes:**
  - "More than half of my working day now happens in a terminal and VS Code. I'm a Product Manager." (r/ProductManagement, 2026-04-19, 701 pts) https://www.reddit.com/r/ProductManagement/comments/1sppgwd/how_i_use_claude_code_as_a_product_manager/
  - "no one teaches how to use claude code for non coding work." (same thread, OP reply)
  - "how are you verifying it's not generating slop, particularly with data analysis?" (same thread)
  - "Staring a CLI interface is daunting for almost all non-technical folks, and Claude Cowork solved a lot of that problem." (r/ProductManagement, 2026-05-24, 8 pts) https://www.reddit.com/r/ProductManagement/comments/1tmf0n1/for_those_who_use_claude_code_for_pm_work_why/
  - "Cowork is not appropriate for our regulated environment because of the security risks and lack of BAA support in the Enterprise agreement." (same thread, 8 pts)
  - "tools like spark, chatPRD, and more are limited and feels more like a chat bot where I can't get pridictable output with on going project context" (r/ProductManagement, ~Jul 2026) https://www.reddit.com/r/ProductManagement/comments/1v0a90z/has_anyone_seen_product_agents
- **Workaround:** Each PM builds a personal "PM OS" with markdown memory, skills, and MCP connectors. One commenter: "Integrations with all appropriate context (Gong, Jira, ADO, GitHub, etc etc etc) costs significant time and maintenance abilities." (1v0a90z).
- **Products:** Claude Code (powerful but needs CLI comfort), Cowork (easier, but distrusted in regulated firms), ChatPRD and Productboard Spark (seen as chatbots).

### P7. Agile/Scrum: AI absorbs the admin, but the human coordination and accountability are not solved, and multi-person + multi-agent coordination is new pain
- **How often:** Medium (r/agile, r/scrum, HN).
- **Quotes:**
  - "gone are the days where companies are willing to pay for someone who just helps meetings go smoothly or come up with fun retro games." (r/agile, date not visible) https://www.reddit.com/r/agile/comments/1vy4emu/whats_the_impact_on_the_scrum_master_role_with_ai
  - "telling Rovo to 'move tickets from epic A to B' is much better than having to do it by hand." (same thread)
  - "It looks impressivr until you realize teams stop thinking and also avoid accountability." (r/scrum OP, ~Jul 2026) https://www.reddit.com/r/scrum/comments/1o7p6s0/is_aiassisted_scrum_even_compatible_with_agile
  - "Forecasting should not be done using a black box like AI. How would you know when it got things wrong?" (same thread)
  - "Single-player AI development is solved. Multiple people with their own agents building the same product together is not." (r/agile, ~Apr 2026) https://www.reddit.com/r/agile/comments/1spyq1w/pulse_a_postscrum_manifesto_for_ainative_teams
  - "I don't understand why coding agents are playing dress-up with these human engineering rituals." (HN ozozozd, 2026-05-04) https://news.ycombinator.com/item?id=48018091
  - "So you will still need a human to pester the dev." (HN ItsYan, 2026-03-13) https://news.ycombinator.com/item?id=47363065
- **Products:** Atlassian Rovo (praised for ticket chores), Linear agents, Paca (open-source "Jira alternative for human-AI collaboration", 174 pts on HN), Backlog.md, and hand-rolled repo backlogs.

### P8. Agents need maintenance like a new hire. "Set and forget" fails quietly
- **Quotes:**
  - "AI agents are not 'set it and forget it.' They're closer to a new employee in their first 90 days." (r/Solopreneur, 2026-03-13) https://www.reddit.com/r/Solopreneur/comments/1rsojbs/ive_helped_20_small_businesses_deploy_ai_agents/
  - "If there's no human catching those cases during that window, they silently produce bad output. The agent quietly fails." (same)
  - "I have vibe coded some tools for my team and am fearful of what will happen when they break. I'm not an engineer." (r/ProductManagement, ~Oct 2026) https://www.reddit.com/r/ProductManagement/comments/1x2c2ud/a_new_direction_that_i_think_will_take_off/
  - "the agent was working perfectly. Their ticket tagging at the CRM level was a mess" (r/AI_Agents agency, ~May 2026) https://www.reddit.com/r/AI_Agents/comments/1to1uoo/ive_built_50_ai_automations_for_clients_heres_why/

### P9. Desktop/general agents for non-devs are scary: irreversible actions and data leaks
- "unlike code which can be reverted - most file system and application operations cannot." (HN jryio, Cowork launch, 2026-01-12, story 1298 pts / 565 comments) https://news.ycombinator.com/item?id=46593022
- "I can foresee non-technical users giving access to unfortunate folders and getting into a lot of trouble." (HN jfletch321, same)
- "zero chance im trusting any cloud or third party SaaS if this doesn't run fully local its a no go for enterprise let alone ordinary users" (HN zuzululu, Screenpipe, 2026-07-23) https://news.ycombinator.com/item?id=49024620

---

## Evidence FOR / AGAINST the hypotheses

### H1. Nobody independently verifies agents' work across vendors
- **FOR (strong, outside coding):** The "done but not done" cluster (P1) shows people building their own verifiers because nothing exists. Key line: "the model shouldn't be allowed to grade its own homework" (1wtvtcf). On HN: "reviews using the same model rarely caught its own mistakes—only a different model or execution receipts (rc·sha256·stderr) caught them." (NimadFlow, 2026-09-10) https://news.ycombinator.com/item?id=49630606. The OtoDock author admitted: "OtoDock does not verify a report by itself, but you can set it up in your installation."
- **AGAINST / nuance:** Users say the fix is mostly *checking the system of record*, not a second model. "this failure needed something simpler: don't ask another agent what happened, go check the artifact itself." (1w4wy06). A second-bot reviewer also missed visual errors in the webmaster case: "a second bot reading the diff would miss a visual mismatch unless it actually renders the flyer" (1wqh0s0). [Inference] "Cross-vendor" matters less to non-devs than "independent of the model's own claim". Vendor neutrality is a secondary benefit, not the pain itself.

### H2. A portable "proof of done" / audit record per AI-made change
- **FOR (strong):** People ask for exactly this: "making every step return proof instead of a generic success message, stuff like IDs, counts, screenshots, or the exact changed state" (r/AI_Agents) https://www.reddit.com/r/AI_Agents/comments/1ssz1qu/how_are_you_forcing_agents_to_prove_a_task/. Also "'Done' should probably mean 'verified,' not just 'I stopped working.'" (1vk75xp), and "Auditability often has to come before autonomy" (1s3sa1v). In the webmaster thread, users designed "approval bound to preview hash + commit" receipts. The sharpest version of the need comes from non-dev use: "There's no cheap rerun for any of that" (1v7bhm2).
- **AGAINST:** No one in my sample said they would *pay* for a portable proof format specifically. Users want the result inside their existing surface (Slack, Teams, a queue with an approve button), not a new artifact (1uw7xhj). [Inference] The record must be human-readable for non-tech owners ("did it work / cost / next action"), not a developer trace.

### H3. Open, self-hosted, vendor-neutral alternatives are missing
- **FOR (medium):** "I've been trying keeping an eye on open source issue trackers/project managament tools I can self-host -- with good cli/mcp capabilities. So quite happy to see this as I feel there isn't a lot!" (HN sambucini, Paca, 2026-06-13) https://news.ycombinator.com/item?id=48515385. Vendor lock-in worry: "instead of the memory being locked away with a specific vendor, it's stored in version control" (HN lexandstuff, 47783940), and "My company is always shifting between Claude and OpenAI enterprise contracts." (1sppgwd). On Paperclip, the top issue is local LLM via Ollama (79 reactions) https://github.com/paperclipai/paperclip/issues/187.
- **AGAINST (strong):** The space is crowded. Paperclip alone has 103,934 stars, and HN shows OtoDock, Paca, OpenWork (231 pts), Pizza Bot, Lemma, Mercury, and more. Many builders say people just roll their own: "it's gotten so cheap to just build and evolve your own as the models and harnesses change that picking up someone else's stops making much sense" (HN Tsarp, 48515385). [Inference] "Open + self-hosted" is table stakes, not a moat.

### H4. Agents should learn only from verified outcomes
- **FOR (weak–medium):** The 701-pt PM post uses per-skill `learnings.md` files, and its author says the fix for slop was first anchoring to a "verified Metabase dashboard" (1sppgwd). Users want corrections persisted: "every one would have been gone on a rebuild. They now live in one versioned prompt file" (1wqh0s0). Screenpipe commenters ask for provenance: "Each derived claim should point back to the exact scr[een]…" and "Treating every captured event as memory will make retrieval noisy, while replacing the events with summaries will make the result hard to audit." (HN makeyouragent, 49024620).
- **AGAINST:** Nobody in T5 sources explicitly asked for "learn only from verified outcomes". The felt pain is upstream: the agent claiming success at all. [Inference] H4 makes sense as a feature of H1/H2 (verified receipts feed memory), not as a separate selling point to non-devs.

---

## NEW open spaces (not in H1–H4)

1. **Business-system reconciliation for agent actions ("did reality match the claim?") — strength: STRONG.** This is verification for *non-code* side effects: CRM, billing/refunds, email sent, social post live, ticket moved. Code has tests, but these systems have no equivalent. Users describe the design themselves: read-back against the system of record, identity check ("verify the CRM record belongs to the originating lead"), "unknown" state on timeouts, idempotency keys, and delayed re-checks (a post later removed by a spam filter). Sources: 1wtvtcf, 1v7bhm2, 1ssz1qu, 1w4wy06. [Inference] This is H1/H2 moved out of the code-review frame into ops. It fits non-dev users better than PR review.
2. **Owner-facing "outcome inbox" for non-technical people — strength: STRONG/MEDIUM.** One queue with plain statuses (done / partial / needs you / failed), cost per task, and an approve button, living in Slack, Teams, WhatsApp, or email. Evidence: 1uw7xhj, the webmaster thread (the client "thought Ivan was waiting for my okay"), and Paperclip #923, "The human ('The Board') has no clear queue of things they need to act on." (12 reactions) https://github.com/paperclipai/paperclip/issues/923
3. **Predictable cost per completed outcome — strength: MEDIUM.** "Credit systems being opaque isn't really an accident" (1v2sl9f). Related: Uber capping AI spend (HN 64 pts, 2026-06-02), Paperclip agents "would go over daily budgets all too often" (49630606), and Paperclip #339 cost estimation (16 reactions).
4. **Graduated autonomy / reversibility lanes — strength: MEDIUM.** Users invent "lanes": content = preview + approve, small code = test + second reviewer, money/PII = human only (1wqh0s0). They also cite "graduated access" (1s3sa1v) and "Reversibility matters more than confidence" (1upuzes). [Inference] A policy layer that maps action types to required evidence and approval is a product, not just a pattern.
5. **Multi-human + multi-agent coordination on one product (post-Scrum) — strength: MEDIUM/WEAK.** "Multiple people with their own agents building the same product together is not [solved]" (PULSE), and "three implementations of the same thing". On HN, a harness-agnostic pool "so multiple agents from different harnesses could all work on the same project at once without losing drift" (mattrichmo, 2026-08-10) https://news.ycombinator.com/item?id=49249775. Evidence comes mostly from builders, not from many sufferers.
6. **"PM OS" starter kit: shared, governed context for non-dev roles — strength: MEDIUM.** PMs rebuild memory, skills, and connectors by hand. They lack teaching ("no one teaches how to use claude code for non coding work") and struggle with least privilege in companies. A team-shareable, permissioned version (Cowork for business users, Code for data pipelines) was suggested by medson61 (1tmf0n1).
7. **Maintenance/ownership of vibe-built internal tools — strength: WEAK/MEDIUM.** "who owns it after launch, and what happens when that person leaves or changes teams?" (1x2c2ud). This is a non-dev fear of breakage with no engineer around.

---

## Willingness-to-pay signals
- "I pay for a few right now… For marketing, I use BrandoraAI because I was spending like 8-10 hours a week on social posts and SEO contents and ad copy" / "The reason I pay instead of building something myself is because I already went down that road with a bunch of langchain scripts and it was a nightmare to maintain." (r/AI_Agents, 2026-07-16, 4 pts) https://www.reddit.com/r/AI_Agents/comments/1utrjsp/what_ai_agents_are_you_actually_paying_for_and_why/
- "nobody pays for a general 'do anything' agent. They pay for one boring, recurring job done without babysitting" (same thread, 3 pts, vendor-affiliated commenter)
- "I make decisions on implementation and payment for our Enterprise use of Atlassian today (mid 6 figures per annum of spend) - and I'm definitely shopping for alternatives!" (HN jamesvnz, 2026-06-13) https://news.ycombinator.com/item?id=48515385
- "It was providing me tons of value - easily $2-3k/mo. I hate it but I caved, decided I would pay the extra usage charges, and prepaid $1k" (HN sneak, OpenClaw, 2026-04-15) https://news.ycombinator.com/item?id=47783940
- Negative signal: "I spent about 40-50 bucks in one week just working through kinks and having it fix itself" (HN bigpapikite, same thread). Also Duckbill's "$32 for one task" confusion (1v2sl9f).
- AI receptionist pricing threads exist (r/AiForSmallBusiness; snippet only, not quoted, because I did not extract the page).
- [Inference] People pay for (a) one recurring narrow job and (b) escaping maintenance of DIY glue. They resist opaque credits. Nobody in my sample yet pays for verification as a standalone product. The builders asking "do people want a tool or just the pattern" are an honest sign that this is unproven.

---

## What surprised me
1. **The verification pain is sharper outside coding than inside it.** Non-dev users say coders "got it easy" because tests and diffs exist, while refunds, CRM writes, and posts have "no cheap rerun" (1v7bhm2). [Inference] This flips the usual framing of H1/H2, which starts from PR review.
2. **PMs are becoming terminal users.** A 701-point post from a PM living in VS Code. The same crowd says "Cowork is weird" and prefers the CLI. Coding agents are the de facto general agent for knowledge workers.
3. **The "AI employee" story is collapsing into "narrow job + human approval".** Even agent-company builders now say the backbone should be deterministic: "the most critical part is agents NOT doing much and relying on 'boring' deterministic backbones" (HN madamelic, 49630606).
4. **Paperclip at ~104k stars, and its top requests are human-in-the-loop features:** "Ability to replace an agent with a human" (#188, 15 reactions) and human-assigned blocker issues (#923). Users want *mixed* human+agent orgs, not pure agent orgs.
5. **Strong cynicism about astroturfing.** "It's always a brand new account posting this shit…" (r/smallbusiness, 2 pts, 1r6r9fg), and many replies in agent threads were vendor plugs. [Inference] Reddit agent subs are noisy. The signals above favour threads where several independent users describe the same failure.
6. **Scrum Master role is described as already gone.** AI (Rovo) is credited with absorbing the leftover ticket admin, not with replacing coaching. The agile pain is now coordination between humans each running agents, not the ceremonies themselves.
