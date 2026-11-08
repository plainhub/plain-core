import assert from 'node:assert/strict'
import { execFileSync } from 'node:child_process'
import { cpSync, existsSync, mkdirSync, mkdtempSync, readFileSync, realpathSync, rmSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { pathToFileURL } from 'node:url'
import { test } from 'node:test'
import { cargo, configure, resolvedPatches, updateCurrent } from '../../scripts/core-dependencies.mjs'

function packageAt(path, name) {
  mkdirSync(join(path, 'src'), { recursive: true })
  writeFileSync(join(path, 'src/lib.rs'), '')
  writeFileSync(join(path, 'Cargo.toml'), `[package]\nname = "${name}"\nversion = "0.1.0"\nedition = "2024"\n`)
}

function fixture(t, layout = 'desktop') {
  const root = realpathSync(mkdtempSync(join(tmpdir(), 'plain-core-switch-')))
  t.after(() => rmSync(root, { recursive: true }))
  const beforeCargoHome = process.env.CARGO_HOME
  process.env.CARGO_HOME = join(root, 'cargo-home')
  t.after(() => {
    if (beforeCargoHome === undefined) delete process.env.CARGO_HOME
    else process.env.CARGO_HOME = beforeCargoHome
  })
  const origin = join(root, 'origin')
  const packages = ['plain-libs', 'plain-rs', 'plain-server']
  for (const name of packages) packageAt(join(origin, name), name)
  writeFileSync(join(origin, 'Cargo.toml'), `[workspace]\nresolver = "3"\nmembers = ${JSON.stringify(packages)}\n`)
  const core = join(root, 'local core')
  cpSync(origin, core, { recursive: true })
  const git = (...args) => execFileSync('git', ['-c', `core.hooksPath=${join(root, 'fixture-hooks')}`, ...args], {
    cwd: origin,
    encoding: 'utf8',
  }).trim()
  git('init', '-b', 'main')
  git('add', '.')
  git('-c', 'user.name=Fixture', '-c', 'user.email=fixture', 'commit', '-qm', 'fixture')

  const consumer = join(root, 'consumer')
  const crate = layout === 'app' ? join(consumer, 'plain-rs') : consumer
  packageAt(crate, 'consumer')
  const manifest = join(crate, 'Cargo.toml')
  const url = pathToFileURL(origin).href
  const dependencies = packages.map(name => `${name} = { git = "${url}", branch = "main" }\n`).join('')
  writeFileSync(manifest, readFileSync(manifest, 'utf8') + '\n[dependencies]\n' + dependencies)
  const config = join(consumer, '.cargo/config.toml')
  mkdirSync(join(consumer, '.cargo'))
  const original = '[net]\nretry = 0\n# preserve this configuration\n'
  writeFileSync(config, original)
  cargo(consumer, manifest, 'metadata', '--format-version', '1')
  const lock = join(crate, 'Cargo.lock')
  const state = join(consumer, '.cargo/core-dependency-state')
  const record = join(state, 'state.json')
  const switchTo = mode => configure(consumer, manifest, core, mode, url)
  const resolutions = () => resolvedPatches(cargo(consumer, manifest, 'metadata', '--locked', '--format-version', '1'), url)
  return { config, consumer, core, git, lock, manifest, origin, original, packages, record, resolutions, state, switchTo, url }
}

for (const layout of ['desktop', 'app']) {
  test(`switches ${layout} to local Core and restores the original lock and config`, t => {
    const f = fixture(t, layout)
    const pinned = readFileSync(f.lock)
    f.switchTo('local')
    for (const name of f.packages) assert.equal(f.resolutions().get(name), join(f.core, name, 'Cargo.toml'))

    // Refresh a state record made before plain-libs was listed in the override.
    const record = JSON.parse(readFileSync(f.record))
    const oldPatch = record.patch.split('\n').filter(line => !line.startsWith('plain-libs =')).join('\n')
    const suffix = '\n[term]\ncolor = "never"\n'
    writeFileSync(f.record, JSON.stringify({ ...record, patch: oldPatch }))
    writeFileSync(f.config, f.original + oldPatch + suffix)
    f.switchTo('local')
    assert.equal(readFileSync(f.config, 'utf8'), f.original + record.patch + suffix)

    f.switchTo('pinned')
    assert.equal(readFileSync(f.config, 'utf8'), f.original + suffix)
    assert.deepEqual(readFileSync(f.lock), pinned)
    assert.equal(existsSync(f.state), false)
  })
}

test('rolls back a failed local resolution and a failed pinned update', t => {
  const f = fixture(t)
  f.switchTo('local')
  const paths = [f.config, f.lock, f.record, join(f.state, 'Cargo.lock.pinned')]
  const before = paths.map(path => readFileSync(path))
  writeFileSync(join(f.core, 'plain-server', 'Cargo.toml'), '[invalid TOML')
  assert.throws(() => f.switchTo('local'))
  assert.deepEqual(paths.map(path => readFileSync(path)), before)

  writeFileSync(join(f.origin, 'plain-server', 'Cargo.toml'), '[invalid TOML')
  f.git('add', '.')
  f.git('-c', 'user.name=Fixture', '-c', 'user.email=fixture', 'commit', '-qm', 'broken')
  assert.throws(() => updateCurrent(f.consumer, f.manifest, f.core, f.url))
  assert.deepEqual(paths.map(path => readFileSync(path)), before)
})

test('updates the pinned Core to the exact new revision', t => {
  const f = fixture(t)
  f.switchTo('local')
  f.git('-c', 'user.name=Fixture', '-c', 'user.email=fixture', 'commit', '--allow-empty', '-qm', 'new revision')
  const revision = f.git('rev-parse', 'HEAD')
  assert.equal(updateCurrent(f.consumer, f.manifest, f.core, f.url), revision)
  assert.ok(readFileSync(f.lock, 'utf8').includes(`#${revision}`))
  assert.equal(readFileSync(f.config, 'utf8'), f.original)
  assert.equal(existsSync(f.state), false)
})
