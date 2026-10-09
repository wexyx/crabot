import test from 'node:test'
import assert from 'node:assert/strict'
import { createAgentConnection, createSseDecoder, normalizeAgentUrl } from './agent-api.js'

test('Policy conflicts preserve the server error for the editor', async () => {
  const connection = createAgentConnection('https://agent.example', 'key', async () =>
    new Response('{"error":"version_conflict"}', {status:409}))
  await assert.rejects(connection.request('/v1/crabot/control'), /409.*version_conflict/)
  connection.close()
})

test('Agent URLs reject embedded credentials and non-HTTP protocols', () => {
  assert.equal(normalizeAgentUrl('https://agent.example/'), 'https://agent.example')
  for (const value of ['file:///tmp', 'https://user:secret@host', 'https://host/?token=x']) assert.throws(() => normalizeAgentUrl(value))
})

test('SSE preserves split Unicode, CRLF and multiline JSON', () => {
  const events = [], push = createSseDecoder(x => events.push(x))
  const bytes = new TextEncoder().encode(':ping\r\ndata: {"content":\r\ndata: "你好"}\r\n\r\n')
  for (const byte of bytes) push(Uint8Array.of(byte))
  assert.deepEqual(events, [{ content: '你好' }])
})

test('Switching Agent aborts pending work and does not leak credentials', async () => {
  let oldSignal, resolveOld
  const first = createAgentConnection('https://one.example', 'one-key', async (url, options) => {
    assert.equal(url, 'https://one.example/v1/agent'); assert.equal(options.headers.get('x-admin-token'), null)
    oldSignal = options.signal
    return new Promise(resolve => { resolveOld = resolve })
  })
  const pending = first.request('/v1/agent')
  first.close()
  assert.equal(oldSignal.aborted, true)
  resolveOld(new Response('{}', { headers: { 'content-type': 'application/json' } }))
  await assert.rejects(pending, { name: 'AbortError' })
  await assert.rejects(first.request('/v1/agent'), { name: 'AbortError' })
  const second = createAgentConnection('https://two.example', 'two-key', async (url, options) => {
    assert.equal(url, 'https://two.example/v1/agent'); assert.equal(options.headers.get('x-admin-token'), null)
    assert.equal(options.credentials, 'omit'); assert.equal(options.redirect, 'error')
    return new Response('{"id":"two"}')
  })
  assert.deepEqual(await second.request('/v1/agent'), { id: 'two' })
  second.close()
})

test('Closing a connection cancels its event stream', async () => {
  let signal, pushStream
  const connection = createAgentConnection('https://agent.example', 'key', async (_, options) => {
    signal = options.signal
    return new Response(new ReadableStream({ start(ctrl) { pushStream = ctrl } }), { headers: { 'content-type': 'text/event-stream' } })
  })
  const events = []
  await connection.stream('/v1/sessions/id/events', x => events.push(x), () => assert.fail('closed stream must not report errors'))
  connection.close(); assert.equal(signal.aborted, true)
  pushStream.enqueue(new TextEncoder().encode('data: {"content":"stale"}\n\n')); pushStream.close()
  await new Promise(resolve => setTimeout(resolve, 0))
  assert.deepEqual(events, [])
})
