import assert from 'node:assert/strict';
import { spawn, spawnSync } from 'node:child_process';
import { readFile } from 'node:fs/promises';
import { createInterface } from 'node:readline';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { SourceTextModule, createContext, runInContext } from 'node:vm';
import { gzipSync, brotliCompressSync } from 'node:zlib';

const root = fileURLToPath(new URL('../', import.meta.url));
const build = spawnSync('cargo', ['build', '--locked', '-p', 'plain-libs', '--no-default-features', '--features', 'auth', '--example',
  'opaque-wasm-harness', '--target-dir', `${root}target/auth-wasm`], { cwd: root, stdio: 'inherit' });
assert.equal(build.status, 0, 'Rust harness build failed');
const pkg = process.argv[2] ?? `${root}../plain-desktop/src/lib/opaque/pkg/`;
const bytes = await readFile(`${pkg}/plain_auth_bg.wasm`);
const glue = await readFile(`${pkg}/plain_auth.js`, 'utf8');
const { initSync, OpaqueLogin } = await import(pathToFileURL(`${pkg}/plain_auth.js`));
initSync({ module: bytes });
const identity = new Uint8Array(32).fill(7);
const password = 'test password for OPAQUE';
const hex = (bytes) => Buffer.from(bytes).toString('hex');
const unhex = (value) => new Uint8Array(Buffer.from(value, 'hex'));

async function exchange(mode) {
  const child = spawn(`${root}target/auth-wasm/debug/examples/opaque-wasm-harness`, [], {
    stdio: ['pipe', 'pipe', 'inherit'],
  });
  const lines = createInterface({ input: child.stdout })[Symbol.asyncIterator]();
  const next = async () => {
    const line = await lines.next();
    assert.equal(line.done, false, 'Rust harness stopped before response');
    return unhex(line.value);
  };
  let login;
  try {
    const expectedPublicKey = await next();
    const input = new TextEncoder().encode(mode === 'wrong-password' ? 'wrong password' : password);
    login = OpaqueLogin.start(input);
    input.fill(0);
    child.stdin.write(`${hex(login.request())}\n`);
    const ke2 = await next();
    if (mode === 'tampered-ke2') ke2[ke2.length - 1] ^= 1;
    const state = login;
    login = undefined;
    const finish = () => state.finish(ke2,
      mode === 'wrong-context' ? 'wrong-client' : 'wasm-test-client',
      mode === 'wrong-identity' ? new Uint8Array(32).fill(8) : identity);
    if (mode !== 'success') {
      assert.throws(finish);
      assert.throws(() => state.request(), 'consumed login must not be reusable');
      return;
    }
    const started = performance.now();
    const result = finish();
    console.log(`WASM finish: ${(performance.now() - started).toFixed(1)} ms`);
    try {
      assert.deepEqual(result.serverPublicKey, expectedPublicKey);
      const clientKey = result.apiKey;
      try {
        assert.equal(clientKey.length, 32);
        child.stdin.write(`${hex(result.ke3)}\n`);
        assert.deepEqual(await next(), clientKey);
        assert.deepEqual(await next(), result.requestAad);
        assert.deepEqual(await next(), result.responseAad);
      } finally {
        clientKey.fill(0);
      }
    } finally {
      result.free();
    }
  } finally {
    login?.free();
    child.stdin.end();
    child.kill();
  }
}

for (const mode of ['success', 'wrong-password', 'tampered-ke2', 'wrong-context', 'wrong-identity']) {
  await exchange(mode);
  console.log(`PASS ${mode}`);
}

const sandbox = createContext({ TextDecoder, TextEncoder, WebAssembly, console });
const module = new SourceTextModule(glue, { context: sandbox });
await module.link(() => { throw new Error('Unexpected generated glue import'); });
await module.evaluate();
sandbox.initialize = module.namespace.initSync;
sandbox.bytes = bytes;
runInContext("initialize({ module: bytes })", sandbox);
assert.throws(() => module.namespace.OpaqueLogin.start(new Uint8Array([1, 2, 3])),
  (error) => String(error).includes("Secure randomness is unavailable"));
console.log('PASS missing secure randomness fails closed');
console.log(JSON.stringify({ wasmBytes: bytes.length, gzipBytes: gzipSync(bytes).length,
  brotliBytes: brotliCompressSync(bytes).length, glueBytes: Buffer.byteLength(glue) }));
