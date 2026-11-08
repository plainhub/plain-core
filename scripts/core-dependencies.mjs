import { execFileSync } from 'node:child_process'
import { createHash } from 'node:crypto'
import { cpSync, existsSync, mkdirSync, mkdtempSync, readFileSync, realpathSync, rmSync, statSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { dirname, join, relative } from 'node:path'
import { fileURLToPath } from 'node:url'

const coreUrl = 'https://github.com/plainhub/plain-core.git'
const packages = ['plain-libs', 'plain-rs', 'plain-server']

function readIfExists(path) {
  try {
    return readFileSync(path)
  } catch (error) {
    if (error.code !== 'ENOENT') throw error
  }
}

function restoreFile(path, bytes) {
  if (bytes === undefined) rmSync(path, { force: true })
  else writeFileSync(path, bytes)
}

function isFile(path) {
  return statSync(path, { throwIfNoEntry: false })?.isFile() === true
}

export function cargo(root, manifest, ...args) {
  const metadata = args[0] === 'metadata'
  if (metadata) {
    const version = execFileSync('rustc', ['-vV'], { cwd: root, encoding: 'utf8' })
    const host = version.match(/^host: (.+)$/m)?.[1]
    if (!host) throw new Error('Rust host target was not found')
    args.push('--filter-platform', host)
  }
  const result = execFileSync('cargo', [...args, '--manifest-path', manifest], {
    cwd: root,
    encoding: 'utf8',
    stdio: ['ignore', metadata ? 'pipe' : 'ignore', 'inherit'],
    maxBuffer: 32 * 1024 * 1024,
  })
  if (metadata) return JSON.parse(result)
}

function patchFor(core, url) {
  return `\n[patch."${url}"]\n` + packages.map(name => `${name} = { path = ${JSON.stringify(join(core, name))} }\n`).join('')
}

export function resolvedPatches(metadata, url) {
  const byId = new Map(metadata.packages.map(pkg => [pkg.id, pkg]))
  const used = new Map()
  for (const node of metadata.resolve.nodes) {
    const dependencies = byId.get(node.id).dependencies
    for (const edge of node.deps) {
      const target = byId.get(edge.pkg)
      if (!packages.includes(target.name)) continue
      if (!dependencies.some(dep => dep.source?.split('?')[0] === `git+${url}` &&
        (dep.rename ?? dep.name).replaceAll('-', '_') === edge.name)) continue
      const path = realpathSync(target.manifest_path)
      if (used.has(target.name) && used.get(target.name) !== path) {
        throw new Error(`Multiple resolutions for ${target.name}`)
      }
      used.set(target.name, path)
    }
  }
  if (!used.size) throw new Error('No plain-core Git dependencies were resolved')
  return used
}

function manifests(root, manifest) {
  return Object.fromEntries([...new Set([join(root, 'Cargo.toml'), join(root, 'src-tauri/Cargo.toml'), manifest])]
    .filter(isFile)
    .map(path => [relative(root, path), createHash('sha256').update(readFileSync(path)).digest('hex')]))
}

function locations(root, manifest) {
  const state = join(root, '.cargo', 'core-dependency-state')
  return {
    lock: join(dirname(manifest), 'Cargo.lock'),
    config: join(root, '.cargo', 'config.toml'),
    state,
    record: join(state, 'state.json'),
    pinnedLock: join(state, 'Cargo.lock.pinned'),
  }
}

export function configure(root, manifest, core, mode, url = coreUrl) {
  const { lock, config, state, record: recordPath, pinnedLock } = locations(root, manifest)
  const beforeConfig = readIfExists(config)
  const beforeLock = readFileSync(lock)
  const beforeRecord = existsSync(state) ? readFileSync(recordPath) : undefined
  const record = beforeRecord && JSON.parse(beforeRecord)
  const baseConfig = beforeConfig ?? Buffer.alloc(0)
  const useLocal = mode === 'local'
  if (!record && !useLocal) {
    cargo(root, manifest, 'metadata', '--format-version', '1', '--locked')
    return
  }

  let remaining = baseConfig
  let patchIndex = baseConfig.length
  let patchLength = 0
  if (record) {
    const patch = Buffer.from(record.patch)
    patchIndex = baseConfig.indexOf(patch)
    patchLength = patch.length
    if (patchIndex < 0 || baseConfig.indexOf(patch, patchIndex + patchLength) >= 0) {
      throw new Error('The local core override was edited; restore it before switching')
    }
    remaining = Buffer.concat([baseConfig.subarray(0, patchIndex), baseConfig.subarray(patchIndex + patchLength)])
    if (useLocal && record.core !== core) {
      throw new Error('Switch to pinned mode before changing the core checkout')
    }
  } else if (baseConfig.includes(`[patch."${url}"]`)) {
    throw new Error('Existing plain-core patch must be reviewed before switching')
  }

  if (useLocal && !record) {
    for (const name of packages) {
      if (!isFile(join(core, name, 'Cargo.toml'))) throw new Error(`Missing core package: ${name}`)
    }
    mkdirSync(dirname(state), { recursive: true })
    mkdirSync(state)
  }
  try {
    if (useLocal) {
      const patch = patchFor(core, url)
      if (!record) {
        writeFileSync(pinnedLock, beforeLock)
        writeFileSync(join(state, 'config.original'), baseConfig)
      }
      writeFileSync(config, Buffer.concat([baseConfig.subarray(0, patchIndex), Buffer.from(patch), baseConfig.subarray(patchIndex + patchLength)]))
      const metadata = cargo(root, manifest, 'metadata', '--format-version', '1')
      for (const [name, path] of resolvedPatches(metadata, url)) {
        if (path !== realpathSync(join(core, name, 'Cargo.toml'))) {
          throw new Error(`Local override was not applied: ${name}`)
        }
      }
      writeFileSync(recordPath, JSON.stringify({
        ...(record ?? { core, config_exists: beforeConfig !== undefined, manifests: manifests(root, manifest) }),
        patch,
      }))
    } else {
      restoreFile(config, remaining.length || record.config_exists ? remaining : undefined)
      writeFileSync(lock, readFileSync(pinnedLock))
      const current = manifests(root, manifest)
      const unchanged = Object.keys(current).length === Object.keys(record.manifests).length &&
        Object.entries(current).every(([path, hash]) => record.manifests[path] === hash)
      cargo(root, manifest, 'metadata', '--format-version', '1', ...(unchanged ? ['--locked'] : []))
    }
  } catch (error) {
    restoreFile(config, beforeConfig)
    writeFileSync(lock, beforeLock)
    if (!record) rmSync(state, { recursive: true })
    else if (useLocal) writeFileSync(recordPath, beforeRecord)
    throw error
  }
  if (!useLocal) rmSync(state, { recursive: true })
}

export function updateCurrent(root, manifest, core, url = coreUrl) {
  const refs = execFileSync('git', ['ls-remote', url, 'refs/heads/main'], { cwd: root, encoding: 'utf8' })
  const commit = refs.match(/^([0-9a-f]{40})\s+refs\/heads\/main\s*$/)?.[1]
  if (!commit) throw new Error('plain-core/main was not found')
  const { lock, config, state } = locations(root, manifest)
  const beforeConfig = readIfExists(config)
  const beforeLock = readFileSync(lock)
  const directory = mkdtempSync(join(tmpdir(), 'plain-core-update-'))
  const backup = join(directory, 'state')
  try {
    if (existsSync(state)) cpSync(state, backup, { recursive: true })
    try {
      configure(root, manifest, core, 'pinned', url)
      cargo(root, manifest, 'update', '-p', 'plain-server', '--precise', commit)
    } catch (error) {
      restoreFile(config, beforeConfig)
      writeFileSync(lock, beforeLock)
      if (existsSync(backup)) {
        rmSync(state, { recursive: true, force: true })
        cpSync(backup, state, { recursive: true })
      }
      throw error
    }
  } finally {
    rmSync(directory, { recursive: true })
  }
  return commit
}

function main(args) {
  const usage = 'Usage: ./scripts/core local [--off] | update'
  if (args.length === 1 && ['--help', '-h'].includes(args[0])) {
    console.log(usage)
    return
  }
  const [command, flag] = args
  if (args.length > 2 || !(command === 'local' && (!flag || flag === '--off') || command === 'update' && !flag)) {
    throw new Error(usage)
  }
  const root = realpathSync(process.cwd())
  const core = realpathSync(fileURLToPath(new URL('..', import.meta.url)))
  let manifest
  if (isFile(join(root, 'src-tauri', 'Cargo.toml'))) manifest = join(root, 'Cargo.toml')
  else if (isFile(join(root, 'gradlew')) && isFile(join(root, 'plain-rs', 'Cargo.toml'))) manifest = join(root, 'plain-rs', 'Cargo.toml')
  else throw new Error('Run ./scripts/core from the desktop or app repository root')
  if (command === 'update') {
    const commit = updateCurrent(root, manifest, core)
    console.log(`plain-core updated to ${commit.slice(0, 12)}`)
  } else {
    configure(root, manifest, core, flag === '--off' ? 'pinned' : 'local')
    console.log(flag === '--off' ? 'Using locked plain-core' : 'Using local plain-core')
  }
}

if (process.argv[1] && realpathSync(process.argv[1]) === fileURLToPath(import.meta.url)) {
  try {
    main(process.argv.slice(2))
  } catch (error) {
    console.error(`core: ${error.message}`)
    process.exitCode = 1
  }
}
