# Which agents Countercheck can stop at "done"

An agent is **gated** when its own "about to finish" hook runs Countercheck and its answer can send the agent back
to work. Everywhere else Countercheck still checks the work, through `countercheck verify` or CI, but it can only
**report**: it can't stop the agent from saying "done".

Set up a gated agent with one command (`countercheck hook install --agent <name>`; `countercheck hook status` shows
what's installed). Agent hooks can be switched off by the agent itself in some setups (see
[What a local hook can and can't stop](claude-code.md#what-a-local-hook-can-and-cant-stop)), so keep the
[GitHub Action](../guides/github-action.md) on your pull requests as the check nobody can skip.

## Coverage

| Agent | Finish hook | Countercheck today | How sure |
|---|---|---|---|
| Claude Code | `Stop` (block + reason) | **Gated**: `hook install --agent claude-code`; `--scope managed` so it can't be switched off ([page](claude-code.md#strict-mode-managed-settings)) | Tested end to end in CI (every hook path, all three OSes) |
| Snowflake Cortex Code | `Stop`, same format as Claude Code | **Gated**: `hook install --agent cortex-code` ([page](claude-code.md#cortex-code)) | Used in a real project (Cortex Code 1.2.24): pushed back on FAILED, VERIFIED after the fix |
| Goose | `Stop` (block + reason), from a plugin's `hooks/hooks.json` | **Gated**: `hook install --agent goose` ([page](goose.md)) | Tested end to end in CI; run for real with Goose 1.57.0 against a stand-in model (blocked a false claim 3 times, then let it stop at the limit) |
| Hermes Agent | `pre_verify` (continue + message) | **Gated**: `hook hermes-pre-verify`, set up by hand ([page](hermes.md)) | Used in the 29-task benchmark |
| OpenAI Codex | `Stop` (block + reason); hooks must be trusted once | **Gated**: `hook install --agent codex`, then trust it in Codex ([page](codex.md)) | Tested end to end in CI with fixtures from Codex's hook schemas; not yet run in a real Codex session |
| Cursor (editor, CLI, cloud agents) | `stop` (`followup_message`) | **Gated**: `hook install --agent cursor` ([page](cursor.md)) | Tested end to end in CI with payloads from Cursor's docs; not yet run against Cursor |
| GitHub Copilot (CLI, cloud agent, VS Code) | `agentStop` / `Stop` (block + reason) | **Gated**: `hook install --agent copilot`; the cloud agent needs `countercheck` installed by its setup steps ([page](copilot.md)) | Tested end to end in CI with payloads from the docs; not yet run in a live Copilot session |
| Kiro (IDE, CLI V3; not Web) | Agent Stop (block + reason) | **Gated**: `hook install --agent kiro` ([page](kiro.md)) | Tested end to end in CI with Kiro's documented payloads; not yet tried on a real install |
| Devin CLI | `Stop` (block + reason), in `.devin/hooks.v1.json` | **Gated**: `hook install --agent devin` ([page](devin.md)) | Tested end to end in CI with payloads from Devin's docs and CLI binary (3000.11.234); not yet run in a real Devin session |
| JetBrains Junie CLI (EAP; CLI interactive and `-p`, not the IDE/ACP) | `Stop` (block + reason); no hooks from project files | **Gated**: `hook install --agent junie`, user scope only ([page](junie.md)) | Tested end to end in CI with payloads from Junie's docs and its 26.11.3 code; not yet run in a real Junie session |
| Amp, OpenClaw | a blocking finish hook | Report only | Planned |
| Amp | `agent.end` in a plugin (continue + user message) | **Gated**: `hook install --agent amp` writes the plugin ([page](amp.md)) | Tested end to end in CI with the plugin's payloads, and the plugin run under Bun with a stand-in for Amp's API; not yet run in a live Amp thread |
| Devin CLI, JetBrains Junie CLI, OpenClaw | a blocking finish hook | Report only | Planned |
| Gemini CLI | `AfterAgent` (documented, but an open bug says it never runs) | Report only | Waiting on the bug |
| Windsurf / Devin Desktop, Cline, OpenCode, Aider, Zed's agent, Warp | no finish hook that can block | **Report only**: `countercheck verify` after the agent, or CI | Can't be gated until the agent adds a hook |

"Report only" still gives the full check (locked checks, clean copy, sandbox, sealed receipt) and an exit code; it
just runs after the agent instead of stopping it. The usual ways: ask the agent to run
`countercheck verify --claim "<what you did>"` before it says done (put that in its instructions file), run it
yourself, or let CI run it on every pull request.

Hook details come from each agent's own documentation (checked in October 2026); "How sure" says which rows were
also run against the real agent. Since 0.19 every project hook file `hook install` writes is guarded: if the agent removes
or weakens Countercheck's hook in a committed file, the next run is FAILED, naming the file. User files (`--scope
user`, and Junie's `~/.junie/config.json`) are outside the repository and aren't compared.

## Adding an agent

Each agent is one file in `apps/countercheck/src/agents/` plus a test file; the step-by-step guide is
[.claude/skills/add-agent-hook/SKILL.md](../../.claude/skills/add-agent-hook/SKILL.md).
