// Test driver for the Amp plugin (see README.md): a stub of Amp's API, one agent.end event.
const [pluginPath, root, message, status] = process.argv.slice(2)
const handlers: Record<string, Function> = {}
const logs: string[] = []
const amp: any = {
	on: (e: string, h: Function) => (handlers[e] = h),
	logger: { log: (...a: unknown[]) => logs.push(a.join(' ')) },
	system: { workspaceRoot: root === 'null' ? null : { toString: () => 'file://' + root } },
	helpers: { filePathFromURI: (u: any) => String(u).replace('file://', '') },
}
const mod = await import(pluginPath)
mod.default(amp)
const notes: string[] = []
const ctx = { ui: { notify: async (m: string) => notes.push(m) } }
const result = await handlers['agent.end'](
	{ thread: { id: process.env.THREAD || 'T-0001' }, message: 'fix calc', id: 1, status: status || 'done', messages: [
		{ role: 'user', id: 1, content: [{ type: 'text', text: 'fix calc' }] },
		{ role: 'assistant', id: 2, stopReason: 'end_turn', usage: { model: 'claude-test-1' }, content: [{ type: 'text', text: message }] },
	] },
	ctx,
)
console.log(JSON.stringify({ result, notes, logs: logs.map((l) => l.slice(0, 200)) }, null, 1))
