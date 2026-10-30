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
| Hermes Agent | `pre_verify` (continue + message) | **Gated**: `hook hermes-pre-verify`, set up by hand ([page](hermes.md)) | Used in the 29-task benchmark |
| OpenAI Codex | `Stop` (block + reason); hooks must be trusted once | **Gated**: `hook install --agent codex`, then trust it in Codex ([page](codex.md)) | Tested end to end in CI against fixtures from Codex's hook schemas; not yet tried in a real Codex session |
| Cursor | `stop` (`followup_message`) | Report only, for now | Adapter in progress |
| GitHub Copilot (CLI, cloud agent, VS Code) | `agentStop` / `Stop` (block + reason) | Report only, for now | Adapter in progress |
| Goose | `Stop` (block + reason, or exit 2) | Report only, for now | Adapter in progress |
| Kiro (IDE, CLI V3; not Web) | Agent Stop (block + reason) | **Gated**: `hook install --agent kiro` ([page](kiro.md)) | Tested end to end in CI with Kiro's documented payloads; not yet tried on a real install |
| Goose | `Stop` (block + reason, or exit 2), from a plugin's `hooks/hooks.json` | **Gated**: `hook install --agent goose` ([page](goose.md)) | Tested end to end in CI; tried with Goose 1.57.0 against a stand-in model (blocked a false claim, then gave up at the limit) |
| Kiro (IDE, CLI) | Agent Stop (block + reason) | Report only, for now | Adapter in progress |
| Devin CLI, JetBrains Junie CLI, Amp, OpenClaw | a blocking finish hook | Report only | Planned |
| Gemini CLI | `AfterAgent` (documented, but an open bug says it never runs) | Report only | Waiting on the bug |
| Windsurf / Devin Desktop, Cline, OpenCode, Aider, Zed's agent, Warp | no finish hook that can block | **Report only**: `countercheck verify` after the agent, or CI | Can't be gated until the agent adds a hook |

"Report only" still gives the full check (locked checks, clean copy, sandbox, sealed receipt) and an exit code; it
just runs after the agent instead of stopping it. The usual ways: ask the agent to run
`countercheck verify --claim "<what you did>"` before it says done (put that in its instructions file), run it
yourself, or let CI run it on every pull request.

Hook details come from each agent's own documentation (checked in October 2026). Rows marked "Adapter in progress"
or "Planned" were not tested against the real agent yet; each adapter's own page will say what was.

## Adding an agent

Each agent is one file in `apps/countercheck/src/agents/` plus a test file; the step-by-step guide is
[.claude/skills/add-agent-hook/SKILL.md](../../.claude/skills/add-agent-hook/SKILL.md).
