# Claude Code (and Cortex Code) Stop hook

`countercheck hook claude-stop` runs when Claude Code tries to finish a turn (every turn end, not only when the agent
says "done"). It runs `countercheck verify` on the working tree. If the checks FAILED, it blocks the stop and tells
the agent why, so the agent keeps working. Otherwise the agent stops, and the user sees one line in the agent's
window (`systemMessage`), so a pass never looks like a hook that didn't run:

| Outcome | The user sees |
|---|---|
| VERIFIED | `✓ Countercheck: VERIFIED, 2 check(s) passed in a clean copy, including uncommitted changes (receipt …)` |
| UNVERIFIED | `Countercheck: UNVERIFIED: <the first reason> (receipt …)` |
| FAILED, the agent reports a blocker | `⚠ Countercheck: FAILED (…); the agent says it is blocked, so it may stop. The work is not done (receipt …)` |
| FAILED after the push-back limit | `⚠ Countercheck: FAILED (…). Stopped sending the agent back after the push-back limit: the work did NOT pass (receipt …)` |

"Including uncommitted changes" means the receipt covers the files on disk: commit them, or HEAD may not pass.

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

Commit `.countercheck/checks.toml` first (`countercheck init`), then let Countercheck add the hooks:

```sh
countercheck hook install --agent claude-code      # or cortex-code; --scope user for every project
git add .claude/settings.json && git commit -m "Countercheck hooks"
```

It prints the change and asks before writing (`--yes` skips the question, `--dry-run` only shows it), keeps the
previous file as `settings.json.countercheck-backup`, leaves every other setting as written, and adds nothing when the
hooks are already there. `countercheck hook status` lists which agents are hooked here and for your user, and
whether the project file is committed; `countercheck hook uninstall --agent claude-code` takes only ours out. If
`countercheck` isn't on the agent's PATH, pass `--bin <full path>`.

The same thing by hand, in `.claude/settings.json`:

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

`countercheck hook stop` is the same hook under a shorter name (0.18+); Cortex Code users can use it so the
settings don't say "claude". Changing an existing hook to it is a settings change like any other: commit it, or the
next run reports the hook as changed.

**Why `SessionStart`:** it records the commit the session started from (in `.git/countercheck/sessions/`, outside
your files). The Stop hook then locks the checks to that commit. Without it the checks are locked to `HEAD`, so an
agent that **commits** an edit to a test or to `checks.toml` would move the "locked" rules with it. A resumed session
keeps its first start. It also records a fingerprint of each folder in `link` (a `.venv`): if one changes during the
session (a package installed or edited), the stop says UNVERIFIED, since the checks may have run with tools the agent
changed. Start a new session after you change the venv yourself.

## Windows

Claude Code runs natively on Windows, and so do these hooks. Use the same settings; `countercheck.exe` must be on
`PATH` (`install.ps1` tells you how), or give its full path with forward slashes:
`"command": "C:/Users/you/AppData/Local/Programs/countercheck/countercheck.exe hook claude-stop"`.

