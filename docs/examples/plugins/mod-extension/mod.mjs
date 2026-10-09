// Plain ESM runs under the reviewed host without a compiler or package install.
// The public author SDK is not published; these are the documented host shims.
export const name = 'mod-extension'
export const inject = ['tools', 'commands', 'prompt', 'storage']

export function apply(ctx) {
  const disposePrompt = ctx.prompt.registerSection({
    id: 'counter-example',
    text: 'When the user explicitly tests mod_counter, report its returned count and label. Do not increment this example counter for unrelated tasks.',
  })

  // This listener touches only this example's own tool. It cannot approve it.
  const disposeHook = ctx.on('tools/pre-execute', async (exec, next) => {
    if (exec.name !== 'mod_counter') return next()
    if (exec.arguments.label === 'blocked') {
      return { kind: 'deny', reason: 'The example reserves the label "blocked" to demonstrate admission refusal.' }
    }
    return { kind: 'annotate', text: 'This counter is owner-local example state, not session history.' }
  })

  // Serialize this example's read/modify/write sequence within its owner.
  // Storage itself is last-writer-wins across separate host processes.
  let counterTail = Promise.resolve()
  const readCount = async () => {
    const value = await ctx.storage.get('count')
    if (value === undefined) return 0
    if (!Number.isSafeInteger(value) || value < 0) throw new Error('Stored example count must be a non-negative safe integer.')
    return value
  }
  const disposeTool = ctx.tools.register({
    name: 'mod_counter',
    description: 'Explicitly test the mod counter. Writes only its owner-local state; no network or workspace files.',
    parameters: {
      type: 'object',
      properties: {
        label: { type: 'string', maxLength: 64 },
        increment: { type: 'boolean' },
      },
      additionalProperties: false,
    },
    execute(input, exec) {
      const run = counterTail.then(async () => {
        exec.signal.throwIfAborted()
        let count = await readCount()
        if (input.increment !== false) {
          if (count === Number.MAX_SAFE_INTEGER) throw new Error('The example counter is full.')
          count += 1
          exec.signal.throwIfAborted()
          await ctx.storage.set('count', count)
        }
        exec.signal.throwIfAborted()
        // The host preserves this JSON as the tool's structured result.
        return {
          count,
          label: input.label ?? 'example',
          callId: exec.callId,
          sessionId: exec.sessionId ?? null,
          agentId: exec.agentId ?? null,
          originTurnId: exec.originTurnId ?? null,
        }
      })
      counterTail = run.then(() => undefined, () => undefined)
      return run
    },
  })

  const disposeCommand = ctx.commands.register({
    name: 'mod-count',
    description: 'Show the saved example count without incrementing it.',
    async handler({ signal }) {
      signal.throwIfAborted()
      await counterTail
      return { kind: 'success', text: `Saved example count: ${await readCount()}` }
    },
  })

  // Each registration is already fiber-owned. Explicit undo is idempotent;
  // the async cleanup also waits for the example's queued work to settle.
  ctx.effect(() => async () => {
    disposeCommand()
    disposeTool()
    disposeHook()
    disposePrompt()
    await counterTail
  })
}
