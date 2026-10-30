# OpenClaw: `before_agent_finalize` plugin

Countercheck's OpenClaw plugin runs when OpenClaw is about to accept an agent's final answer
(`before_agent_finalize`). It hands the event to `countercheck hook openclaw-stop`, which runs `countercheck verify` on
the agent's working folder. If the checks FAILED, the plugin returns `{ action: "revise", reason }`: OpenClaw doesn't
deliver the answer, it runs one more model pass with the reason. Otherwise the answer goes through, and one line lands
in OpenClaw's log (`[plugins] ✓ Countercheck: VERIFIED, 1 check(s) passed in a clean copy …`).

## Fit check: why this is set up by hand

`countercheck hook install --agent openclaw` does not write the plugin; it points here. OpenClaw is a gateway and
runner for many agents and channels, not one coding CLI, and its plugins don't fit the "one file in the project" that
`hook install` writes:

- **A plugin is a folder of files, not one file** (docs, observed). OpenClaw needs `openclaw.plugin.json` next to the
  code even for a single-file plugin (the loader reads the manifest from the entry's folder), and
  `openclaw plugins install --link` also needs a `package.json`.
- **Plugins aren't found in the git repository** (docs, source). OpenClaw looks in `plugins.load.paths`, in its own
  agent workspace (`<workspace>/.openclaw/extensions`; the workspace defaults to `~/.openclaw/workspace`, not your
  project) and in `~/.openclaw/extensions`. Workspace plugins are off until you enable them.
- **The hook needs a grant in OpenClaw's config** (docs, observed). A plugin that isn't bundled with OpenClaw gets
  `before_agent_finalize` only with `plugins.entries.countercheck.hooks.allowConversationAccess: true` in
  `~/.openclaw/openclaw.json`.

So the plugin is installed once per machine, with OpenClaw's own commands, which also record where it came from.

## Setup

Commit `.countercheck/checks.toml` in each project first (`countercheck init`). Then, once:

1. Put the three files below in a folder you own (not world-writable; OpenClaw refuses those), for example
   `~/.local/share/countercheck/openclaw/`. In a clone of this repository they are
   `apps/countercheck/src/agents/openclaw/`.
2. Install, enable and grant it:

   ```sh
   openclaw plugins install --link ~/.local/share/countercheck/openclaw --force --accept-capabilities
   openclaw plugins enable countercheck
   openclaw config set plugins.entries.countercheck.hooks.allowConversationAccess true
   ```

   `--force` acknowledges a local source; `--accept-capabilities` is the consent OpenClaw asks for (the plugin declares
   none beyond its hook). If `countercheck` isn't on the Gateway's PATH:
   `openclaw config set plugins.entries.countercheck.config.bin /full/path/to/countercheck`.
3. Point the agent at the project: OpenClaw sends the run's `cwd` (`agents.defaults.cwd` or the agent's own), else its
   workspace. That folder must be inside the git repository, or the plugin lets every answer through.
4. Check it: `openclaw plugins inspect countercheck --runtime --json` shows `"status": "loaded"`,
   `"typedHooks": [{"name": "before_agent_finalize"}]` and `"allowConversationAccess": true`.

`openclaw plugins uninstall countercheck` takes it out.

`openclaw.plugin.json`:

```json
{
  "id": "countercheck",
  "name": "Countercheck",
  "description": "Re-runs the project's locked checks before OpenClaw accepts a final answer, and asks for another pass while they fail",
  "categories": ["developer-tools"],
  "activation": { "onCapabilities": ["hook"] },
  "configSchema": {
    "type": "object",
    "additionalProperties": false,
    "properties": {
      "bin": { "type": "string", "description": "The command that runs Countercheck (default: countercheck on PATH)" }
    }
  }
}
```

`package.json`:

```json
{
  "name": "countercheck",
  "version": "0.19.0",
  "private": true,
  "type": "module",
  "openclaw": { "extensions": ["./index.ts"] }
}
```

`index.ts`:

```ts
// Countercheck for OpenClaw: before a final answer is accepted (`before_agent_finalize`), run
// `countercheck hook openclaw-stop` with the event as JSON on stdin, and ask for one more pass
// when it says so. Countercheck decides; this file only relays. It fails open: an error, a
// timeout or output it doesn't expect lets the answer through. No shell: agent text only ever
// travels on stdin.
import { spawn } from "node:child_process";

// Checks can take minutes; OpenClaw's default budget for this hook is 15 s.
const TIMEOUT_MS = 600_000;
const MAX_OUTPUT = 1_000_000;

function run(bin: string, input: string): Promise<string> {
  return new Promise((resolve) => {
    let out = "";
    let child: any;
    try {
      child = spawn(bin, ["hook", "openclaw-stop"], { stdio: ["pipe", "pipe", "inherit"] });
    } catch {
      resolve("");
      return;
    }
    const timer = setTimeout(() => child.kill(), TIMEOUT_MS - 5_000);
    child.stdout.on("data", (d: unknown) => {
      if (out.length < MAX_OUTPUT) out += String(d);
    });
    child.on("error", () => {
      clearTimeout(timer);
      resolve("");
    });
    child.on("close", () => {
      clearTimeout(timer);
      resolve(out);
    });
    child.stdin.on("error", () => {});
    child.stdin.end(input);
  });
}

export default {
  id: "countercheck",
  name: "Countercheck",
  description: "Re-runs the project's locked checks before OpenClaw accepts a final answer",
  register(api: any) {
    const configured = api.pluginConfig?.bin;
    const bin = typeof configured === "string" && configured ? configured : "countercheck";
    api.on(
      "before_agent_finalize",
      async (event: any) => {
        try {
          const input = JSON.stringify({
            cwd: event.cwd,
            sessionId: event.sessionId,
            runId: event.runId,
            provider: event.provider,
            model: event.model,
            lastAssistantMessage: event.lastAssistantMessage,
          });
          const answer = JSON.parse((await run(bin, input)).trim() || "{}");
          if (answer.action === "revise" && typeof answer.reason === "string" && answer.reason) {
            return { action: "revise", reason: answer.reason };
          }
          if (typeof answer.message === "string") api.logger?.info?.(answer.message);
        } catch (e) {
          api.logger?.warn?.(`countercheck: ${String(e)}`);
        }
        return undefined;
      },
      { timeoutMs: TIMEOUT_MS },
    );
  },
};
```

