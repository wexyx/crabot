import test from 'node:test'
import assert from 'node:assert/strict'
import {mkdtemp, mkdir, writeFile, readFile, cp, rm, lstat} from 'node:fs/promises'
import {tmpdir} from 'node:os'
import {join, resolve} from 'node:path'
import {execFileSync, spawnSync} from 'node:child_process'

test('Linux bundle carries dereferenced C++ runtimes and license notices', async () => {
  const root = await mkdtemp(join(tmpdir(), 'crabot-linux-package-'))
  try {
    for (const dir of ['scripts/ci', 'scripts/release', 'apps/web/dist', 'conf', 'skills/system', 'docs', 'tools', 'runtime', 'unpacked']) {
      await mkdir(join(root, dir), {recursive: true})
    }
    for (const file of ['scripts/package-release.sh', 'scripts/ci/check-linux-libraries.sh', 'scripts/release/crabot']) {
      await cp(resolve(file), join(root, file))
    }
    for (const file of ['README.md', 'LICENSE', 'CONTRIBUTING.md', 'Cargo.lock', 'install.sh']) {
      await cp(resolve(file), join(root, file))
    }
    await writeFile(join(root, 'apps/web/dist/index.html'), 'fixture')
    await writeFile(join(root, 'agent-node'), '#!/bin/sh\necho "Crabot v0.0.0"\n', {mode: 0o755})
    await writeFile(join(root, 'fts'), 'fixture FTS')
    for (const library of ['libstdc++.so.6', 'libgcc_s.so.1']) {
      await writeFile(join(root, 'runtime', library), `fixture ${library}`)
    }
    await writeFile(join(root, 'tools/g++'), '#!/bin/sh\nif [ "$1" = -dumpfullversion ]; then echo 13.5.0; else printf "%s/%s\\n" "$FIXTURE_RUNTIME" "${1#-print-file-name=}"; fi\n', {mode: 0o755})
    await writeFile(join(root, 'tools/ldd'), '#!/bin/sh\necho "libssl.so.3 => /lib/libssl.so.3"\n', {mode: 0o755})
    await writeFile(join(root, 'tools/readelf'), '#!/bin/sh\necho "GLIBC_2.36"\n', {mode: 0o755})
    await writeFile(join(root, 'tools/curl'), `#!${process.execPath}
const fs = require('node:fs'), args = process.argv.slice(2);
const url = args.find(v => v.startsWith('https://'));
if (!url.startsWith('https://raw.githubusercontent.com/gcc-mirror/gcc/releases/gcc-13.5.0/')) process.exit(1);
fs.writeFileSync(args[args.indexOf('-o') + 1], 'license fixture');
`, {mode: 0o755})
    const result = spawnSync('bash', [join(root, 'scripts/package-release.sh'), 'x86_64-unknown-linux-gnu', join(root, 'agent-node')], {
      encoding: 'utf8', env: {...process.env, PATH: `${root}/tools:${process.env.PATH}`, CXX: 'g++',
        FIXTURE_RUNTIME: join(root, 'runtime'), CRABOT_LBUG_FTS_EXTENSION: join(root, 'fts'), CRABOT_RELEASE_VERSION: 'v0.0.0'},
    })
    assert.equal(result.status, 0, result.stderr)
    execFileSync('tar', ['-xzf', join(root, 'dist/crabot-x86_64-unknown-linux-gnu.tar.gz'), '-C', join(root, 'unpacked')])
    for (const library of ['libstdc++.so.6', 'libgcc_s.so.1']) {
      const path = join(root, 'unpacked/crabot/lib', library)
      assert.equal((await lstat(path)).isSymbolicLink(), false)
      assert.equal(await readFile(path, 'utf8'), `fixture ${library}`)
    }
    for (const name of ['COPYING3', 'COPYING.RUNTIME']) {
      assert.equal(await readFile(join(root, 'unpacked/crabot/licenses/gcc', name), 'utf8'), 'license fixture')
    }
  } finally {
    await rm(root, {recursive: true, force: true})
  }
})
