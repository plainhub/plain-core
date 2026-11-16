import assert from 'node:assert/strict'
import { spawnSync } from 'node:child_process'
import { copyFileSync, mkdirSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { dirname, join } from 'node:path'
import test from 'node:test'

const script = new URL('../../scripts/gen-app-features.mjs', import.meta.url)
const desktop = 'src/lib/app-feature.generated.ts'
const kotlin = 'shared/src/commonMain/kotlin/com/ismartcoding/plain/platform/system/AppFeatureType.kt'
const header = '// Enum values generated from plain-core AppFeatureType; do not edit.\n'

function fixture(t) {
  const root = mkdtempSync(join(tmpdir(), 'plain-app-features-'))
  t.after(() => rmSync(root, { recursive: true, force: true }))

  function put(file, content) {
    const path = join(root, file)
    mkdirSync(dirname(path), { recursive: true })
    writeFileSync(path, content)
  }

  put('plain-core/plain-rs/src/features/app_feature.rs', 'pub enum AppFeatureType {\n    Docs,\n    ScreenMirror,\n}\n')
  const cli = join(root, 'plain-core/scripts/gen-app-features.mjs')
  mkdirSync(dirname(cli), { recursive: true })
  copyFileSync(script, cli)
  return {
    root,
    put,
    run: (...args) => spawnSync(process.execPath, [cli, ...args], { encoding: 'utf8' }),
  }
}

test('checks Desktop in a Core + Desktop checkout and rejects stale TypeScript', t => {
  const { put, run } = fixture(t)
  const file = `plain-desktop/${desktop}`
  const expected = header + "export enum AppFeatureType {\n  DOCS = 'DOCS',\n  SCREEN_MIRROR = 'SCREEN_MIRROR',\n}\n"
  put(file, expected)

  const current = run('--target=desktop')
  assert.equal(current.status, 0, current.stderr)
  assert.notEqual(run().status, 0, 'default checking must still require both clients')

  put(file, expected.replace('SCREEN_MIRROR', 'SCREEN'))
  const stale = run('--target=desktop')
  assert.notEqual(stale.status, 0)
  assert.match(stale.stderr, /Stale generated AppFeatureType/)

  const refreshed = run('--target=desktop', '--write')
  assert.equal(refreshed.status, 0, refreshed.stderr)
  assert.equal(run('--target=desktop').status, 0)
})

test('checks Kotlin from its explicit checkout without Desktop and rejects a stale enum', t => {
  const { put, root, run } = fixture(t)
  const file = `app checkout/${kotlin}`
  const expected = header + 'package example\n\nenum class AppFeatureType {\n    DOCS,\n    SCREEN_MIRROR;\n}\n'
  const args = ['--target=android', `--consumer-root=${join(root, 'app checkout')}`]
  put(file, expected)
  const current = run(...args)
  assert.equal(current.status, 0, current.stderr)

  put(file, expected.replace('SCREEN_MIRROR', 'SCREEN'))
  const stale = run(...args)
  assert.notEqual(stale.status, 0)
  assert.match(stale.stderr, /Stale generated AppFeatureType/)
})