- The `cwd` and `transcript_path` in the hook input may use `\` or `/`; a transcript path starting with `~\` is read
  from `USERPROFILE` when `HOME` isn't set.
- Checks run with Git for Windows' `sh` (Claude Code on Windows needs Git for Windows too), or with
  `shell = "pwsh"`/`"cmd"` per check.
- `pwsh` and `cmd` checks run in the Windows sandbox (writes only in the check folder, the internet not blocked;
  the receipt says `windows-restricted-token (...)`); `sh` checks run without it, and the receipt marks each one.

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

## The agent can't quietly switch the hook off

The hook lives in a file the agent can edit, and Claude Code picks up edits mid-session: one line in
`.claude/settings.json` or `.claude/settings.local.json` (`"disableAllHooks": true`) stops every hook, and a project
setting overrides yours. So every `verify` (and every hook run) compares the agent's hook settings files
(`.claude/settings.json`, `.claude/settings.local.json`, `.cortex/settings.json`, `.cortex/settings.local.json`;
since 0.19 also the files `hook install` writes for Cursor, Codex, Copilot, Goose and Kiro, and Goose's project
`.config/goose/settings*.json`) in the working tree, git-ignored ones included, with the base. The run is **FAILED**, whatever the checks say, and the
receipt lists the change under `hook_settings` when the agent:

- turned on `disableAllHooks`;
- removed or changed a Countercheck hook (its command, `timeout` or type);
- set a `COUNTERCHECK_*` variable in the file's `env` block that changes what the hook checks (any except
  `COUNTERCHECK_ROLE` and `COUNTERCHECK_HOOK_LOG`);
- replaced the file with a symlink or something that isn't a JSON object.

Adding other hooks or settings is fine. If you made the change yourself, commit it: the base then has it.

The hooks compare these files with the commit the session started from (recorded by the `SessionStart` hook), not
with `COUNTERCHECK_BASE`: a settings file's `env` block can set that variable for the hook, so an agent could
otherwise commit its change and point the base past it. Without the `SessionStart` hook the hooks compare with the
same base as the checks.

A hook that is switched off writes no receipt, so the run that catches it is the next `verify`: another agent's hook,
your own `countercheck verify`, or CI. Until then, `countercheck report` and `countercheck ui` show a warning at the
top while hooks are switched off in the project's settings files.

They also list every session that is **Not checked**: the `SessionStart` hook recorded it starting (in
`.git/countercheck/sessions/`) more than 30 minutes ago, and it has no receipt and no run in progress. Its Stop hook
was switched off or removed, it timed out, or the agent was never given a task. The list is display only: it never
changes a verdict. It needs the `SessionStart` hook from the setup above.

## What a local hook can and can't stop

The hooks stop the common shortcuts: editing or skipping a protected test, editing `checks.toml`, committing either,
planting a fake output, switching the hook off in the project's settings. An agent with a shell could still go further
on purpose (rewrite files under `.git/`, edit your user settings in `~/.claude/`, run with
`COUNTERCHECK_HOOK_DISABLED=1` in its own shell). The hard guarantee is the same check in CI, against a base the
agent can't touch: `countercheck verify --base origin/main`.

### Strict mode: managed settings

```sh
sudo countercheck hook install --agent claude-code --scope managed    # Windows: an administrator terminal
```

This writes Countercheck's own file into Claude Code's managed settings folder
(`managed-settings.d/50-countercheck.json` in `/Library/Application Support/ClaudeCode/` on macOS,
`/etc/claude-code/` on Linux and WSL, `C:\Program Files\ClaudeCode\` on Windows), next to any policy already
there, never inside it. The command names `countercheck` by its full path, since the hook runs for every user of the
machine. Per Claude Code's docs ([hooks](https://code.claude.com/docs/en/hooks.md),
[managed settings](https://code.claude.com/docs/en/managed-settings.md)): hooks from managed settings merge with
everyone's own hooks, and `disableAllHooks` in user, project or local settings can't switch them off, so the agent
(which can't write a system folder without an administrator password) can't either. Not tried on a real machine yet:
it would hook every session on it.

Limits: if your organization already delivers managed settings by MDM or from the claude.ai console, Claude Code may
use that source instead of the files, so ask your administrator to add the hook there; cloud sessions don't read this
folder. `countercheck hook status` shows the managed line; `sudo countercheck hook uninstall --agent claude-code
--scope managed` removes it.

## Cortex Code

Snowflake's Cortex Code (CoCo) reads hooks from the same settings files (project `.claude/settings.json` or
`.cortex/settings.json`, and `~/.snowflake/cortex/hooks.json`), and sends the same fields on stdin (`session_id`,
`transcript_path`, `cwd`, `hook_event_name`), so the snippet above works there too. What differs, and what
Countercheck does about it:

- **No `last_assistant_message`.** The claim is read from Cortex Code's own session files in
  `~/.snowflake/cortex/conversations/`: `<id>.json`, with the messages in its `history` array or in
  `<id>.history.jsonl` next to it. `transcript_path` may name either file.
- **The receipt says `cortex-code`** when the transcript is under `.snowflake/cortex/`. When the Stop input has no
  transcript path, the app the `SessionStart` hook recorded for that session is used; else `claude-code`. A label
  only: the model inside Cortex Code (Claude or another) never changes the verdict.
- **Push-back:** Snowflake's Desktop hooks page says Stop "can force continuation"; the CLI page lists Stop as unable
  to block. Not yet seen in a real session. Either way the FAILED receipt is written: read it with
  `countercheck report --open`.

**Seen in a real setup** (Cortex Code 1.2.24, macOS, a user's project, October 2026): hooks load from the project's
`.cortex/settings.json` when a session starts (a session already open doesn't pick up a new hook); the `Stop` hook ran
at every turn end and its receipts say `cortex-code`; `SessionStart` recorded the session. Cortex Code also loads
`AGENTS.md`, `CLAUDE.md`, `CORTEX.md`, `RULES.md` and `.cursorrules` from the project as instructions (see `/rules`).

**Claude Code and Cortex Code in one project:** Claude Code reads only `.claude/settings.json`. Snowflake's docs say
Cortex Code reads `.claude/settings.json` too; if both files hold the hook and a Cortex session writes two receipts
per stop, keep only `.claude/settings.json`. Each session gets its own base, push-back count and receipts, and
`COUNTERCHECK_ROLE` in each file's `env` names them on the page. Please
[open an issue](https://github.com/datacrate360/agent-countercheck/issues) with what you see.
