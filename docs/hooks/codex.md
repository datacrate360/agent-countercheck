# OpenAI Codex Stop hook

`countercheck hook codex-stop` runs when Codex (the `codex` CLI, and the Codex app on the same config) is about to
end a turn. It runs `countercheck verify` on the working tree. If the checks FAILED, it answers
`{"decision":"block","reason":...}`: Codex doesn't end the turn, it carries on with the reason as a new prompt.
Otherwise Codex finishes, and the user sees one line as a warning in the Codex UI (`systemMessage`):

| Outcome | The user sees |
|---|---|
| VERIFIED | `✓ Countercheck: VERIFIED, 2 check(s) passed in a clean copy, including uncommitted changes (receipt …)` |
| UNVERIFIED | `Countercheck: UNVERIFIED: <the first reason> (receipt …)` |
| FAILED, the agent reports a blocker | `⚠ Countercheck: FAILED (…); the agent says it is blocked, so it may stop. The work is not done (receipt …)` |
| FAILED after the push-back limit | `⚠ Countercheck: FAILED (…). Stopped sending the agent back after the push-back limit: the work did NOT pass (receipt …)` |

The rules are the same as for every agent ([Claude Code page](claude-code.md)): only FAILED sends Codex back; an
agent whose last message already says it is stuck may stop, unless it edited a protected file. The claim on the
receipt is `last_assistant_message` from the hook input; when Codex sends `null` there, there is no claim. Receipts
say `codex`, with the model from the hook input.

## Setup

Commit `.countercheck/checks.toml` first (`countercheck init`), then:

```sh
countercheck hook install --agent codex      # --scope user for every project (~/.codex/hooks.json)
git add .codex/hooks.json && git commit -m "Countercheck hooks"
```

It shows the change and asks first (`--yes`, `--dry-run`), keeps the previous file as `hooks.json.countercheck-backup`,
leaves your other hooks as written, and adds nothing the second time. `countercheck hook status` shows it;
`countercheck hook uninstall --agent codex` takes only ours out. If `countercheck` isn't on the PATH Codex's shell
sees, pass `--bin <full path>`.

What lands in `.codex/hooks.json`:

```json
{
  "hooks": {
    "SessionStart": [
      {"hooks": [{"type": "command", "command": "countercheck hook session-start", "timeout": 10}]}
    ],
    "Stop": [
      {"hooks": [{"type": "command", "command": "countercheck hook codex-stop", "timeout": 600}]}
    ]
  }
}
```

Install writes `hooks.json` only. Codex also reads `[hooks]` tables in `config.toml` in the same folder: if that file
already runs Countercheck, install warns, because each stop would be checked twice. Keep one. Set `timeout` (seconds,
Codex's default is 600) longer than your checks take.

## Trust the hook, or it never runs

**Codex skips every hook you haven't trusted.** It records trust against each hook's hash, so a new or edited hook is
skipped until you review it again. Right after installing:

1. Start `codex` in the project. If Codex asks whether you trust this folder, say yes: hooks in a project's
   `.codex/` load only when that folder is trusted. (User hooks in `~/.codex/hooks.json` skip this step.)
2. At **Hooks need review**, pick **Review hooks**, check that both commands run `countercheck hook ...`, and trust
   them. **Trust all and continue** does the same for every pending hook at once. **Continue without trusting** means
   they don't run.
3. Or, inside any session: type `/hooks`, and press `t` to trust (on the events list, `t` trusts all that need
   review). The hook's detail page shows `Trust: Trusted` when it's done.

Editing the hooks, or a `countercheck hook install` that changes them, asks again. Managed hooks (system, MDM, cloud
or `requirements.toml`) are trusted by policy and need no review.

`codex exec --dangerously-bypass-hook-trust` runs enabled hooks without the stored trust for one run; Codex's docs
mean it for automation that already vets its hook files. Don't put it where an agent could edit `.codex/hooks.json`.

## Why `SessionStart`

Codex's `SessionStart` sends `session_id` and `cwd`, like Claude Code's. `countercheck hook session-start` records
the commit the session started from (in `.git/countercheck/sessions/`), and the Stop hook locks the checks to it, so an
agent that **commits** an edit to a test or to `checks.toml` doesn't move the rules with it. It fires on every
`source` (`startup`, `resume`, `clear`, `compact`, `fork`); a resumed session keeps its first record.

## `verify` from a Codex shell

Without `--agent`, `countercheck verify` run by Codex labels the receipt `codex`: `CODEX_THREAD_ID` is set in every
shell Codex starts (`CODEX_SANDBOX=seatbelt` only under the macOS sandbox). A label, never a proof.

## Environment

| Variable | Effect |
|---|---|
| `COUNTERCHECK_BASE` | Revision the checks come from. Default: the commit the session started from (needs `SessionStart`), else `HEAD`. |
| `COUNTERCHECK_HOOK_DISABLED=1` | The hook does nothing. |
| `COUNTERCHECK_SANDBOX=none` | Run checks without Countercheck's sandbox. |
| `COUNTERCHECK_RECEIPTS` | Folder for receipts (default `.countercheck/receipts`). |
| `COUNTERCHECK_HOOK_LOG` | Append one JSON line per call to this file. |
| `COUNTERCHECK_MAX_NUDGES` | Push-backs allowed per session (default 3). Codex documents no cap of its own, so this is the only one. |

Codex runs hooks with the session's `cwd` as the working folder, through a login shell (`sh -lc` on macOS and
Linux), so your login PATH applies.

## Fails open

Bad input, a folder that isn't a git repository, no check file: the hook prints the error on stderr, exits 0 with
nothing on stdout, and Codex finishes. A broken referee must never trap an agent.

## Not covered yet

- **Tamper check:** `countercheck verify` compares `.codex/hooks.json` with the base (0.19+): an agent that removes
  or weakens the hook there makes the run FAILED. Since 0.20.1 `.codex/config.toml` is compared too: `[features] hooks
  = false` (or the old `codex_hooks = false`, or either under a profile's `features`) there, or a Countercheck entry in its `[hooks]` tables removed or changed,
  makes the run FAILED. Your user files (`~/.codex/config.toml`, `~/.codex/hooks.json`) aren't compared; keep the check
  in CI (`countercheck verify --base origin/main`).
- **Not tried in a real Codex session yet.** Checked against Codex 0.166.0 (`codex --version`, `codex features list`:
  `hooks stable true`) and its source (`codex-rs/hooks`, Oct 30 2026). The input and output shapes come from Codex's
  generated hook schemas; see the PR for each claim's source.

Sources: [Codex hooks docs](https://learn.chatgpt.com/docs/hooks) (moved from developers.openai.com/codex/hooks),
[openai/codex](https://github.com/openai/codex) `codex-rs/hooks/`.
