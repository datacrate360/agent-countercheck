Amp hook inputs. Amp itself never calls `countercheck`: Countercheck's plugin
(`apps/countercheck/src/agents/amp/plugin.ts`) handles Amp's `agent.end` and `session.start` events and sends these
payloads on stdin, so their shape comes from the plugin's source. The fields it reads from Amp (`thread.id`,
`status`, `messages[].role/content/usage.model`) are from the Plugin API reference
(https://ampcode.com/docs/plugin-api, `AgentEndEvent`, `SessionStartEvent`, `ThreadAssistantMessage`), checked on
2026-10-30. Ids and the model are made up; the tests replace `__CWD__` and `__MESSAGE__`.

`drive.ts` loads the plugin under Bun with a stub of Amp's API and fires `agent.end`
(`bun drive.ts <plugin> <project folder or null> <final message> [status]`, thread id from `THREAD`); it prints what
the plugin returned to Amp and what it showed the user. `tests/hook_amp.rs` runs it when `bun` is on PATH.
