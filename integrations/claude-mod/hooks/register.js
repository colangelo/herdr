// Fork issue 137 part C spike: report a Claude Code question to herdr.
//
// A mod wraps the AskUserQuestion tool call as middleware, so it sees the
// question OPEN before `next(e)` and CLOSED when `next(e)` settles, however
// the dialog ends: answered, Esc, cancel or /clear. No screen reading, no
// hook that fails to fire after Esc.
//
// The report is a display-only pane token with a TTL as a backstop. Herdr
// itself decides what a token means.

const SOURCE = 'herdr:claude-mod'
const TOKEN = 'question'
const TTL_MS = 10 * 60 * 1000

// Reports in order, even when two arrive close together.
let seq = 0

async function report($, args, why) {
  const pane = await $.env.get('HERDR_PANE_ID')
  if (!pane) return
  const bin = (await $.env.get('HERDR_MOD_BIN')) || 'herdr-beta'
  seq += 1
  try {
    if (why) $.ui.log('question closed: ' + why)
    const done = await $.process.run([bin, 'pane', 'report-metadata', pane, '--source', SOURCE, '--agent', 'claude', '--seq', String(Date.now() * 1000 + (seq % 1000)), ...args])
    if (done.exitCode !== 0) $.ui.log('herdr report failed (' + done.exitCode + '): ' + (done.stderr || done.stdout || '').slice(0, 200))
  } catch (err) {
    // herdr is not there or refused: the question still goes on.
    $.ui.log('herdr report threw: ' + String(err).slice(0, 200))
  }
}

const open = ($, id) => report($, ['--token', TOKEN + '=open:' + id, '--ttl-ms', String(TTL_MS)])
const close = ($, why) => report($, ['--clear-token', TOKEN], why)

export function register(on) {
  on('tool.call', { tool: 'AskUserQuestion' }, async ($, e, next) => {
    // A subagent's question must not mark the parent pane.
    if (e.agentId) return next(e)
    await open($, e.tool_use_id ?? '')
    // Esc abandons the call without settling `next`, so a `finally` alone is
    // not enough: the abort signal is the other way this question closes.
    const onAbort = () => { void close($, 'abort') }
    next.signal.addEventListener('abort', onAbort, { once: true })
    try {
      return await next(e)
    } finally {
      next.signal.removeEventListener('abort', onAbort)
      await close($, 'settled')
    }
  })

  // An interrupted turn ends with no answer to the question.
  on('turn.complete', async ($, e, next) => {
    if (!e.agentId) await close($, 'turn.complete')
    return next(e)
  })

  // /clear, /resume and exit end the session with no answer to the question.
  on('session.end', async ($, e, next) => {
    await close($, 'session.end')
    return next(e)
  })
}
