# JetBrains Junie CLI Stop hook

**Junie runs no hooks from a project file.** Hooks in `<project>/.junie/config.json` are skipped and reported as a
hook configuration warning, because that file comes from the repository. Junie runs hooks only from your user
config `~/.junie/config.json` (`$JUNIE_HOME/config.json` when `JUNIE_HOME` is set), from files you pass with
`--config-location`, and from extensions you install through `/extensions`
([Junie hooks docs](https://junie.jetbrains.com/docs/junie-cli-hooks.html)).

So `countercheck hook install --agent junie` writes your **user** file, and `--scope project` is refused with the
reason: a project install would do nothing while looking installed. The hook then runs in every project you open with
Junie; in one without `.countercheck/checks.toml` it lets Junie finish and says UNVERIFIED.

`countercheck hook junie-stop` runs when Junie is about to submit a task as done. It runs `countercheck verify` on
the working tree. If the checks FAILED, it answers `{"decision":"block","reason":...}`: Junie doesn't submit, the
reason goes back to the agent, and it tries again. Otherwise Junie finishes.

| Outcome | The line Countercheck writes |
|---|---|
| VERIFIED | `✓ Countercheck: VERIFIED, 2 check(s) passed in a clean copy, including uncommitted changes (receipt …)` |
| UNVERIFIED | `Countercheck: UNVERIFIED: <the first reason> (receipt …)` |
| FAILED, the agent reports a blocker | `⚠ Countercheck: FAILED (…); the agent says it is blocked, so it may stop. The work is not done (receipt …)` |
| FAILED after the push-back limit | `⚠ Countercheck: FAILED (…). Stopped sending the agent back after the push-back limit: the work did NOT pass (receipt …)` |

That line goes in `systemMessage`, Junie's user-facing field, but Junie's docs say its Stop step doesn't show it in
the terminal yet; the same line also goes to stderr (Junie's debug log) and to `COUNTERCHECK_HOOK_LOG`. To see the
result, run `countercheck report --open` or look at the receipt. Countercheck never answers with `additionalContext`
on an allowed stop: Junie would run one more agent step with it.

The rules are the same as for every agent ([Claude Code page](claude-code.md)): only FAILED sends Junie back; an
agent whose last message already says it is stuck may stop, unless it edited a protected file. The claim on the
receipt is `last_assistant_message` from the hook input (an empty submission is no claim). Receipts say `junie`;
Junie's hook input has no model.

## Setup

Hooks are in Junie's **Early Access** build only (`curl -fsSL https://junie.jetbrains.com/install-eap.sh | bash`;
`junie --version` says `eap`). Commit `.countercheck/checks.toml` first (`countercheck init`), then:

```sh
countercheck hook install --agent junie      # writes ~/.junie/config.json
```

It shows the change and asks first (`--yes`, `--dry-run`), keeps the previous file as
`config.json.countercheck-backup`, leaves your other settings (`model`, other hooks) as written, and adds nothing the
second time. `countercheck hook status` shows it; `countercheck hook uninstall --agent junie` takes only ours out.
If `countercheck` isn't on the PATH Junie's shell sees, pass `--bin <full path>` (there is no `COUNTERCHECK_BIN`
variable). Start a new `junie` session to use
it.

What lands in `~/.junie/config.json`:

```json
{
  "hooks": {
    "SessionStart": [
      {"hooks": [{"type": "command", "command": "countercheck hook session-start", "timeout": 10}]}
    ],
    "Stop": [
      {"hooks": [{"type": "command", "command": "countercheck hook junie-stop", "timeout": 600}]}
    ]
  }
}
```

There is no `blockOnError`: with it, any crash of the hook would send the agent back, and a broken referee must never
trap an agent. `timeout` is Junie's Stop default (600 seconds); raise it if your checks take longer.

### For a team, or CI

A user file isn't shared. Two documented ways to share the same hooks, both chosen on purpose by whoever starts Junie:

- **`--config-location`**: commit the JSON above as, say, `tools/junie-hooks.json` and start Junie with
  `junie --config-location tools/junie-hooks.json` (CI: `JUNIE_CONFIG_LOCATION`). Junie runs hooks from files passed
  this way even in an untrusted project. Forget the flag and nothing is checked, so prefer the user file on your own
  machine.
- **An extension** (`hooks/hooks.json` at the extension root, Claude plugin layout) installed through `/extensions`.
  Junie copies extensions into `~/.junie/extensions/` and needs a marketplace for them; Countercheck doesn't write one.

Either way the file lives where an agent can edit it; keep the CI check (below).

## Why `SessionStart`

Junie's `SessionStart` sends `session_id`, `cwd` and `project_path`. `countercheck hook session-start` records the
commit the session started from (in `.git/countercheck/sessions/`, labelled `junie`), and the Stop hook locks the
checks to it when the Stop input carries the same `session_id`, so an agent that **commits** an edit to a test or to
`checks.toml` doesn't move the rules with it. Junie runs `SessionStart` in the background, without waiting for it.

Junie's docs say the Stop input holds only `hook_event_name`, `stop_hook_active` and `last_assistant_message`. Junie
26.11.3 also sends `session_id`, `cwd` and `project_path` (read from its code, not seen in a live run). The hook uses
them when they are there. Without them it checks the folder Junie runs it in, against `HEAD` (or `COUNTERCHECK_BASE`).

## Push-back limit

`COUNTERCHECK_MAX_NUDGES` (default 3) push-backs per session, then Junie may stop with the ⚠ line. Junie's own cap
(`JUNIE_STOP_HOOK_BLOCK_CAP`, 8 consecutive blocks per task) is higher, so Countercheck's limit is the one that
applies. Without a `session_id` in the input the limit counts per task: Junie sets `stop_hook_active` to `false` on a
task's first Stop, which starts a new count (two Junie sessions without ids in one repository share it).

## Environment

| Variable | Effect |
|---|---|
| `COUNTERCHECK_BASE` | Revision the checks come from. Default: the commit the session started from (needs `SessionStart` and a `session_id` in the Stop input), else `HEAD`. |
| `COUNTERCHECK_HOOK_DISABLED=1` | The hook does nothing. |
| `COUNTERCHECK_SANDBOX=none` | Run checks without Countercheck's sandbox. |
| `COUNTERCHECK_RECEIPTS` | Folder for receipts (default `.countercheck/receipts`). |
| `COUNTERCHECK_HOOK_LOG` | Append one JSON line per call to this file. |
| `COUNTERCHECK_MAX_NUDGES` | Push-backs allowed per session (default 3); keep it below `JUNIE_STOP_HOOK_BLOCK_CAP` (8). |

Junie runs hook commands with `sh -c` (macOS, Linux) or `cmd /c` (Windows).

## Fails open

Bad input, a folder that isn't a git repository, no check file: the hook prints the error on stderr, exits 0 with
nothing on stdout, and Junie submits. (Junie treats an exit code other than 0 and 2 as a warning, not a block.)

## Not covered yet

- **Tamper check:** `~/.junie/config.json` is outside the repository, so `countercheck verify` can't compare it with
  the base (like every agent's user file). An agent that edits it can switch the hook off. Keep the check in CI
  (`countercheck verify --base origin/main`, or the GitHub Action).
- **Where Junie runs hooks:** the CLI's interactive and batch (`-p`) modes. Junie's docs say ACP and server hosts
  (the JetBrains IDE integration runs through ACP) don't run hooks yet, and Stop hooks don't run for chat-type tasks.
- **`verify` from a Junie shell:** Junie documents no variable that marks its shells, so `countercheck verify`
  without `--agent` doesn't label the receipt `junie`.
- **Not tried in a real Junie session.** Checked against Junie CLI 26.11.3 (build 3959.2, EAP: `junie --version`,
  `--help`, its code for the hook input and where `config.json` lives); no account was available to run a task.

Sources: [Junie hooks docs](https://junie.jetbrains.com/docs/junie-cli-hooks.html),
[config.json](https://junie.jetbrains.com/docs/junie-cli-configuration.html),
[extensions](https://junie.jetbrains.com/docs/junie-cli-extensions.html),
[environment variables](https://junie.jetbrains.com/docs/environment-variables.html).
