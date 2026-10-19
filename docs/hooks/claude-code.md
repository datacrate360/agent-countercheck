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

## Environment

| Variable | Effect |
|---|---|
| `COUNTERCHECK_BASE` | Revision the checks come from (default `HEAD`). Set it to a fixed commit if the agent commits. |
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

## Cortex Code

Snowflake's Cortex Code reads the same hook format from the same settings files, so the snippet above works there
too. Receipts record the agent as `claude-code`, because the hook input doesn't say which of the two tools is
running. This adapter hasn't been tested in a real Cortex Code session.
