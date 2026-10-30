# Goose hook payloads

- `stop-docs.json`: the `Stop` example from Goose's hooks guide
  (documentation/docs/guides/context-engineering/hooks.md, "Hook Payload"), verbatim. No folder: the hook runs in
  Goose's working directory.
- `stop-source.json`, `session-start-source.json`: the shape Goose 1.57.0 serializes (`HookContext` in
  crates/goose/src/hooks/mod.rs; Stop built in crates/goose/src/agents/state_machine/ops_stop_hook.rs, SessionStart in
  crates/goose/src/agents/state_machine/session.rs, both with `working_dir`). `matcher_context` is serialized even
  when empty; the other `None` fields are skipped.
- `session-start-client.json`: SessionStart as a client fires it (`emit_session_start` in
  crates/goose/src/agents/services.rs): no `working_dir`.

Not recorded from a live run. The tests fill in `working_dir` with the test project.
