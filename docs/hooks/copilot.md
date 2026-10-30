# GitHub Copilot hooks (CLI, cloud agent, VS Code)

`countercheck hook copilot-stop` runs when Copilot tries to end a turn (`agentStop`). It runs `countercheck verify` on
the working tree. If the checks FAILED, it blocks the stop and tells the agent why, so it keeps working. Otherwise the
agent stops. The same hook file works in the three places Copilot runs an agent: the **Copilot CLI**, the **Copilot
cloud agent** (the one you assign issues to on GitHub), and **VS Code**'s Copilot agent, which uses the CLI's hook
format and reads the same files.

The rules are the same as for every agent ([Claude Code page](claude-code.md)):

- **VERIFIED or UNVERIFIED:** the agent may stop. The receipt is written either way.
- **FAILED:** the stop is blocked. The reason lists the failed checks and their last output.
- **FAILED, but the agent's last message already says it is stuck**, and it changed no protected file: the agent may
  stop. It is telling the truth, and the FAILED receipt is kept.
- **FAILED after protected tests were edited:** the stop is blocked, whatever the agent says.

Needs a Countercheck release with `hook copilot-stop` (the one after 0.18.0).

## Setup

Commit `.countercheck/checks.toml` first (`countercheck init`), then:

```sh
countercheck hook install --agent copilot          # .github/hooks/countercheck.json
git add .github/hooks/countercheck.json && git commit -m "Countercheck hooks"
```

`--scope user` writes `~/.copilot/hooks/countercheck.json` instead (every project, CLI and VS Code; the cloud agent
reads only the repository). It shows the change and asks first (`--yes`, `--dry-run`), adds nothing when the hooks are
there, and `countercheck hook uninstall --agent copilot` takes only ours out. If `countercheck` isn't on the agent's
PATH, pass `--bin <full path>`.

