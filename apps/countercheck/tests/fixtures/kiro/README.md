# Kiro hook payloads

Copied verbatim from Kiro's docs on 2026-10-30, not recorded from a real run (Kiro needs a login):

- `stop.json`: the Agent Stop event on STDIN, <https://kiro.dev/docs/hooks/types.md#agent-stop>.
- `agent-spawn.json`: the session-start event (CLI 2.x `agentSpawn`; CLI V3 calls the trigger `SessionStart`),
  <https://kiro.dev/docs/hooks/types.md#agent-spawn-cli-2x-and-v3-alias>.
- `block.json`: a Stop hook's "keep working" answer on STDOUT, same page, Agent Stop section.

The tests replace `cwd` with a real project. `assistant_response` (the final answer) is not in the public docs; the
Kiro CLI 2.36.0 binary lists it as a hooks keyword, so the tests that add it are marked as inferred.
