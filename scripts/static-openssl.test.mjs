import test from 'node:test'
import assert from 'node:assert/strict'
import {mkdtemp, writeFile, readFile, realpath, rm} from 'node:fs/promises'
import {tmpdir} from 'node:os'
import {join, resolve} from 'node:path'
import {spawnSync} from 'node:child_process'

const builder = resolve('scripts/ci/build-static-openssl.sh')
async function fixture(target, overrides = {}) {
  const root = await realpath(await mkdtemp(join(tmpdir(), 'crabot-openssl-build-')))
  try {
    const prefix = join(root, 'install path'), configure = join(root, 'Configure'), log = join(root, 'calls')
    await writeFile(configure, '# fixture\n')
    for (const command of ['perl', 'make']) {
      await writeFile(join(root, command), `#!${process.execPath}
const fs = require('node:fs'), path = require('node:path');
const command = path.basename(process.argv[1]), args = process.argv.slice(2);
fs.appendFileSync(process.env.FIXTURE_LOG, JSON.stringify({command, args, cwd: process.cwd()}) + '\\n');
if (command === 'perl' && process.env.FAIL_CONFIGURE) process.exit(1);
if (command === 'make' && args[0] === 'install_dev' && !process.env.MISSING_ARCHIVE) {
  for (const file of ['include/openssl/ssl.h', 'lib/libssl.a', 'lib/libcrypto.a']) {
    const output = path.join(process.env.FIXTURE_PREFIX, file);
    fs.mkdirSync(path.dirname(output), {recursive: true});
    fs.writeFileSync(output, 'fixture');
  }
}
`, {mode: 0o755})
    }
    const result = spawnSync('bash', [builder, target, configure, prefix], {
      encoding: 'utf8', env: {...process.env, PATH: `${root}:${process.env.PATH}`,
        OPENSSL_SRC_PERL: join(root, 'perl'), CARGO_BUILD_JOBS: '2',
        FIXTURE_LOG: log, FIXTURE_PREFIX: prefix, ...overrides},
    })
    const calls = (await readFile(log, 'utf8').catch(() => '')).trim().split('\n').filter(Boolean).map(JSON.parse)
    return {result, calls, prefix, configure}
  } finally {
    await rm(root, {recursive: true, force: true})
  }
}

for (const [target, platform] of [['x86_64-unknown-linux-gnu', 'linux-x86_64'], ['aarch64-unknown-linux-gnu', 'linux-aarch64']]) {
  test(`shared static OpenSSL build for ${target} uses only library targets`, async () => {
    const {result, calls, prefix, configure} = await fixture(target)
    assert.equal(result.status, 0, result.stderr)
    assert.deepEqual(calls[0].args.slice(0, 3), [configure, platform, `--prefix=${prefix}`])
    for (const arg of ['no-shared', 'no-module', '--libdir=lib', '-fPIC']) assert.ok(calls[0].args.includes(arg))
    assert.ok(calls.every(call => call.cwd === `${prefix}-build`))
    assert.deepEqual(calls.slice(1).map(call => call.args), [['depend'], ['-j', '2', 'build_libs'], ['install_dev']])
  })
}

test('unsupported target cannot start a native build', async () => {
  const {result, calls} = await fixture('unknown-target')
  assert.notEqual(result.status, 0)
  assert.equal(calls.length, 0)
})

test('Configure failure prevents make and installation', async () => {
  const {result, calls} = await fixture('x86_64-unknown-linux-gnu', {FAIL_CONFIGURE: '1'})
  assert.notEqual(result.status, 0)
  assert.deepEqual(calls.map(call => call.command), ['perl'])
})

test('successful make without installed headers and archives is rejected', async () => {
  const {result} = await fixture('x86_64-unknown-linux-gnu', {MISSING_ARCHIVE: '1'})
  assert.notEqual(result.status, 0)
  assert.match(result.stderr, /Missing OpenSSL build output/)
})
