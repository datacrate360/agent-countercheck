// Countercheck for Amp, written by `countercheck hook install --agent amp` (docs/hooks/amp.md).
// When the agent ends a turn, this runs `countercheck hook amp-stop` with the turn on stdin; while the
// project's locked checks fail, it sends the agent back with the reason. Any error lets Amp stop.
// Don't edit it: Countercheck compares this file with the committed one on every run.
import type { PluginAPI, ThreadMessage } from '@ampcode/plugin'
import { spawn } from 'node:child_process'

export const description =
	'Countercheck: re-runs the locked checks when the agent says it is done, and sends it back while they fail.'

const BIN = "__COUNTERCHECK_BIN__"
// Longer than the checks take; past it, Amp may stop.
const TIMEOUT_MS = 15 * 60 * 1000

// `BIN hook <sub>` with `input` as JSON on stdin (never in a command line); its stdout, or '' on any error.
function run(amp: PluginAPI, sub: string, input: unknown, cwd: string): Promise<string> {
	return new Promise((resolve) => {
		try {
			const child = spawn(BIN, ['hook', sub], { cwd, stdio: ['pipe', 'pipe', 'pipe'], windowsHide: true })
			let out = ''
			let err = ''
			const timer = setTimeout(() => child.kill(), TIMEOUT_MS)
			child.stdout?.on('data', (d) => (out += d))
			child.stderr?.on('data', (d) => (err += d))
			child.on('error', (e) => {
				clearTimeout(timer)
				amp.logger.log(`countercheck: ${e}`)
				resolve('')
			})
			child.on('close', (code) => {
				clearTimeout(timer)
				if (err.trim()) amp.logger.log(`countercheck: ${err.trim().slice(-2000)}`)
				resolve(code === 0 ? out : '')
			})
			child.stdin?.on('error', () => {})
			child.stdin?.end(JSON.stringify(input))
		} catch (e) {
			amp.logger.log(`countercheck: ${e}`)
			resolve('')
		}
	})
}

// The project: the workspace Amp has open, else the folder the plugin runs in (inside the project).
function projectDir(amp: PluginAPI): string {
	try {
		const root = amp.system.workspaceRoot
		if (root) return amp.helpers.filePathFromURI(root)
	} catch {}
	return process.cwd()
}

// The agent's last words in this turn, and the model that wrote them.
function finalMessage(messages: ThreadMessage[]): { text: string | null; model: string | null } {
	for (let i = messages.length - 1; i >= 0; i--) {
		const m = messages[i]
		if (m.role !== 'assistant') continue
		const text = m.content
			.flatMap((b) => (b.type === 'text' ? [b.text] : []))
			.join('\n')
			.trim()
		// `usage` is in the API docs but not yet in every version of the types.
		const usage = (m as { usage?: { model?: string } }).usage
		if (text) return { text, model: usage?.model ?? null }
	}
	return { text: null, model: null }
}

export default function (amp: PluginAPI) {
	// Locks the checks to the commit the thread started from.
	amp.on('session.start', async (event) => {
		const cwd = projectDir(amp)
		await run(amp, 'session-start', { hook_event_name: 'session.start', session_id: event.thread.id, cwd }, cwd)
	})

	amp.on('agent.end', async (event, ctx) => {
		try {
			const cwd = projectDir(amp)
			const { text, model } = finalMessage(event.messages)
			const input = { cwd, thread_id: event.thread.id, status: event.status, model, last_assistant_message: text }
			const out = (await run(amp, 'amp-stop', input, cwd)).trim()
			if (!out) return
			const answer = JSON.parse(out)
			if (answer.action === 'continue' && typeof answer.userMessage === 'string') {
				return { action: 'continue', userMessage: answer.userMessage }
			}
			if (typeof answer.message === 'string') await ctx.ui.notify(answer.message)
		} catch (e) {
			amp.logger.log(`countercheck: ${e}`)
		}
	})
}
