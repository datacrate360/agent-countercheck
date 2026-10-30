Copilot hook payloads, built from the docs (not recorded from a live session, which needs a Copilot login):

- `agent-stop.json`, `session-start.json`: the camelCase input of `agentStop` / `sessionStart`,
  https://docs.github.com/en/copilot/reference/hooks-reference
- `stop-vscode-style.json`: the snake_case input a PascalCase `Stop` entry gets (same page).
- `events.jsonl`: the session event log `transcriptPath` names, shaped by `schemas/session-events.schema.json`
  in the `@github/copilot` 1.0.105 package (`assistant.message` with `data.content`; sub-agent events carry
  `agentId`).

`SESSION`, `CWD`, `TRANSCRIPT` and `CLAIM` are filled in by `tests/hook_copilot.rs`.
