// Tests for the herdr-attention mod (fork issue 157). `claude plugin test`.
import { expect, mock, test } from 'claude-code/testing'

type Run = { argv: string[] }

const HERDR_ENV = { HERDR_PANE_ID: 'w1:p1', HERDR_SOCKET_PATH: '/tmp/herdr.sock' }

// Stubs shared by the tests: a herdr pane in the environment, a recorder for
// every process the mod starts, a recorder for its log lines, and the engine
// beneath the mod (the permission decision, and an optional hold that keeps a
// tool call open). Every stub is registered before the test's first call on $.
function harness(on: any, env: Record<string, string> = HERDR_ENV, exitCode = 0) {
  const clock = mock.clock(on, { now: 1_790_000_000_000 })
  mock.env(on, env)
  const runs: Run[] = []
  const logs: string[] = []
  on('process.run', ($: any, e: any) => {
    runs.push({ argv: e.argv })
    return { value: { exitCode, stdout: '', stderr: exitCode ? 'refused' : '' } }
  })
  on('ui.log', ($: any, e: any) => {
    logs.push(e.text)
    return { value: undefined }
  })
  const engine = { decision: 'ask', hold: null as Promise<unknown> | null }
  on('session.start', () => ({ cwd: '/work' }))
  on('session.end', () => ({ sessionId: 'test-session' }))
  on('turn.complete', () => ({ text: '' }))
  on('tool.check', () => ({ decision: engine.decision }))
  on('tool.call', async () => {
    if (engine.hold) await engine.hold
    return { result: 'ok' }
  })
  return { clock, runs, logs, engine }
}

// The helper's arguments after `sh <script>`: socket, pane, kind|clear, id, seq, ttl.
const call = (run: Run) => ({
  socket: run.argv[2],
  pane: run.argv[3],
  kind: run.argv[4],
  id: run.argv[5],
  seq: Number(run.argv[6]),
  ttl: run.argv[7],
})
const kinds = (runs: Run[]) => runs.map((run) => call(run).kind)

const startSession = ($: any) => $.session.start({ surface: 'terminal', isInteractive: true, cwd: '/work' })
const endTurn = ($: any, extra: Record<string, unknown> = {}) =>
  $.turn.complete({ turnId: 't1', answer: '', durationMs: 5, isAborted: false, usage: null, ...extra })

test('a question is reported while AskUserQuestion is open and cleared when it settles', async ($, on) => {
  const { runs, clock, engine } = harness(on)
  let release: (v: unknown) => void = () => {}
  engine.hold = new Promise((resolve) => { release = resolve })
  const pending = $.tool.call({ tool: 'AskUserQuestion', tool_use_id: 'toolu_q1' })
  await clock.settle()

  expect(runs.length).toBe(1)
  expect(runs[0].argv[0]).toBe('sh')
  expect(runs[0].argv[1].endsWith('hooks/herdr-hint.sh')).toBe(true)
  expect(call(runs[0])).toMatchObject({ socket: '/tmp/herdr.sock', pane: 'w1:p1', kind: 'question', id: 'toolu_q1', ttl: '15000' })

  release(undefined)
  await pending
  await clock.settle()
  expect(kinds(runs)).toEqual(['question', 'clear'])
})

test('a permission prompt is reported from tool.check and cleared when the call settles', async ($, on) => {
  const { runs, clock } = harness(on)
  const decision = await $.tool.check({ tool: 'Bash', input: { command: 'rm x' }, tool_use_id: 'toolu_p1' })
  expect(decision.decision).toBe('ask')
  await clock.settle()
  expect(runs.length).toBe(1)
  expect(call(runs[0])).toMatchObject({ kind: 'permission', id: 'toolu_p1' })

  // A deny or an Esc settles the call at once.
  await $.tool.call({ tool: 'Bash', tool_use_id: 'toolu_p1' })
  await clock.settle()
  expect(kinds(runs)).toEqual(['permission', 'clear'])
})

test('an allowed tool call reports nothing', async ($, on) => {
  const { runs, clock, engine } = harness(on)
  engine.decision = 'allow'
  await $.tool.check({ tool: 'Bash', input: { command: 'ls' }, tool_use_id: 'toolu_a1' })
  await $.tool.call({ tool: 'Bash', tool_use_id: 'toolu_a1' })
  await clock.settle()
  expect(runs.length).toBe(0)
})

