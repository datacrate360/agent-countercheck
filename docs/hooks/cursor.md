# Cursor stop hook

`countercheck hook cursor-stop` runs when a Cursor agent's loop ends (Cursor's `stop` hook). It runs `countercheck
verify` on the working tree. If the checks FAILED, it answers `{"followup_message": "<why>"}`, which Cursor sends as
the next user message, so the agent keeps working. Otherwise it answers `{}` and the agent stops.

Same rules as every Countercheck hook (see [claude-code.md](claude-code.md) for the full table):

- **VERIFIED or UNVERIFIED:** the agent may stop. The receipt is written either way.
- **FAILED:** the agent is sent back with the failed checks and their last output.
- **FAILED, but the agent's last message says it is stuck**, and it changed no protected file: it may stop; the
  FAILED receipt is kept.
- **FAILED after protected tests were edited:** sent back, whatever the agent says.

Only a stop with `"status": "completed"` is checked. `aborted` (you pressed stop) and `error` (the agent failed) are
not a claim that the work is done, so nothing runs and the agent stops.

Hook docs used: <https://cursor.com/docs/hooks> and <https://cursor.com/docs/reference/third-party-hooks> (read
2026-10-30). This adapter was built from those docs; it has not been run against a real Cursor install yet.

## Setup

Commit `.countercheck/checks.toml` first (`countercheck init`), then:

```sh
countercheck hook install --agent cursor      # --scope user for every project (~/.cursor/hooks.json)
git add .cursor/hooks.json && git commit -m "Countercheck hooks"
```

It shows the change and asks first (`--yes`, `--dry-run`), keeps the previous file as
`hooks.json.countercheck-backup`, leaves your other hooks as written, and adds nothing when ours are there.
`countercheck hook status` shows it; `countercheck hook uninstall --agent cursor` takes only ours out. If
`countercheck` isn't on the PATH Cursor's hooks see, pass `--bin <full path>`.

What lands in `.cursor/hooks.json`:

```json
{
  "version": 1,
  "hooks": {
    "sessionStart": [
      {"command": "countercheck hook session-start", "timeout": 10}
    ],
    "stop": [
      {"command": "countercheck hook cursor-stop", "timeout": 600}
    ]
  }
}
```

Cursor watches `hooks.json` and reloads it on save; there is no approval step. Check that both hooks are listed in
the **Hooks** tab of **Customize** (or the **Hooks** output channel). Project hooks run only in a **trusted**
workspace. Set `timeout` (seconds) longer than your checks take.

**Why `sessionStart`:** it records the commit the conversation started from, so the stop hook locks the checks to
it even if the agent commits a test edit. Cursor's `sessionStart` gives `session_id` (the same value as
`conversation_id`) and `workspace_roots`, which Countercheck reads. It is fire-and-forget: Cursor doesn't wait for
it.

## Where it runs

| Where | `stop` | `sessionStart` | Which file |
|---|---|---|---|
| Cursor editor (Agent chat) | yes | yes | project and user |
| Cloud agents | yes | **no** (Cursor defers it) | project file only (`~/.cursor` isn't there) |
| `cursor-agent` CLI | yes in 2026.10.22 (read from its code, not run) | same | project and user |

In a cloud agent there's no session start, so the checks are locked to `HEAD`: an agent that commits a weakened
test moves the base. Keep the [GitHub Action](../guides/github-action.md) on pull requests for that case.

**The CLI:** Cursor's hooks page doesn't list the CLI separately. The code of `cursor-agent` 2026.10.22-27dcf5f
calls the `stop` hook with `status: "completed"`, `loop_count` and `workspace_roots`, and queues a non-empty
`followup_message` as the next user message; this was read from the package, not run (it needs a login). Forum
threads from 2026 report builds that skipped hooks (for example 2026.08.11 on Linux), so check yours with one
failing task.

## What the agent and you see

- Sent back: the message is the next user turn in the chat, starting "Countercheck re-ran this project's checks in
  a clean copy and they FAILED".
- Let through: Cursor's `stop` answer has no field shown to the user, so the one-line result
  (`✓ Countercheck: VERIFIED, …` / `UNVERIFIED: …` / `⚠ … FAILED …`) goes to stderr, which shows in the **Hooks**
  output channel. The receipt is in `.countercheck/receipts/` (`countercheck report --open`).

**The claim:** Cursor's `stop` input has no final message. Countercheck reads the last assistant text from
`transcript_path`. Cursor doesn't document the transcript layout; the reader handles the JSON Lines layout
community tools report (`{"role":"assistant","message":{"content":[...]}}` per line) and the plain-text one
(`assistant:` lines). With transcripts off (`transcript_path` is null) there's no claim, so an honest "I'm blocked"
can't be recognised and a FAILED stop is always sent back (up to the push-back limit).

**Which project:** the first entry of `workspace_roots` (else the hook's own folder). In a multi-root workspace only
the first root is checked.

## Running twice with Claude Code hooks

Cursor also runs Claude Code's hooks from `.claude/settings.json`, `.claude/settings.local.json` and
`~/.claude/settings.json` when **Cursor Settings → Agents → Third-Party Imports → Include Third-Party Plugins,
Skills, and Other Configs** is on, and it is on by default. If the project also has `countercheck hook install
--agent claude-code`, each Cursor stop is checked twice (once through `claude-stop`, labelled `claude-code`). `hook
install --agent cursor` warns about this. Keep one: switch that setting off, or don't install the Cursor hook and let
Cursor run the Claude Code one (Cursor accepts its `decision: "block"` answer as a follow-up, but gives that hook no
loop limit).

## Loop guard

Countercheck allows `COUNTERCHECK_MAX_NUDGES` (default 3) push-backs per conversation, counted in
`<receipts>/hook-state/<conversation_id>.json`; then it lets the agent stop with the ⚠ line, and the receipt still
says FAILED. Cursor's own cap is `loop_limit`, 5 per script by default, so Countercheck's limit is reached first.
If you raise `COUNTERCHECK_MAX_NUDGES` above 5, set `"loop_limit"` on the stop entry too.

## Environment

| Variable | Effect |
|---|---|
| `COUNTERCHECK_MAX_NUDGES` | Push-backs allowed per conversation (default 3). |
| `COUNTERCHECK_HOOK_LOG` | Append one JSON line per call to this file. |
| `COUNTERCHECK_BASE` | Revision the checks come from. Default: the commit the conversation started from, else `HEAD`. |
| `COUNTERCHECK_HOOK_DISABLED=1` | The hook does nothing. |
| `COUNTERCHECK_SANDBOX=none` | Run checks without the sandbox. |
| `COUNTERCHECK_RECEIPTS` | Folder for receipts (default `.countercheck/receipts`). |

There is no `COUNTERCHECK_BIN` variable: to run a `countercheck` that isn't on PATH, install with `--bin <full path>`,
which writes that path into the hooks file.

Receipts are labelled `cursor`. A `countercheck verify` run from Cursor's agent shell is labelled `cursor` too
(`CURSOR_AGENT`, or `CURSOR_VERSION`, which Cursor sets for hooks).

## Fails open

Bad input, not a git repository, no check file: the hook prints the error on stderr, exits 0 and lets the agent
stop. A broken referee must never trap an agent.

## Not covered yet

- The tamper check (`hook_settings`) doesn't watch `.cursor/hooks.json` yet, so an agent that deletes the hook
  there isn't reported by `verify`. Commit the file, and keep CI as the check nobody can skip.
- `subagentStop` isn't hooked: only the main agent's finish is checked.