The plugin is a relay: it never builds a shell command (the event goes to `countercheck` on stdin), and it fails open:
an error, a missing `countercheck`, a timeout or output it doesn't expect lets the answer through. It sets its own
hook budget to 10 minutes, since OpenClaw's default for `before_agent_finalize` is 15 s; to change it, set
`plugins.entries.countercheck.hooks.timeouts.before_agent_finalize` (ms, up to 600000).

## What the agent and you see

| Outcome | OpenClaw does | In OpenClaw's log |
|---|---|---|
| VERIFIED | delivers the answer | `✓ Countercheck: VERIFIED, 1 check(s) passed in a clean copy (receipt …)` |
| UNVERIFIED | delivers the answer | `Countercheck: UNVERIFIED: <the first reason> (receipt …)` |
| FAILED, the agent claims success | one more model pass, with the failing check's output as the reason | OpenClaw's own `before_agent_finalize requested one more pass … attempt=1/3` |
| FAILED, the agent reports a blocker | delivers the answer | `⚠ Countercheck: FAILED (…); the agent says it is blocked, so it may stop …` |
| FAILED after Countercheck's push-back limit | delivers the answer | `⚠ Countercheck: FAILED (…). Stopped sending the agent back after the push-back limit: the work did NOT pass …` |

The rules are the same as for every agent ([Claude Code page](claude-code.md)). The claim on the receipt is the
event's `lastAssistantMessage`; receipts say `openclaw`, with the event's `model` and `sessionId`.

### Two limits: OpenClaw's and Countercheck's

OpenClaw's embedded runner allows **3** revisions per run; after that it skips `before_agent_finalize` altogether
(observed: `before_agent_finalize revision limit reached; skipping further finalize hooks … attempts=3/3`).
Countercheck's `COUNTERCHECK_MAX_NUDGES` is also 3, per session. At the defaults OpenClaw stops first, so the
answer is delivered without the ⚠ line (the last receipt still says FAILED). Set `COUNTERCHECK_MAX_NUDGES=2` in the
Gateway's environment to get the ⚠ line in the log instead (observed). The plugin sends no `retry` metadata, so
OpenClaw's per-instruction retry budget (one pass by default) doesn't apply.

## Environment

Set in the environment the Gateway (or `openclaw agent --local`) runs in; the plugin passes it on.

| Variable | Effect |
|---|---|
| `COUNTERCHECK_BASE` | Revision the checks come from. Default: `HEAD` (see "Not covered yet"). |
| `COUNTERCHECK_HOOK_DISABLED=1` | The hook does nothing. |
| `COUNTERCHECK_SANDBOX=none` | Run checks without Countercheck's sandbox. |
| `COUNTERCHECK_RECEIPTS` | Folder for receipts (default `.countercheck/receipts`). |
| `COUNTERCHECK_HOOK_LOG` | Append one JSON line per call to this file. |
| `COUNTERCHECK_MAX_NUDGES` | Push-backs allowed per session (default 3; 2 recommended with OpenClaw, above). |

## Fails open

Bad input, no `cwd` in the event (the plugin runs in the Gateway's folder, which is never taken for the project), a
folder that isn't a git repository, no check file: `countercheck` prints the error on stderr and answers nothing, and
the answer goes through. If the plugin itself throws or times out, OpenClaw logs it and finalizes normally.

## Not covered yet

- **Which runtimes:** the embedded runner (the default, `openclaw agent --local` and the Gateway) dispatches
  `before_agent_finalize` (observed). The docs say Codex native `Stop` hooks are relayed into it, and the **Copilot
  harness does not dispatch it**: under that harness the plugin never runs (docs; not tried).
- **Session start:** OpenClaw's `session_start` event has no `cwd`, so the checks lock to `HEAD` at each finalize,
  not to the commit the session started from. An agent that commits an edit to a test or `checks.toml` moves the
  rules with it; keep `countercheck verify --base origin/main` in CI.
- **Tamper check:** the plugin and its grant live outside the repository (`~/.openclaw/openclaw.json`), so
  `countercheck verify` can't see an agent switch them off. An agent with shell access could run
  `openclaw plugins disable countercheck`; CI is the check nobody can skip.
- **Real model:** run with OpenClaw 2026.10.9 (`openclaw agent --local`) against a stand-in OpenAI-compatible model
  that always answers "All tests pass. Done.": sent back 3 times on a failing check, then OpenClaw's cap; VERIFIED on
  the fixed project with one model call. Not yet with a real model or the Gateway's channels.

Sources: [Prompt and session hooks](https://docs.openclaw.ai/plugins/hooks/prompt-and-session) (`before_agent_finalize`),
[Plugin hooks](https://docs.openclaw.ai/plugins/hooks) (permissions), [hook reference](https://docs.openclaw.ai/plugins/hooks/reference)
(timeouts), [Plugins](https://docs.openclaw.ai/tools/plugin) (discovery and load paths),
[Hook event types](https://docs.openclaw.ai/automation/hooks/event-types); the `openclaw` npm package 2026.10.9.