test('a turn or session ending with nothing open starts no process', async ($, on) => {
  const { runs, clock } = harness(on)
  await endTurn($)
  await $.session.end({ reason: 'other' })
  await clock.settle()
  expect(runs.length).toBe(0)
})

test('an interrupted turn and the session end clear what is open', async ($, on) => {
  const { runs, clock } = harness(on)
  await $.tool.check({ tool: 'Bash', input: {}, tool_use_id: 'toolu_p2' })
  await endTurn($, { isAborted: true })
  await clock.settle()
  expect(kinds(runs)).toEqual(['permission', 'clear'])

  await $.tool.check({ tool: 'Bash', input: {}, tool_use_id: 'toolu_p3' })
  await $.session.end({ reason: 'clear' })
  await clock.settle()
  expect(kinds(runs)).toEqual(['permission', 'clear', 'permission', 'clear'])
})

test('a subagent cannot mark a question, its permission prompt is reported, and its turn ending clears nothing', async ($, on) => {
  const { runs, clock } = harness(on)
  await $.tool.call({ tool: 'AskUserQuestion', tool_use_id: 'toolu_sq', agentId: 'agent-1' })
  await clock.settle()
  expect(runs.length).toBe(0)

  await $.tool.check({ tool: 'Bash', input: {}, tool_use_id: 'toolu_sp', agentId: 'agent-1' })
  await clock.settle()
  expect(kinds(runs)).toEqual(['permission'])

  await endTurn($, { agentId: 'agent-1' })
  await clock.settle()
  expect(kinds(runs)).toEqual(['permission'])
})

test('an open dialog is repeated every five seconds with a rising seq, and the first seq is the wall clock', async ($, on) => {
  const { runs, clock } = harness(on)
  await startSession($)
  await $.tool.check({ tool: 'Bash', input: {}, tool_use_id: 'toolu_h1' })
  await clock.settle()
  expect(runs.length).toBe(1)
  // Wall-clock microseconds plus a counter: the reload-proof sequence.
  expect(call(runs[0]).seq).toBe(1_790_000_000_000 * 1000 + 1)

  await clock.advance(5000)
  await clock.advance(5000)
  expect(runs.length).toBe(3)
  const seqs = runs.map((run) => call(run).seq)
  expect(seqs[1] > seqs[0] && seqs[2] > seqs[1]).toBe(true)
  expect(runs.every((run) => call(run).kind === 'permission' && call(run).id === 'toolu_h1')).toBe(true)
})

test('the heartbeat sends nothing while nothing is open', async ($, on) => {
  const { runs, clock } = harness(on)
  await startSession($)
  await clock.advance(60_000)
  expect(runs.length).toBe(0)
})

test('a dialog open for thirty minutes is released with one log line', async ($, on) => {
  const { runs, logs, clock } = harness(on)
  await startSession($)
  await $.tool.check({ tool: 'Bash', input: {}, tool_use_id: 'toolu_long' })
  await clock.advance(29 * 60 * 1000)
  expect(logs.length).toBe(0)
  expect(call(runs[runs.length - 1]).kind).toBe('permission')

  await clock.advance(2 * 60 * 1000)
  expect(logs.length).toBe(1)
  expect(logs[0]).toContain('30 minutes')
  expect(kinds(runs)[runs.length - 1]).toBe('clear')
  const count = runs.length
  await clock.advance(60_000)
  expect(runs.length).toBe(count)
})

test('outside a herdr pane the mod starts nothing', async ($, on) => {
  const { runs, clock } = harness(on, {})
  await $.tool.check({ tool: 'Bash', input: {}, tool_use_id: 'toolu_x' })
  await clock.settle()
  expect(runs.length).toBe(0)
})

test('a failing report is logged once a minute, not every time', async ($, on) => {
  const { runs, logs, clock } = harness(on, HERDR_ENV, 1)
  await startSession($)
  await $.tool.check({ tool: 'Bash', input: {}, tool_use_id: 'toolu_f' })
  await clock.advance(5000)
  await clock.advance(5000)
  expect(runs.length).toBe(3)
  expect(logs.length).toBe(1)
  expect(logs[0]).toContain('herdr hint failed')

  await clock.advance(60_000)
  expect(logs.length).toBe(2)
})
