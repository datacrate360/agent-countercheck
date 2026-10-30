# Devin CLI hook payloads

Built from Devin CLI's docs and binary on 2026-10-30 (Devin CLI 3000.11.234), not recorded from a real run (a session
needs a Devin login). Ids, the model and the message are made up; the tests replace `cwd` and the message.

- `stop.json`: only the fields the hooks pages list for `Stop`: `hook_event_name`, `session_id`, `prompt_id` (every
  event) and `stop_hook_active` (<https://docs.devin.ai/cli/extensibility/hooks/overview>,
  <https://docs.devin.ai/cli/extensibility/hooks/lifecycle-hooks#stop>). No `cwd`: hooks get the project root in
  `DEVIN_PROJECT_DIR`, which the tests set.
- `stop-full.json`: `stop.json` plus `last_assistant_message`, which the CLI's changelog (bundled in
  `share/devin/docs/changelog/stable.mdx`) says `Stop` hooks receive, and `cwd`, `model` and `turn_id`, which only the
  binary's hook input type names (strings next to `hook_event_name` … `session_id prompt_id turn_id`). Inferred: whether
  Devin fills in `cwd` and `model` for `Stop` is not confirmed.
- `session-start.json`: `SessionStart` (`source`, documented; `cwd` from the binary's input type, inferred).
