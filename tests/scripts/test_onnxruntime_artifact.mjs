import assert from 'node:assert/strict'
import { spawnSync } from 'node:child_process'
import { createHash } from 'node:crypto'
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs'
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
    env: { ...process.env, PLAIN_ONNX_RUNTIME_LIBRARY: library, PLAIN_ONNX_RUNTIME_TARGET: targetTriple, PLAIN_ONNX_RUNTIME_SHA256: trustedDigest },
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
