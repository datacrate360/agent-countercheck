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