What lands in the file (Countercheck's own file, so your other hook files stay untouched):

```json
{
  "version": 1,
  "hooks": {
    "sessionStart": [
      {"type": "command", "bash": "countercheck hook session-start", "powershell": "countercheck hook session-start", "timeoutSec": 10}
    ],
    "agentStop": [
      {"type": "command", "bash": "countercheck hook copilot-stop", "powershell": "countercheck hook copilot-stop", "timeoutSec": 600}
    ]
  }
}
```

- `bash` runs on Linux and macOS, `powershell` on Windows (the cloud agent runs `bash` only).
- `timeoutSec`: Copilot's default is **30 seconds**; set it above the time your checks take. If the hook times out,
  the agent stops unchecked.
- `sessionStart` records the commit the session started from (in `.git/countercheck/sessions/`), so the checks stay
  locked to it even if the agent commits an edit to a test. Without it they are locked to `HEAD`.
- No trust or approval step: Copilot loads the hooks when a session starts. Start a new session after installing.

### Don't install it for Claude Code in the same project too

The Copilot CLI (and VS Code's Copilot agent) also runs the hooks in `.claude/settings.json` and
`.claude/settings.local.json`. If Countercheck is installed there for Claude Code as well, each Copilot stop is
checked twice (and that second run is labelled `claude-code`). `hook install` warns when it sees this; keep one. A team
that uses both agents can keep the Claude Code hook in `~/.claude/settings.json` (user scope), which Copilot doesn't read.

## What Copilot sends and gets back

`agentStop` input (stdin): `sessionId`, `cwd`, `transcriptPath`, `stopReason`, `stop_hook_active`. There is no final
message, so the claim on the receipt is the main agent's last text in the transcript (Copilot's `events.jsonl`: the
last `assistant.message` without an `agentId`); its model goes on the receipt too. A `Stop` entry (PascalCase) gets the
same fields in snake_case; the hook reads both.

Answer (stdout):

| Outcome | Output |
|---|---|
| FAILED, keep going | `{"decision":"block","reason":"…","hookSpecificOutput":{"hookEventName":"Stop","decision":"block","reason":"…"}}`: the top-level pair is the CLI's format, the `hookSpecificOutput` copy is VS Code's |
| may stop | `{"systemMessage":"✓ Countercheck: VERIFIED, …"}` (the same lines as [Claude Code's](claude-code.md)); VS Code shows it to you, the CLI's docs don't list `systemMessage` |

The receipts say `"agent": "copilot"`. `countercheck verify` run by the agent itself is labelled `copilot` too (the CLI
sets `COPILOT_CLI=1` for the commands it runs; the cloud agent sets `COPILOT_AGENT_PROMPT`).

## The cloud agent

The cloud agent works in an ephemeral, **non-interactive Linux sandbox** on GitHub Actions, with a firewall that limits
the network. For hooks that means:

- It reads hooks **only from `.github/hooks/*.json` in the repository** (not `.claude/`, not your user folder), from
  the branch it works on. Commit the hook file to your default branch.
- **Only `bash` (or `command`) entries run**; `powershell` is ignored.
- Its working folder is `/workspace` and `HOME` is `/root`. Files written there, receipts included, go when the job
  ends (receipts are never committed: their folder carries its own `.gitignore`).
- **`countercheck` must be installed in that sandbox first**, or the hook fails (and fails open: the agent finishes
  unchecked). Do it in `.github/workflows/copilot-setup-steps.yml`, the job Copilot runs before it starts. It must be
  on the default branch, and the job must be called `copilot-setup-steps`:

```yaml
name: Copilot Setup Steps

on:
  workflow_dispatch:
  push:
    paths: [.github/workflows/copilot-setup-steps.yml]
  pull_request:
    paths: [.github/workflows/copilot-setup-steps.yml]

jobs:
  copilot-setup-steps:
    runs-on: ubuntu-latest
    timeout-minutes: 10
    permissions:
      contents: read
    steps:
      - uses: actions/checkout@v4
      - name: Install Countercheck
        run: |
          curl -fsSL https://raw.githubusercontent.com/datacrate360/agent-countercheck/PROD/install.sh \
            | sudo env COUNTERCHECK_INSTALL_DIR=/usr/local/bin sh
          countercheck --version
```

`/usr/local/bin` is on every PATH, so the hook finds `countercheck` without `--bin`. The install script checks the
download against `SHA256SUMS.txt`. With Python on the runner, `pip install agent-countercheck` works too, if its
`bin` folder is on the agent's PATH. Run the workflow once from the Actions tab (`workflow_dispatch`) to see that
the install works.

The checks run with the tools the setup steps installed, and without the internet unless a check says
`network = true` (and the cloud agent's firewall allows the host).

## Windows

The Copilot CLI runs natively on Windows and runs the `powershell` entry. `countercheck.exe` must be on `PATH`
(`install.ps1` says how), or pass `--bin` with its full path: the hook then calls it as
`& "C:/Program Files/…/countercheck.exe" hook copilot-stop`. Checks run with Git for Windows' `sh`, or
`shell = "pwsh"`/`"cmd"` per check ([Claude Code page](claude-code.md#windows)).

## Environment

| Variable | Effect |
|---|---|
| `COUNTERCHECK_BASE` | Revision the checks come from. Default: the commit the session started from (needs `sessionStart`), else `HEAD`. |
| `COUNTERCHECK_HOOK_DISABLED=1` | The hook does nothing. |
| `COUNTERCHECK_SANDBOX=none` | Run checks without the sandbox. |
| `COUNTERCHECK_RECEIPTS` | Folder for receipts (default `.countercheck/receipts`). |
| `COUNTERCHECK_HOOK_LOG` | Append one JSON line per call to this file. |
| `COUNTERCHECK_MAX_NUDGES` | Push-backs allowed per session (default 3). |

A hook entry can set these in its own `env` block. Copilot's `COPILOT_HOME` moves the user hooks folder;
`hook install --scope user` always writes under your home folder.

## Loop guard

After `COUNTERCHECK_MAX_NUDGES` blocks in one session (default 3), the hook lets the agent stop, with a ⚠ line that
says the work did NOT pass; the receipt still says FAILED. Copilot also ends the turn itself after 8 blocks in a row.

## Fails open

If anything goes wrong (bad input, no git repo, no check file, `countercheck` missing), the hook prints the error on
stderr, exits 0 and lets the agent stop. A broken referee must never trap an agent.

## What it can't stop (yet)

The agent can edit `.github/hooks/countercheck.json` like any other file. Countercheck's tamper check watches Claude
Code's and Cortex Code's settings files, **not yet** `.github/hooks/`, so a Copilot agent that removes or weakens its
own hook isn't reported. Keep the [GitHub Action](../guides/github-action.md) on your pull requests: on a pull request
the checks run from the base branch, whatever the agent did to its hooks.

## Sources

Checked on 2026-10-30 against Copilot CLI 1.0.105:
[hooks reference](https://docs.github.com/en/copilot/reference/hooks-reference),
[about hooks](https://docs.github.com/en/copilot/concepts/customizations/hooks),
[customize the cloud agent's environment](https://docs.github.com/en/copilot/how-tos/use-copilot-agents/cloud-agent/customize-the-agent-environment),
[VS Code hooks reference](https://code.visualstudio.com/docs/agents/reference/hooks-reference),
[VS Code agent hooks](https://code.visualstudio.com/docs/agent-customization/hooks). The input and transcript shapes in
the tests come from these pages and the CLI's own event schema; the hook has not yet been run in a live Copilot session.
