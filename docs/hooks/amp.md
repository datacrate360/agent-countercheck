# Amp plugin

[Amp](https://ampcode.com) runs hooks as plugins: TypeScript files that handle its events. `countercheck hook
install --agent amp` writes one, `countercheck.ts`. When the agent ends a turn (Amp's `agent.end` event), the plugin
runs `countercheck hook amp-stop`, which runs `countercheck verify` on the working tree. If the checks FAILED, the
plugin starts another turn with the reason, so the agent keeps working. It uses the same rules as the
[Claude Code hook](claude-code.md): VERIFIED or UNVERIFIED may stop; FAILED is sent back; FAILED with an honest "I'm
blocked" may stop, unless protected files were changed.

**How sure:** built from Amp's [Plugin API reference](https://ampcode.com/docs/plugin-api) and
[Plugins guide](https://ampcode.com/docs/customize/plugins) (checked 2026-10-30). Tested end to end in CI with the
plugin's payloads. The plugin itself was run under Bun with a stand-in for Amp's API, and its `session.start` handler
through Amp's own `amp plugins exec` (Amp CLI 0.0.1793376030-g45d45f). Not yet run in a live Amp thread: that needs
an Amp account.

## Setup

Commit `.countercheck/checks.toml` first (`countercheck init`), then:

```sh
countercheck hook install --agent amp      # --scope user for every project
git add .amp/plugins/countercheck.ts && git commit -m "Countercheck hooks"
```

The plugin goes in `<project>/.amp/plugins/countercheck.ts`, or `~/.config/amp/plugins/countercheck.ts` with
`--scope user` (Windows: `%USERPROFILE%\.config\amp\plugins\`). If `XDG_CONFIG_HOME` is set, Amp looks in
`$XDG_CONFIG_HOME/amp/plugins/` instead; copy the file there by hand. When a project and a user plugin share a name,
Amp loads only the project one, so the check never runs twice.

Amp loads plugins when a session starts. In a running session, run `plugins: reload` from the command palette
(Ctrl+O). `amp plugins list` shows the plugin with its `agent.end` and `session.start` events. There is no approval
step, but Amp's docs say to load only plugins you trust: read the file first. It is short.

Countercheck owns the file. `hook install` writes it only where there is no file yet, or an older Countercheck
plugin. It refuses someone else's `countercheck.ts`. `hook uninstall --agent amp` deletes it only if it is exactly what
Countercheck wrote (pass the same `--bin`). `hook status` says when it was edited. If `countercheck` isn't on Amp's
PATH, pass `--bin <full path>`. It lands in the plugin as a string, so spaces are fine.

## What the plugin does

- **`session.start`:** runs `countercheck hook session-start` with the thread id and the project folder. It records
  the commit the thread started from, so an agent that commits an edit to a test or `checks.toml` doesn't move the
  checks with it. See [the Claude Code page](claude-code.md).
- **`agent.end`:** sends `{cwd, thread_id, status, model, last_assistant_message}` to `countercheck hook amp-stop`
  on stdin. The claim is the text of the turn's last assistant message, and the model is the one Amp recorded for it.
  Agent text is never put in a command line. If the answer is `{"action":"continue","userMessage":…}`, the plugin
  returns that to Amp, which starts the next turn with it. If the answer is `{"message":…}`, the plugin shows the line
  to the user (`ctx.ui.notify`) and lets Amp stop.
- **Which folder:** the workspace Amp has open (`amp.system.workspaceRoot`). Without one, it uses the folder the
  plugin runs in, which for a project plugin is `.amp/plugins/` inside the project. A user plugin without a workspace
  has no project: nothing is checked.
- A turn the user cancelled, or that ended in an error (`status` other than `done`), isn't the agent saying it is
  done. It isn't checked.

## What the agent and the user see

- **Sent back:** a new user message in the thread, starting "Countercheck re-ran this project's checks in a clean
  copy and they FAILED", then the reasons and the failing check's output.
- **Allowed:** a notification with the line: `✓ Countercheck: VERIFIED …`, `Countercheck: UNVERIFIED: …`, or
  `⚠ Countercheck: FAILED …` (an honest blocker, or the push-back limit). Where Amp can't show plugin UI, nothing
  is shown. Use `countercheck report --open`, the receipts in `.countercheck/receipts/`, or
  `COUNTERCHECK_HOOK_LOG`.

## Loop guard

Countercheck allows `COUNTERCHECK_MAX_NUDGES` push-backs per thread (default 3), then lets the agent stop and logs
`"gave_up": true`; the receipt still says FAILED. Amp ignores a plugin's `continue` after 5 in a row (any user message
resets that count). Keep Countercheck's limit below 5, so Countercheck's own give-up decides.

## Environment

| Variable | Effect |
|---|---|
| `COUNTERCHECK_BASE` | Revision the checks come from. Default: the commit the thread started from, else `HEAD`. |
| `COUNTERCHECK_HOOK_DISABLED=1` | The hook does nothing. |
| `COUNTERCHECK_SANDBOX=none` | Run checks without the sandbox. |
| `COUNTERCHECK_RECEIPTS` | Folder for receipts (default `.countercheck/receipts`). |
| `COUNTERCHECK_HOOK_LOG` | Append one JSON line per call to this file. |
| `COUNTERCHECK_MAX_NUDGES` | Push-backs allowed per thread (default 3). |

Set them in the environment Amp starts in. The plugin runs `countercheck` with that environment. Receipts from the
plugin name the agent `amp`. `countercheck verify` run by hand from Amp's shell tool doesn't know it is Amp yet: Amp
documents no variable that marks its shell.

## Fails open

If anything goes wrong, Amp stops as it would without the plugin: `countercheck` not found, a non-zero exit, output
that isn't JSON, no git repository, no check file. Errors go to the plugin's log (`amp.logger`). The plugin waits up
to 15 minutes for `countercheck`, then gives up and lets Amp stop. Keep your checks well under that.

## Limits

- **The agent can try to switch the plugin off** by deleting or editing `.amp/plugins/countercheck.ts`. Countercheck
  compares the committed plugin byte for byte with the base: any change makes the run FAILED, naming the file. The
  user-wide plugin isn't compared. Amp's own switches aren't watched yet: the `PLUGINS` environment variable
  (`PLUGINS=off` turns plugins off) and any plugin setting in `.amp/settings.json`. Keep the
  [GitHub Action](../guides/github-action.md) on your pull requests.
- **Remote executors and Orbs** run plugins where the thread runs: `countercheck` must be installed there.
- **Windows:** not tried yet.
