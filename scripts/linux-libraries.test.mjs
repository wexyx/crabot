import test from 'node:test'
import assert from 'node:assert/strict'
import {mkdtemp, writeFile, rm} from 'node:fs/promises'
import {tmpdir} from 'node:os'
import {join, resolve} from 'node:path'
import {spawnSync} from 'node:child_process'

const checker = resolve('scripts/ci/check-linux-libraries.sh')

async function inspect(output, status = 0, versions = 'GLIBC_2.36 GLIBCXX_3.4.32 CXXABI_1.3.14') {
  const dir = await mkdtemp(join(tmpdir(), 'crabot-link-check-'))
  try {
    await writeFile(join(dir, 'ldd'), '#!/bin/sh\nprintf "%s\\n" "$LDD_OUTPUT"\nexit "$LDD_STATUS"\n', {mode: 0o755})
    await writeFile(join(dir, 'readelf'), '#!/bin/sh\nprintf "%s\\n" "$ELF_VERSIONS"\n', {mode: 0o755})
    return spawnSync('bash', [checker, '/fixture/agent-node'], {
      encoding: 'utf8',
      env: {...process.env, PATH: `${dir}:${process.env.PATH}`, LDD_OUTPUT: output, LDD_STATUS: String(status), ELF_VERSIONS: versions},
    })
  } finally {
    await rm(dir, {recursive: true, force: true})
  }
}

test('Linux release accepts resolved base system libraries', async () => {
  const result = await inspect('libc.so.6 => /lib/libc.so.6\nlibstdc++.so.6 => /lib/libstdc++.so.6')
  assert.equal(result.status, 0, result.stderr)
})

for (const library of ['libssl.so.3', 'libcrypto.so.3']) {
  test(`Linux release accepts resolved system ${library}`, async () => {
    const result = await inspect(`${library} => /lib/${library}`)
    assert.equal(result.status, 0, result.stderr)
  })
}

for (const library of ['libssl.so.1.1', 'libcrypto.so.1.0', 'libssl.so.4']) {
  test(`Linux release rejects ${library} even when installed on CI`, async () => {
    const result = await inspect(`${library} => /lib/${library}`)
    assert.notEqual(result.status, 0)
    assert.match(result.stderr, /must use system OpenSSL 3/)
  })
}

test('Linux release rejects unresolved libraries', async () => {
  const result = await inspect('libstdc++.so.6 => not found')
  assert.notEqual(result.status, 0)
  assert.match(result.stderr, /unresolved shared libraries/)
})

test('Linux release fails closed if ldd fails', async () => {
  const result = await inspect('not a dynamic executable', 1)
  assert.notEqual(result.status, 0)
})

for (const version of ['GLIBC_2.37', 'GLIBC_2.38', 'GLIBC_2.40']) {
  test(`Linux release rejects runtime requirement ${version}`, async () => {
    const result = await inspect('libc.so.6 => /lib/libc.so.6', 0, version)
    assert.notEqual(result.status, 0)
    assert.match(result.stderr, /exceeding Debian 12 baseline/)
  })
}

test('Linux release accepts older symbols using numeric version comparison', async () => {
  const result = await inspect('libc.so.6 => /lib/libc.so.6', 0, 'GLIBC_2.9 GLIBC_2.17 GLIBCXX_3.4.9 CXXABI_1.3.9')
  assert.equal(result.status, 0, result.stderr)
})
