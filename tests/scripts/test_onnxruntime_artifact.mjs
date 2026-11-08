import assert from 'node:assert/strict'
import { spawnSync } from 'node:child_process'
import { createHash } from 'node:crypto'
import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { test } from 'node:test'
import { fileURLToPath } from 'node:url'

const native = new URL('../../plain-libs/native/ai_image_search/', import.meta.url)
const verifier = fileURLToPath(new URL('verify_onnxruntime.cmake', native))
const sha256 = bytes => createHash('sha256').update(bytes).digest('hex')

test('validates a prepared runtime against a trusted digest and the target recipe', t => {
  const directory = mkdtempSync(join(tmpdir(), 'plain-onnx-artifact-'))
  t.after(() => rmSync(directory, { recursive: true, force: true }))
  const library = join(directory, 'libonnxruntime.so')
  const receipt = join(directory, 'runtime.json')
  const target = 'x86_64-unknown-linux-gnu'
  const artifact = 'synthetic runtime fixture'
  const digest = sha256(artifact)
  const identity = { target, source: true, recipe: sha256(readFileSync(new URL('prepare_onnxruntime.py', native))) }

  writeFileSync(library, artifact)
  writeFileSync(join(directory, 'LICENSE'), 'Fixture license')
  writeFileSync(join(directory, 'ThirdPartyNotices.txt'), 'Fixture notices')
  writeFileSync(receipt, JSON.stringify({ identity, sha256: digest }))

  const verify = (targetTriple = target, trustedDigest = digest) => spawnSync('cmake', ['-P', verifier], {
    encoding: 'utf8',
    env: { ...process.env, PLAIN_ONNX_RUNTIME_LIBRARY: library, PLAIN_ONNX_RUNTIME_TARGET: targetTriple, PLAIN_ONNX_RUNTIME_SHA256: trustedDigest, PLAIN_ONNX_RUNTIME_MANIFEST: '' },
  })
  const valid = verify()
  assert.equal(valid.status, 0, valid.stderr)
  assert.match(verify('aarch64-unknown-linux-gnu').stderr, /target or source recipe mismatch/)
  assert.match(verify(target, sha256('other artifact')).stderr, /SHA-256 mismatch/)

  writeFileSync(library, 'modified runtime fixture')
  assert.match(verify().stderr, /SHA-256 mismatch/)
  writeFileSync(library, artifact)
  writeFileSync(receipt, JSON.stringify({ identity: { ...identity, recipe: sha256('obsolete recipe') }, sha256: digest }))
  assert.match(verify().stderr, /target or source recipe mismatch/)
})

test('uses a trusted target digest before staging the runtime and notices', t => {
  const root = mkdtempSync(join(tmpdir(), 'plain-onnx-manifest-'))
  t.after(() => rmSync(root, { recursive: true, force: true }))
  const target = 'aarch64-apple-darwin'
  const directory = join(root, target)
  const manifest = join(root, 'trusted.json')
  const output = join(root, 'build output')
  const libraryName = 'libonnxruntime.dylib'
  const artifact = 'synthetic arm64 runtime'
  const digest = sha256(artifact)
  const files = {
    [libraryName]: artifact,
    'runtime.json': JSON.stringify({
      identity: { target, source: true, recipe: sha256(readFileSync(new URL('prepare_onnxruntime.py', native))) },
      sha256: digest,
    }),
    LICENSE: 'Fixture license',
    'ThirdPartyNotices.txt': 'Fixture notices',
  }
  mkdirSync(directory)
  for (const [name, contents] of Object.entries(files)) writeFileSync(join(directory, name), contents)
  const verify = () => spawnSync('cmake', ['-P', verifier], {
    encoding: 'utf8',
    env: {
      ...process.env,
      PLAIN_ONNX_RUNTIME_MANIFEST: manifest,
      PLAIN_ONNX_RUNTIME_TARGET: target,
      PLAIN_ONNX_RUNTIME_LIBRARY: join(directory, libraryName),
      PLAIN_ONNX_RUNTIME_OUTPUT_DIR: output,
    },
  })

  writeFileSync(manifest, JSON.stringify({ 'x86_64-apple-darwin': digest }))
  const missingTarget = verify()
  assert.notEqual(missingTarget.status, 0)
  assert.match(missingTarget.stderr, /aarch64-apple-darwin/)
  writeFileSync(manifest, JSON.stringify({ [target]: sha256('different artifact') }))
  const wrongDigest = verify()
  assert.notEqual(wrongDigest.status, 0)
  assert.match(wrongDigest.stderr, /SHA-256 mismatch/)
  assert.equal(existsSync(output), false)

  writeFileSync(manifest, JSON.stringify({ [target]: digest, 'x86_64-apple-darwin': sha256('other target') }))
  const valid = verify()
  assert.equal(valid.status, 0, valid.stderr)
  for (const [name, contents] of Object.entries(files)) assert.equal(readFileSync(join(output, name), 'utf8'), contents)
})
