# Cursor hook fixtures

- `stop.json`, `session-start.json`: the field names and example values of the common input and the
  `stop` / `sessionStart` inputs in <https://cursor.com/docs/hooks> (Reference → Common schema, Hook
  events), read on 2026-10-30, merged into one payload each as Cursor sends them. `{{ROOT}}` and
  `{{TRANSCRIPT}}` are filled in by the test. Not recorded from a real run (no Cursor account here).
- `transcript-*.jsonl`: the transcript layout community parsers report for
  `~/.cursor/projects/<slug>/agent-transcripts/<id>/<id>.jsonl` (one `{"role", "message": {"content"}}`
  per line). Cursor's docs don't describe the layout, so this is inferred, not documented.
