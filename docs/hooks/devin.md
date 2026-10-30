# Devin CLI Stop hook

`countercheck hook devin-stop` runs when the Devin CLI (`devin`, Cognition's terminal agent) is about to end its
turn. It runs `countercheck verify` on the working tree. If the checks FAILED, it answers
`{"decision":"block","reason":...}`: Devin doesn't stop, it carries on with the reason. Otherwise Devin finishes, and
the hook prints one line for the user as `systemMessage`:

| Outcome | The line |
|---|---|
| VERIFIED | `✓ Countercheck: VERIFIED, 2 check(s) passed in a clean copy, including uncommitted changes (receipt …)` |
| UNVERIFIED | `Countercheck: UNVERIFIED: <the first reason> (receipt …)` |
| FAILED, the agent reports a blocker | `⚠ Countercheck: FAILED (…); the agent says it is blocked, so it may stop. The work is not done (receipt …)` |
| FAILED after the push-back limit | `⚠ Countercheck: FAILED (…). Stopped sending the agent back after the push-back limit: the work did NOT pass (receipt …)` |

Devin CLI parses `systemMessage` from a hook's answer (it is a field of its Claude-format hook output), but its docs
don't say where it shows it; the receipt and `COUNTERCHECK_HOOK_LOG` have the same line either way.

The rules are the same as for every agent ([Claude Code page](claude-code.md)): only FAILED sends Devin back; an agent
whose last message already says it is stuck may stop, unless it edited a protected file. The claim on the receipt is
`last_assistant_message` from the Stop input (Devin CLI sends it since the release that added it; see its changelog);
without it there is no claim, never a pass. Receipts say `devin`, with the session's `session_id`.

## Setup

Commit `.countercheck/checks.toml` first (`countercheck init`), then:

```sh
countercheck hook install --agent devin
git add .devin/hooks.v1.json && git commit -m "Countercheck hooks"
```

It shows the change and asks first (`--yes`, `--dry-run`), keeps the previous file as
`hooks.v1.json.countercheck-backup`, leaves your other hooks as written, and adds nothing the second time.
`countercheck hook status` shows it; `countercheck hook uninstall --agent devin` takes only ours out. If `countercheck`
isn't on the PATH Devin's hooks see, pass `--bin <full path>`.

What lands in `.devin/hooks.v1.json` (in this file the hooks object is the whole file, with no `"hooks"` key):

```json
{
  "SessionStart": [
    {"matcher": "", "hooks": [{"type": "command", "command": "countercheck hook session-start", "timeout": 10}]}
  ],
  "Stop": [
    {"matcher": "", "hooks": [{"type": "command", "command": "countercheck hook devin-stop", "timeout": 600}]}
  ]
}
```

Set `timeout` (seconds) longer than your checks take: a hook that times out counts as an error, and Devin goes on
without it. Devin's docs give no default.

### Start a new session and trust the folder

Devin CLI loads hooks when a session starts. Start a new `devin` session in the project. If it asks **"Do you trust
the authors of this directory?"**, say yes. Then type `/hooks`: both Countercheck hooks should be listed, from
`.devin/hooks.v1.json`. (`devin -p` cannot show the trust prompt and fails in an untrusted folder unless you pass
`--respect-workspace-trust false`.)

### Every project (user level)

Install writes the project file only. Devin's user-level hooks sit under a `"hooks"` key in `~/.config/devin/config.json`
(`%APPDATA%\devin\config.json` on Windows), so add them there by hand:

```json
{
  "hooks": {
    "SessionStart": [{"matcher": "", "hooks": [{"type": "command", "command": "countercheck hook session-start", "timeout": 10}]}],
    "Stop": [{"matcher": "", "hooks": [{"type": "command", "command": "countercheck hook devin-stop", "timeout": 600}]}]
  }
}
```

A user-level hook isn't in the project, so the tamper check below can't see it removed.

## Don't run it twice

Devin CLI also runs the hooks in `.devin/config.json`, `.devin/config.local.json` and, by default, Claude Code's
`.claude/settings.json` and `.claude/settings.local.json` (and `~/.claude/settings.json`). If Claude Code's
Countercheck hooks are installed in the same project, Devin runs both stop hooks: each stop is checked twice, and the
`claude-stop` receipt says `claude-code`. `hook install --agent devin` warns when it sees this. Pick one:

- Use Devin only: `countercheck hook uninstall --agent claude-code`.
- Use both agents in the project: stop Devin from reading Claude Code's settings in your Devin user config
  (`~/.config/devin/config.json`):

  ```json
  {"read_config_from": {"claude": false}}
  ```

  This also stops Devin importing Claude Code's rules, skills and MCP servers; see
  [Configuration Import](https://docs.devin.ai/cli/reference/configuration/read-config-from).

## Why `SessionStart`

`countercheck hook session-start` records the commit the session started from (in `.git/countercheck/sessions/`), and
the Stop hook locks the checks to it, so an agent that **commits** an edit to a test or to `checks.toml` doesn't move
the rules with it. Devin's `SessionStart` sends `session_id` and `source`. It needs the project folder: `cwd` from the
input if Devin sends it, else the folder the hook runs in.

## Environment

| Variable | Effect |
|---|---|
| `COUNTERCHECK_BASE` | Revision the checks come from. Default: the commit the session started from (needs `SessionStart`), else `HEAD`. |
| `COUNTERCHECK_HOOK_DISABLED=1` | The hook does nothing. |
| `COUNTERCHECK_SANDBOX=none` | Run checks without Countercheck's sandbox. |
| `COUNTERCHECK_RECEIPTS` | Folder for receipts (default `.countercheck/receipts`). |
| `COUNTERCHECK_HOOK_LOG` | Append one JSON line per call to this file. |
| `COUNTERCHECK_MAX_NUDGES` | Push-backs allowed per session (default 3). Devin documents no cap of its own (only a warning that blocking stop hooks can loop), so this is the only one. |

The Stop hook finds the project in the input's `cwd` if present, else in `DEVIN_PROJECT_DIR` (set by Devin for every
hook to the hook's project root), else the folder it runs in.

## Fails open

Bad input, a folder that isn't a git repository, no check file: the hook prints the error on stderr, exits 0 with
nothing on stdout, and Devin finishes. A broken referee must never trap an agent. The hook never exits 2 (which Devin
also reads as "block").

## Not covered yet

- **Tamper check:** `countercheck verify` compares `.devin/hooks.v1.json` with the base: an agent that removes or
  weakens the hook there makes the run FAILED, naming the file. `.devin/config.json` and `.devin/config.local.json`
  aren't compared; keep the check in CI (`countercheck verify --base origin/main`).
- **`verify` from a Devin shell** isn't labelled `devin` yet: Devin documents no variable in the agent's own shell
  (`DEVIN_PROJECT_DIR` is for hooks). Pass `--agent devin`.
- **The session record** written at `SessionStart` can't tell Devin from Claude Code (same input shape), so a Devin
  session that never gets a receipt may show as `claude-code` on the project page. Receipts are always `devin`.
- **Devin cloud sessions and Devin Desktop:** not checked. The Devin CLI changelog mentions plugins that apply to
  `cloud`, but no page says cloud sessions run project hooks. Use `countercheck verify` or CI there.
- **Not tried in a real Devin session yet** (a session needs a Devin login). Checked against Devin CLI 3000.11.234: its
  bundled docs and changelog (`share/devin/docs/`) and the field names in its binary. See the PR for each claim's source.

Sources: [Hooks](https://docs.devin.ai/cli/extensibility/hooks/overview),
[Lifecycle Hooks](https://docs.devin.ai/cli/extensibility/hooks/lifecycle-hooks),
[Configuration Import](https://docs.devin.ai/cli/reference/configuration/read-config-from).
