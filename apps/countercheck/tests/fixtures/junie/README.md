Junie CLI hook inputs (stdin), not recorded from a live run (no Junie account was available).

- `stop.json`: exactly the docs' Stop example, the only fields the docs promise ("The Stop stdin contains only
  hook_event_name, stop_hook_active, and last_assistant_message"):
  https://junie.jetbrains.com/docs/junie-cli-hooks.html
- `stop-with-session.json`: what Junie CLI 26.11.3 (build 3959.2, EAP) builds for Stop, read from its bytecode
  (`StopHookInput.buildStdinJson` calls the shared `buildBaseStdinJson`, which adds `session_id` when known, `cwd`
  always and `project_path` when known, before `stop_hook_active` and `last_assistant_message`).
- `session-start.json`: the docs' SessionStart example.

The tests replace `__CWD__` and `__MESSAGE__`; the session id is made up.
