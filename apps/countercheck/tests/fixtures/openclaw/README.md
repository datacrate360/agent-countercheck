What Countercheck's OpenClaw plugin (`apps/countercheck/src/agents/openclaw/index.ts`) writes to
`countercheck hook openclaw-stop` on stdin: a subset of OpenClaw's `before_agent_finalize` event
(`PluginHookBeforeAgentFinalizeEvent` in the `openclaw` npm package 2026.10.9,
`dist/config-contracts-*.d.ts`; the embedded runner fills it in `dist/builtin-openclaw-*.mjs`).
Built from that source and https://docs.openclaw.ai/plugins/hooks/prompt-and-session, not recorded
from a live run. The tests replace `__CWD__` and `__MESSAGE__`; ids and model are made up.
