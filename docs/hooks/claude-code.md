# Claude Code (and Cortex Code) Stop hook

`countercheck hook claude-stop` runs when Claude Code tries to finish a turn. It runs `countercheck verify` on the
working tree. If the checks FAILED, it blocks the stop and tells the agent why, so the agent keeps working.
Otherwise it says nothing and the agent stops.

It uses the same rules as the Hermes hook:

- **VERIFIED or UNVERIFIED:** the agent may stop. The receipt is written either way.
- **FAILED:** the stop is blocked. The reason lists the failed checks and their last output.
- **FAILED, but the agent's last message already says it is stuck** (for example "can't pass, it contradicts ..."),
  and it changed no protected file: the agent may stop. It is telling the truth, and the FAILED receipt is kept.
- **FAILED after protected tests were edited:** the stop is blocked, whatever the agent says.

The claim on the receipt is the agent's final message: `last_assistant_message` from the hook input. If that field
is missing, the hook reads the last assistant text from the transcript file. If there's no transcript, there's no
claim.

## Setup

Commit `.countercheck/checks.toml` first (`countercheck init`), then add this to `.claude/settings.json`:

```json
{
  "hooks": {
    "SessionStart": [
      {
        "hooks": [
          { "type": "command", "command": "countercheck hook session-start", "timeout": 10 }
        ]
      }
    ],
    "Stop": [
      {
        "hooks": [
          { "type": "command", "command": "countercheck hook claude-stop", "timeout": 600 }
        ]
      }
    ]
  }
}
```

Set `timeout` (seconds) longer than your checks take. If the hook times out, the agent stops.

**Why `SessionStart`:** it records the commit the session started from (in `.git/countercheck/sessions/`, outside
your files). The Stop hook then locks the checks to that commit. Without it the checks are locked to `HEAD`, so an
agent that **commits** an edit to a test or to `checks.toml` would move the "locked" rules with it. A resumed session
keeps its first start.

## Windows

Claude Code runs natively on Windows, and so do these hooks. Use the same settings; `countercheck.exe` must be on
`PATH` (`install.ps1` tells you how), or give its full path with forward slashes:
`"command": "C:/Users/you/AppData/Local/Programs/countercheck/countercheck.exe hook claude-stop"`.

- The `cwd` and `transcript_path` in the hook input may use `\` or `/`; a transcript path starting with `~\` is read
  from `USERPROFILE` when `HOME` isn't set.
- Checks run with Git for Windows' `sh` (Claude Code on Windows needs Git for Windows too), or with
  `shell = "pwsh"`/`"cmd"` per check.
- `pwsh` and `cmd` checks run in the Windows sandbox (writes only in the check folder, the internet not blocked;
  the receipt says `windows-restricted-token (...)`); a run with `sh` checks runs without it, and the receipt says so.

## Environment

| Variable | Effect |
|---|---|
| `COUNTERCHECK_BASE` | Revision the checks come from. Default: the commit the session started from (needs the `SessionStart` hook), else `HEAD`. |
| `COUNTERCHECK_HOOK_DISABLED=1` | The hook does nothing. |
| `COUNTERCHECK_SANDBOX=none` | Run checks without the macOS sandbox. |
| `COUNTERCHECK_RECEIPTS` | Folder for receipts (default `.countercheck/receipts`). |
| `COUNTERCHECK_HOOK_LOG` | Append one JSON line per call to this file. |
| `COUNTERCHECK_MAX_NUDGES` | Push-backs allowed per session (default 3). |

## Loop guard

Countercheck counts how many times it has blocked the stop in each session. The count is kept in
`<receipts>/hook-state/<session_id>.json`. After `COUNTERCHECK_MAX_NUDGES` blocks, it lets the agent stop and logs
`"gave_up": true`. The receipt still says FAILED. Claude Code also has its own cap on how many times in a row Stop
hooks can block.

## Fails open

If anything goes wrong (bad input, no git repo, no check file), the hook prints the error on stderr, exits 0 and
lets the agent stop. A broken referee must never trap an agent.

## What a local hook can and can't stop

The hooks stop the common shortcuts: editing or skipping a protected test, editing `checks.toml`, committing either,
planting a fake output. An agent with a shell could still go further on purpose (rewrite files under `.git/`, run
with `COUNTERCHECK_HOOK_DISABLED=1`). The hard guarantee is the same check in CI, against a base the agent can't touch:
`countercheck verify --base origin/main`.

## Cortex Code

Snowflake's Cortex Code (CoCo) reads hooks from the same settings files (project `.claude/settings.json` or
`.cortex/settings.json`, and `~/.snowflake/cortex/hooks.json`), and sends the same fields on stdin (`session_id`,
`transcript_path`, `cwd`, `hook_event_name`), so the snippet above works there too. What differs, and what
Countercheck does about it:

- **No `last_assistant_message`.** The claim is read from Cortex Code's own session files in
  `~/.snowflake/cortex/conversations/`: `<id>.json`, with the messages in its `history` array or in
  `<id>.history.jsonl` next to it. `transcript_path` may name either file.
- **The receipt says `cortex-code`** when the transcript is under `.snowflake/cortex/`, else `claude-code`.
- **Whether Stop can push back is not settled.** Snowflake's Desktop hooks page says Stop "can force continuation";
  the CLI page lists Stop as unable to block. If the CLI ignores the push-back, the receipt is still written (FAILED
  when the work fails), but the agent is not sent back to work: read it with `countercheck report --open`.

Tested only with hook input and session files shaped from Snowflake's docs, its `cortex-code-agent-sdk` types and
the `cortex-replay` parser; not yet in a real Cortex Code session. If you try it, please
[open an issue](https://github.com/datacrate360/agent-countercheck/issues) with what you saw.
