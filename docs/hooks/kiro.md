# Kiro Stop hook

`countercheck hook kiro-stop` runs when Kiro finishes a turn (its **Agent Stop** hook, trigger `Stop`). It runs
`countercheck verify` on the working tree. If the checks FAILED, it answers with Kiro's "keep working" format, so Kiro
starts another turn with the failing check's name and output. Otherwise Kiro finishes. The rules are the shared ones
(see [the Claude Code page](claude-code.md)): VERIFIED or UNVERIFIED may stop; FAILED is sent back; FAILED with an
honest blocker report may stop unless protected files were changed; after `COUNTERCHECK_MAX_NUDGES` push-backs Kiro
may stop and the receipt says the work did NOT pass.

Hooks run in the **Kiro IDE (1.0 and later)** and **Kiro CLI V3**. **Kiro Web doesn't run hooks**: it can store
hook configuration, but Web sessions never fire them, so there use `countercheck verify` or CI.

## Setup

Commit `.countercheck/checks.toml` first (`countercheck init`), then:

```sh
countercheck hook install --agent kiro       # --scope user: ~/.kiro/hooks/, every workspace (CLI V3 only)
git add .kiro/hooks/countercheck.json && git commit -m "Countercheck hooks"
```

Countercheck writes **its own file**, `.kiro/hooks/countercheck.json`, so your other hook files are never touched.
It shows the change and asks first (`--yes`, `--dry-run`), keeps the previous file as
`countercheck.json.countercheck-backup` when one existed, and adds nothing when the hooks are already there.
`countercheck hook uninstall --agent kiro` takes out only Countercheck's two hooks. When nothing else was in the file,
it leaves `{"version": "v1"}`: delete the file then.

What lands in the file:

```json
{
  "version": "v1",
  "hooks": [
    {"name": "countercheck-session-start", "trigger": "SessionStart", "action": {"type": "command", "command": "countercheck hook session-start"}, "timeout": 10},
    {"name": "countercheck-stop", "trigger": "Stop", "action": {"type": "command", "command": "countercheck hook kiro-stop"}, "timeout": 600}
  ]
}
```

- `SessionStart` records the commit the session started from, so the checks stay locked to it even if the agent
  commits an edited test. CLI 2.x called this event `agentSpawn`; CLI V3 still accepts that name.
- Stop hooks take no `matcher`.
- `timeout` is in seconds. Kiro's default is 60; raise it above your checks' run time (`0` turns it off).

### The step after install

Kiro loads hooks when a session starts, so **start a new session**.

- **Kiro CLI** (2.33 and later, V3 sessions): the first time you start it in a folder, it asks **Trust this
  workspace?** and loads the folder's hooks only after you trust it. `--no-interactive` runs don't ask.
- **Kiro IDE**: the hooks appear in the **Agent Hooks** panel. Check that both are enabled (eye icon).

Writes to `.kiro/hooks/` always ask for approval in Kiro, so the agent can't quietly switch its own hook off from
inside a session. A person can still disable it in the IDE panel. Countercheck's tamper check compares
`.kiro/hooks/countercheck.json` with the base (0.19+): a removed, disabled or weakened hook makes the run FAILED. Keep
CI as the check nobody can skip.

## What Kiro sends and gets back

Kiro's documented Stop input:

```json
{"hook_event_name": "Stop", "cwd": "/current/working/directory", "session_id": "abc123-def456-789"}
```

There is no transcript path and, in the public docs, no final message. Without the final message there is no claim
on the receipt, and an agent that honestly reports a blocker is still sent back (up to the nudge limit). If Kiro
sends `assistant_response` (the CLI's own help names that field for hooks; the public docs don't), it is used as the
claim. If `cwd` is missing, the folder Kiro runs the hook in is used (Kiro runs command hooks in the project root).

To send the agent back, the hook prints `{"decision": "block", "reason": "<the failing checks>"}` and exits 0. For
`Stop`, "block" means "keep working". Kiro also starts another turn on **exit 1**, so this hook always exits 0, even
when it fails: a broken referee lets the agent finish, and the error goes to stderr.

When Kiro may stop, the hook prints one plain line (`✓ Countercheck: VERIFIED, …`, `Countercheck: UNVERIFIED: …`,
`⚠ Countercheck: FAILED …`). Where Kiro shows a Stop hook's output varies by surface; the receipt and
`COUNTERCHECK_HOOK_LOG` are the reliable record.

## Loop limit

Kiro documents no cap on how often a Stop hook may start another turn, so Countercheck's own limit is the only one:
`COUNTERCHECK_MAX_NUDGES` (default 3) push-backs per session, then Kiro may stop and the user is told the work did
NOT pass.

## Environment

| Variable | Effect |
|---|---|
| `COUNTERCHECK_MAX_NUDGES` | push-backs per session before Kiro may stop (default 3) |
| `COUNTERCHECK_HOOK_LOG` | append one JSON line per call (verdict, receipt, nudged, gave_up) |
| `COUNTERCHECK_HOOK_DISABLED=1` | the hook does nothing |
| `COUNTERCHECK_BASE` | lock the checks to this revision instead of the session's start |

When `countercheck` isn't on Kiro's PATH, install with `--bin <full path to countercheck>`. Receipts name the agent `kiro`. `countercheck verify` run by hand inside Kiro isn't named automatically: Kiro sets no
documented environment variable that says it is Kiro, so pass `--agent kiro`.

## Sources

Checked 2026-10-30: [Hooks](https://kiro.dev/docs/hooks.md) (file schema, locations, timeout),
[Hook types](https://kiro.dev/docs/hooks/types.md) (Agent Stop: IDE ✓, CLI ✓, Web —; input and answer),
[Hooks migration](https://kiro.dev/docs/cli/v3/hooks-migration.md) (trigger names),
[Agent configuration reference](https://kiro.dev/docs/custom-agents/configuration-reference.md#hooks-field) (inline
profile hooks), [Permissions](https://kiro.dev/docs/permissions.md#workspace-trust) (workspace trust, `.kiro/hooks`
writes). Not yet tried against a running Kiro (it needs a login).

### Why a standalone file, not profile hooks

Kiro CLI V3 also reads hooks inline in an agent profile (`.kiro/agents/*.json`, a `hooks` array; CLI 2.x used an
object keyed by camelCase events). Those run only for that profile and only in the CLI. A standalone
`.kiro/hooks/*.json` file runs for every profile, in both the IDE and the CLI, and is a file Countercheck can own
outright. If you also put `countercheck hook kiro-stop` in a profile, each stop is checked twice: keep one.
