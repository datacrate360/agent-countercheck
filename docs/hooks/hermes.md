# Hermes Agent pre_verify hook

`countercheck hook hermes-pre-verify` runs when Hermes is about to finish a turn in which it edited files. It runs
`countercheck verify` on each project the turn changed. If the checks FAILED, it answers
`{"action": "continue", "message": ...}` and Hermes keeps working with the failing checks in front of it.
Otherwise it says nothing and the turn ends.

Same rules as the [Claude Code hook](claude-code.md):

- **VERIFIED or UNVERIFIED:** the agent may finish. The receipt is written either way.
- **FAILED:** the agent is sent back with the failed checks and their last output.
- **FAILED, but the agent's final answer already says it is stuck**, and it changed no protected file: it may finish.
  It is telling the truth, and the FAILED receipt is kept.
- **FAILED after protected tests were edited:** sent back, whatever the agent says.

The claim on the receipt is Hermes's `final_response`.

## Which project gets checked

Hermes sends **its own** working directory as `cwd`. The desktop app and the messaging gateway run from your home
folder, not from the project, so `cwd` alone is often not a git repository at all.

So when Hermes lists the files the turn changed (`changed_paths`), the hook checks the **git repositories that hold
those files and have a `.countercheck/checks.toml`**, each one separately (up to 8). One profile can then work on
several projects, and only the ones the turn touched are checked:

| The turn edited | What runs |
|---|---|
| files in one project with `.countercheck/checks.toml` | that project's checks |
| files in two such projects | both; a failure names its project |
| only files outside any such project (notes, scratch files, another repo) | nothing (silent, no receipt) |
| nothing listed (`changed_paths` missing or empty) | `cwd`, as before |

Relative paths are read from `cwd`. A deleted file counts through its nearest folder that still exists.

## Setup

1. Commit `.countercheck/checks.toml` in the project (`countercheck init`).
2. Add the hooks to the profile's `config.yaml` (`~/.hermes/config.yaml`, or
   `~/.hermes/profiles/<name>/config.yaml` for a named profile):

   ```yaml
   hooks:
     on_session_start:
       - command: "countercheck hook session-start"
     pre_verify:
       - command: "countercheck hook hermes-pre-verify"
         timeout: 300
   ```

   Give `pre_verify` enough time for the slowest full run of your checks.

3. **Approve the hooks once.** Hermes only runs a shell hook after you allow that exact command. Until then it skips
   it, and nothing tells the agent:

   ```sh
   hermes hooks list     # "✗ not allowlisted" means the hook will NOT run
   hermes                # (or hermes -p <name>) asks "Allow this hook to run? [y/N]" once per hook: answer y
   hermes hooks list     # now "✓ allowed"
   ```

   The question is only asked in a terminal. The desktop app and the gateway never ask, so approve from a terminal
   first (or set `hooks_auto_accept: true` / `HERMES_ACCEPT_HOOKS=1` if you accept that any configured hook runs).
   Editing the command later needs a new approval.

4. Start a new session: hooks are read when a session starts.

## Locking the base

`countercheck hook session-start` remembers the commit a session started from, so checks and protected files stay
locked even if the agent commits its own edits. It uses `cwd` to find the repository. When Hermes runs from your home
folder there is no repository there, so nothing is recorded and the checks are locked to `HEAD` (the README's
"Agent hooks" section explains the risk). To lock a fixed base anyway, set `COUNTERCHECK_BASE` in the hook's
environment, or start Hermes from the project folder.

## Environment

| Variable | Effect |
|---|---|
| `COUNTERCHECK_HOOK_DISABLED=1` | the hook does nothing |
| `COUNTERCHECK_HOOK_LOG=<file>` | one JSON line per checked project: verdict, receipt, whether the agent was sent back |
| `COUNTERCHECK_BASE=<rev>` | lock the checks to this revision |
| `COUNTERCHECK_SANDBOX=none` | run checks without the macOS sandbox |

The hook fails open: if Countercheck itself breaks, the error goes to stderr and the agent may finish. A broken
referee must never trap the agent.
