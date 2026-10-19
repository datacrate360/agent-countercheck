---
name: add-agent-hook
description: Use when making Countercheck check another AI agent automatically when it tries to finish (Codex, Cursor, OpenCode, Aider, …). Adds a `countercheck hook <agent>` subcommand that reuses the shared verify + decide logic.
---

# Add an agent hook

A hook runs when an agent tries to stop, verifies its work, and either lets it finish or sends it back with the
failing check. Two exist: `hermes-pre-verify` and `claude-stop` (also used by Snowflake Cortex Code), both in
`apps/countercheck/src/hook.rs`. A new hook only translates: the agent's input JSON in, the agent's answer format out.
The decision itself is shared and must not be copied.

## Before writing code

1. Find the agent's **official** docs for a "before stop / on finish" hook: what JSON it sends on stdin (working
   folder, session id, last message or transcript path), and how a hook tells it "keep going, here's why". Put the
   links in the PR. If the agent has no such hook, document `countercheck verify --claim` usage in `docs/hooks/`
   instead; don't invent one.
2. Check whether its format matches an existing hook (Cortex Code reuses `claude-stop`). If so, only add docs.

## Procedure

1. `hook.rs`: add `pub fn <agent>_stop() -> Result<(), String>` that reads the payload (`read_payload`), builds a
   `HookInput` (cwd, session, the agent's final message), calls `check(input)`, and maps the `Decision`:
   `Silent` → let it finish; `HonestBlocker` → let it finish (log it); `Continue(msg)` → the agent's "keep going" answer
   with `msg`. Respect the nudge limit (`take_nudge`) and log with `log_call`.
2. `main.rs`: add a `HookCmd` variant (kebab-case name) and route it.
   Also wire the agent's **session-start** event to `countercheck hook session-start` (it only needs `session_id`
   and `cwd` in the JSON; if the agent names them differently, add a mapping there). Without it the checks are locked
   to HEAD, and an agent that commits its own test edits moves the base.
3. **Fail open:** any error (bad JSON, not a git repo, no checks) prints to stderr and lets the agent finish. A broken
   referee must never trap an agent.
4. Docs: `docs/hooks/<agent>.md` (install snippet for the agent's config, what the agent sees, the env vars
   `COUNTERCHECK_MAX_NUDGES`, `COUNTERCHECK_HOOK_LOG`, `COUNTERCHECK_BIN`), plus a row in the README "Where it fits"
   table and the `where-it-fits.svg` left column.

## Tests (in `apps/countercheck/tests/verify.rs`, see the `claude_hook_*` tests)

- verified work → the agent may finish; failed + success claim → "keep going" with the failing check's name;
  failed + honest blocker → may finish; nudge limit reached → may finish;
- garbage stdin, missing cwd, a folder that isn't a git repo → exit 0 (fail open);
- the exact output format the agent expects (field names, exit code), copied from its docs.

## Verification

Run the gate from the `write-tests` skill, then try it for real once: install the hook in the agent, give it a task
whose visible test it's tempted to edit, and confirm it gets sent back. Note the agent version in the PR.

## Pitfalls

- Never decide in the hook with your own rules: `hook::decide` and `claim::reports_blocker` are shared so every
  agent gets the same referee.
- Some agents send the last message, others a transcript path (`last_assistant_text` reads Claude-style JSONL).
- Hooks have timeouts (Claude Code / Cortex Code: 60 s by default): mention `timeout` in the docs for slow checks.
