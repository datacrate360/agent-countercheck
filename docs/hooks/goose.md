# Goose Stop hook

`countercheck hook goose-stop` runs when [Goose](https://github.com/aaif-goose/goose) tries to end a turn. It runs
`countercheck verify` on the working tree. If the checks FAILED, it blocks the stop and tells the agent why, so the
agent keeps working. It uses the same rules as the [Claude Code hook](claude-code.md): VERIFIED or UNVERIFIED may
stop; FAILED is sent back; FAILED with an honest "I'm blocked" may stop, unless protected files were changed.

Tried with Goose 1.57.0 (CLI, `goose run`), against a local stand-in for the model: a false "all tests pass" was
blocked three times, then let through at the push-back limit with a FAILED receipt; passing work ended the turn at
once.

## Setup

Commit `.countercheck/checks.toml` first (`countercheck init`), then:

```sh
countercheck hook install --agent goose      # --scope user for every project
git add .agents/plugins/countercheck/hooks/hooks.json && git commit -m "Countercheck hooks"
```

Goose reads hooks only from plugins, so Countercheck installs as its own plugin, named `countercheck`:
`<project>/.agents/plugins/countercheck/hooks/hooks.json`, or `~/.agents/plugins/countercheck/hooks/hooks.json`
with `--scope user`. No `plugin.json` is needed: Goose loads any folder there that has `hooks/hooks.json`. There is no
approval step; Goose picks the plugin up the next time it starts.

**Start Goose from the project's root folder.** Goose looks for project plugins in the folder it starts in, not the
repository root: started from a subfolder, it doesn't see the project's plugin and nothing is checked. If you work
from subfolders, use `--scope user`. With both installed, Goose loads only the project one (one plugin per name), so
the check never runs twice.

What lands in the file:

```json
{
  "hooks": {
    "SessionStart": [
      {"hooks": [{"type": "command", "command": "countercheck hook session-start", "timeout": 10}]}
    ],
    "Stop": [
      {"hooks": [{"type": "command", "command": "countercheck hook goose-stop", "timeout": 600}]}
    ]
  }
}
```

The install prints the change and asks first (`--yes`, `--dry-run`), keeps any previous file as
`hooks.json.countercheck-backup`, and leaves other hooks in the file as written. `countercheck hook status` shows it;
`countercheck hook uninstall --agent goose` takes out only Countercheck's entries. If `countercheck` isn't on Goose's
PATH, pass `--bin <full path>`.

Goose's default hook timeout is 30 seconds, and a Stop hook that times out lets the turn end. Set `timeout`
(seconds) longer than your checks take.

**Why `SessionStart`:** it records the commit the session started from, so an agent that commits an edit to a test
or `checks.toml` doesn't move the checks with it. See [the Claude Code page](claude-code.md) for details.

## What the agent and the user see

- **Blocked:** the agent gets the reason as a message, wrapped by Goose: ``Stop hook `countercheck` blocked ending
  this turn: … Address this policy hook denial before trying to stop again.`` The user sees ``· Stop hook
  `countercheck` blocked ending this turn.``
- **Allowed:** Goose shows nothing. Unlike Claude Code, it has no way for a Stop hook to show a line to the user. The
  hook still writes the `✓ Countercheck: VERIFIED …` line to stderr, but Goose drops it. To see results, use
  `countercheck report --open`, the receipts in `.countercheck/receipts/`, or `COUNTERCHECK_HOOK_LOG`.

## Input and answer

Goose sends `{"event":"Stop","session_id":…,"last_assistant_message":…,"working_dir":…}` on stdin. The hook checks
`working_dir`. Without it (Goose's docs example has none), it checks the folder the hook runs in, which is the folder
Goose started in. The claim on the receipt is `last_assistant_message`. Goose's hook input has no model, so receipts
from this hook don't name one.

It answers `{"decision":"block","reason":…}` to send the agent back, and `{"decision":"allow"}` otherwise. That is
why Goose has its own hook: anything else on stdout, such as Claude Code's `systemMessage`, counts as a failed hook
in Goose's log.

## Loop guard

Countercheck allows `COUNTERCHECK_MAX_NUDGES` push-backs per session (default 3), then lets the agent stop and logs
`"gave_up": true`; the receipt still says FAILED. Goose has its own cap on blocks in a row within a turn
(`GOOSE_STOP_HOOK_BLOCK_CAP`, default 8). Keep Countercheck's limit below it, so Countercheck's own give-up decides.

## Environment

| Variable | Effect |
|---|---|
| `COUNTERCHECK_BASE` | Revision the checks come from. Default: the commit the session started from, else `HEAD`. |
| `COUNTERCHECK_HOOK_DISABLED=1` | The hook does nothing. |
| `COUNTERCHECK_SANDBOX=none` | Run checks without the sandbox. |
| `COUNTERCHECK_RECEIPTS` | Folder for receipts (default `.countercheck/receipts`). |
| `COUNTERCHECK_HOOK_LOG` | Append one JSON line per call to this file. |
| `COUNTERCHECK_MAX_NUDGES` | Push-backs allowed per session (default 3). |

Set them in the environment Goose starts in: hooks inherit it. `countercheck verify` run from Goose's shell tool
names the agent `goose` on the receipt (Goose sets `GOOSE_TERMINAL=1` and `AGENT=goose` there).

## Fails open

If anything goes wrong (bad input, not a git repository, no check file), the hook prints the error on stderr, exits 0
with nothing on stdout, and Goose ends the turn. Goose also lets the turn end on any Stop hook failure.

## Limits

- **The agent can try to switch the plugin off,** with `disabledPlugins` in `<project>/.config/goose/settings.json`
  or `settings.local.json`, or by editing `.agents/plugins/countercheck/hooks/hooks.json`. Countercheck's tamper
  check compares all three with the base (0.19+): either makes the run FAILED, naming the file. The user-wide
  `~/.config/goose/` files aren't compared. Keep the [GitHub Action](../guides/github-action.md) on your pull requests.
- **Windows:** Goose runs hooks with `sh -c` on every OS, so on Windows it needs an `sh` on PATH (Git for Windows).
  Not tried yet.
- `GOOSE_PATH_ROOT` moves Goose's user plugin folder to `$GOOSE_PATH_ROOT/.agents/plugins/`. `--scope user` writes
  under your home folder; copy the file there by hand.
