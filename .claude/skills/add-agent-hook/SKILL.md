---
name: add-agent-hook
description: Use when making Countercheck check another AI agent automatically when it tries to finish (Codex, Cursor, Copilot, Goose, Kiro, …). Adds one file in `apps/countercheck/src/agents/` (where the agent keeps hooks, what to add, and its stop hook if its format differs), so `countercheck hook install --agent <name>` sets it up and the shared verify + decide logic judges it.
---

# Add an agent hook

A hook runs when an agent tries to stop, verifies its work, and either lets it finish or sends it back with the
failing check. Existing: `hermes-pre-verify`, `claude-stop` (alias `stop`; also used by Snowflake Cortex Code) and
`session-start`, in `apps/countercheck/src/hook.rs`. Setup is `countercheck hook install --agent <name>`
(`apps/countercheck/src/agents/`): each agent is one `Target` there. A new agent only translates: where its hooks
file lives, its input JSON in, its answer format out. The decision itself is shared and must not be copied.

## Before writing code

1. Read the agent's **official**, current hook docs for a "before stop / on finish" event: which file the hooks live
   in (project and user), that file's shape, the JSON on stdin (working folder, session id, last message or
   transcript path), how a hook says "keep going, here's why", how it shows a message to the user, its loop cap, and
   any trust/approval step. Put the links and the agent version in the PR. If the agent has no blocking finish hook,
   document `countercheck verify --claim` usage in `docs/hooks/` instead; don't invent one.
2. Check whether its input AND answer match `claude-stop` exactly (Cortex Code does). Then the agent needs only a
   `Target` whose entries run `claude-stop`, and docs: no new stop hook.

## Procedure

1. `apps/countercheck/src/agents/<agent>.rs`:
   - `pub fn target() -> Target` (model: `claude.rs`): `agent` (kebab-case; also the receipt label), `label`,
     `project_file`, `user_file`, `top` (keys a new file needs, e.g. `"version": 1`), `entries` (JSON text appended at
     each path, built with `super::command(bin, "<sub>")`, keys in the order the agent's docs show), each entry's
     `needles` (the Countercheck commands that count as "already there", aliases included), `also_reads` (other files
     the agent reads hooks from, so install warns about a double run), `after` (the one step the user must take:
     approve, trust, reload), `docs`.
   - If the stop input or answer differs: `pub fn stop() -> Result<(), String>` that reads the payload
     (`hook::read_payload`), builds a `hook::HookInput` (cwd, session, the agent's final message, agent label, model),
     calls `hook::check`, applies the nudge limit (`hook::take_nudge`), logs with `hook::log_call`, and prints the
     agent's own "keep going" answer for `Decision::Continue(msg)` and its user-visible message (`hook::stop_message`)
     otherwise. Make the `hook.rs` items you need `pub(crate)`; never copy them.
   - Wire the agent's **session-start** event to `countercheck hook session-start` (it reads `session_id` and `cwd`;
     if the agent names them differently, add a mapping there). Without it the checks are locked to HEAD, and an
     agent that commits its own test edits moves the base.
2. `agents/mod.rs`: `mod <agent>;` and one line in `all()`. `main.rs`: a `HookCmd` variant `<agent>-stop` routed to
   `agents::<agent>::stop()`, only if step 1 added one.
3. **Fail open:** any error in a stop hook (bad JSON, not a git repo, no checks) prints to stderr and lets the agent
   finish; a broken referee must never trap an agent. `install`/`uninstall`/`status` errors are real errors (exit 1).
4. Agent name on receipts: add the agent's env marker to `verify::agent_from_env` if it has one.
5. Docs: `docs/hooks/<agent>.md` (the install command, what lands in the file, the trust step, what the agent sees,
   the env vars `COUNTERCHECK_MAX_NUDGES`, `COUNTERCHECK_HOOK_LOG`, `COUNTERCHECK_BIN`), a row in the README "Where
   it fits" table and the `where-it-fits.svg` left column, a line under `## Unreleased` in CHANGELOG.

## Tests: `apps/countercheck/tests/hook_<agent>.rs` (its own file, so parallel work doesn't collide)

- Payload fixtures copied from the agent's docs (or recorded from a real run; say which) in
  `apps/countercheck/tests/fixtures/<agent>/`.
- Stop hook: verified work → may finish, with the user message; failed + success claim → the agent's exact "keep
  going" format with the failing check's name; failed + honest blocker → may finish; nudge limit reached → may finish
  with the ⚠ line; garbage stdin, missing cwd, a folder that isn't a git repo → exit 0 (fail open).
- Install (`apps/countercheck/tests/install.rs` shows how): `hook install --agent <agent> --yes` writes exactly the
  documented shape into an empty project and into one with other hooks (kept as written); a second install adds
  nothing; `hook uninstall` takes out only ours; `hook status` shows it.
- If the agent's hooks file isn't watched by the tamper check yet (`countercheck_core::hook_settings::FILES`), say so in
  the PR: adding it there is a separate, reviewed change.

## Verification

Run the gate from the `write-tests` skill, then try it for real once if you can: install the hook in the agent, give
it a task whose visible test it's tempted to edit, and confirm it gets sent back. Note the agent version. If you
can't run the agent, say so plainly and tag every claim observed / docs / inferred.

## Pitfalls

- Never decide in the hook with your own rules: `hook::decide` and `claim::reports_blocker` are shared so every
  agent gets the same referee.
- Some agents send the last message, others a transcript path (`last_assistant_text` reads Claude-style JSONL).
- Hooks have timeouts (Claude Code / Cortex Code: 60 s by default) and loop caps: the docs page says how to set the
  timeout above the checks' run time, and `COUNTERCHECK_MAX_NUDGES` (3) must stay below the agent's own cap.
- Settings files are the user's: `hook install` edits them through `agents::json`, which keeps their key order and
  formatting. Never rewrite them with `serde_json` (it sorts keys).
