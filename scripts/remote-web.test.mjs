import test from 'node:test'
import assert from 'node:assert/strict'
import { mkdtemp, rm, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join, resolve } from 'node:path'
import { spawn } from 'node:child_process'
import { pause, stop } from './admin-fixture.mjs'

test('instance remote Web configuration survives saved addresses and protects browser entry', { timeout: 30000 }, async () => {
  const dir = await mkdtemp(join(tmpdir(), 'crabot-remote-web-'))
  const token = 'remote-web-test-credential-1234567890'
  const env = { ...process.env, CRABOT_DATA_DIR: dir, AGENT_WORKDIR: dir, ADMIN_AGENT_PROVIDER: 'mock', NODE_LINKS_JSON: '[]' }
  delete env.BIND_ADDR
  delete env.WEB_ACCESS_TOKEN
  delete env.CRABOT_LAUNCH_DIR
  delete env.CRABOT_CONFIG_DEFAULT_KEYS
  let server
  async function launch() {
    const child = spawn(resolve('target/debug/agent-node'), [], { cwd: dir, env, stdio: ['ignore', 'pipe', 'pipe'] })
    server = { child }
    let output = ''
    child.stdout.on('data', chunk => output += chunk)
    child.stderr.on('data', chunk => output += chunk)
    for (let i = 0; i < 240; i++) {
      const address = output.match(/Server: http:\/\/(\S+)/)?.[1]
      if (address) return address
      if (child.exitCode !== null) throw new Error(output)
      await pause(25)
    }
    throw new Error('Startup timeout: ' + output)
  }
  try {
    await writeFile(join(dir, '.agent.env'), 'BIND_ADDR=127.0.0.1:0\n', { mode: 0o600 })
    assert.match(await launch(), /^127\.0\.0\.1:/)
    await stop(server)
    await writeFile(join(dir, '.agent.env'), `BIND_ADDR=0.0.0.0:0\nWEB_ACCESS_TOKEN=${token}\n`)
    const address = await launch()
    assert.match(address, /^0\.0\.0\.0:/, 'instance config overrides the saved loopback listener')
    const url = `http://127.0.0.1:${address.split(':')[1]}`
    for (const path of ['/', '/v1/agent', '/v1/attachments/test/content']) {
      const response = await fetch(url + path)
      assert.equal(response.status, 401)
      assert.match(response.headers.get('www-authenticate'), /^Basic /)
    }
    const authorization = 'Basic ' + Buffer.from(`crabot:${token}`).toString('base64')
    assert.equal((await fetch(url + '/v1/agent', { headers: { authorization } })).status, 200)
    assert.equal((await fetch(url + '/v1/agent', { headers: { authorization, origin: 'https://evil.example' } })).status, 403)
    await stop(server)
    await writeFile(join(dir, '.agent.env'), 'BIND_ADDR=0.0.0.0:0\n')
    await assert.rejects(launch(), /Remote Web requires WEB_ACCESS_TOKEN/)
  } finally {
    await stop(server)
    await rm(dir, { recursive: true, force: true })
  }
})
