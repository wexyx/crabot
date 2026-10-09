import test from 'node:test'
import assert from 'node:assert/strict'
import {mkdtemp, mkdir, writeFile, readFile, rm, readdir} from 'node:fs/promises'
import {tmpdir} from 'node:os'
import {join, resolve} from 'node:path'
import {execFileSync, spawnSync} from 'node:child_process'

const prepare = resolve('scripts/ci/prepare-linux-lbug.sh')
async function fixture(target, {version = '0.21.0', missing, downloadFailure = false} = {}) {
  const root = await mkdtemp(join(tmpdir(), 'crabot-prebuilt-'))
  try {
    const bin = join(root, 'bin'), source = join(root, 'source'), destination = join(root, 'output path')
    await mkdir(bin); await mkdir(source)
    for (const file of ['liblbug.a', 'lbug.h', 'lbug.hpp']) {
      if (file !== missing) await writeFile(join(source, file), `fixture ${file}`)
    }
    const archive = join(root, 'fixture.tar.gz'), log = join(root, 'url')
    execFileSync('tar', ['-czf', archive, '-C', source, '.'])
    await writeFile(join(bin, 'curl'), `#!${process.execPath}
const fs = require('node:fs'), args = process.argv.slice(2);
fs.writeFileSync(process.env.URL_LOG, args.find(arg => arg.startsWith('https://')));
if (process.env.DOWNLOAD_FAILURE === '1') process.exit(22);
fs.copyFileSync(process.env.ARCHIVE, args[args.indexOf('-o') + 1]);
`, {mode: 0o755})
    const result = spawnSync('bash', [prepare, target, version, destination], {
      encoding: 'utf8', env: {...process.env, PATH: `${bin}:${process.env.PATH}`,
        ARCHIVE: archive, URL_LOG: log, DOWNLOAD_FAILURE: downloadFailure ? '1' : '0'},
    })
    return {result, url: await readFile(log, 'utf8').catch(() => ''), files: await readdir(destination).catch(() => [])}
  } finally {
    await rm(root, {recursive: true, force: true})
  }
}

for (const arch of ['x86_64', 'aarch64']) {
  test(`prebuilt download pins version and compat variant for ${arch}`, async () => {
    const {result, url, files} = await fixture(`${arch}-unknown-linux-gnu`)
    assert.equal(result.status, 0, result.stderr)
    assert.equal(url, `https://github.com/LadybugDB/ladybug/releases/download/v0.21.0/liblbug-static-linux-${arch}-compat.tar.gz`)
    assert.deepEqual(files.sort(), ['liblbug.a', 'lbug.h', 'lbug.hpp'].sort())
  })
}

for (const missing of ['liblbug.a', 'lbug.h', 'lbug.hpp']) {
  test(`missing ${missing} fails before copying any prebuilt files`, async () => {
    const {result, files} = await fixture('x86_64-unknown-linux-gnu', {missing})
    assert.notEqual(result.status, 0)
    assert.match(result.stderr, /Missing Ladybug prebuilt file/)
    assert.deepEqual(files, [])
  })
}

test('download failure does not fall back to source compilation', async () => {
  const {result, files} = await fixture('x86_64-unknown-linux-gnu', {downloadFailure: true})
  assert.notEqual(result.status, 0)
  assert.deepEqual(files, [])
})

for (const [target, version] of [['unknown', '0.21.0'], ['x86_64-unknown-linux-gnu', '../latest']]) {
  test(`invalid prebuilt request fails without network: ${target} ${version}`, async () => {
    const {result, url} = await fixture(target, {version})
    assert.notEqual(result.status, 0)
    assert.equal(url, '')
  })
}
