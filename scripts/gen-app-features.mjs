import { readFileSync, writeFileSync } from 'node:fs'
import { resolve, sep } from 'node:path'
import { pathToFileURL } from 'node:url'
import { parseArgs } from 'node:util'

const { values: { target, 'consumer-root': consumerRoot, write } } = parseArgs({
  options: {
    target: { type: 'string', default: 'both' },
    'consumer-root': { type: 'string' },
    write: { type: 'boolean' },
  },
})
if (!['desktop', 'android', 'both'].includes(target)) throw new Error('Expected --target=desktop|android|both')
if (consumerRoot !== undefined && (target === 'both' || !consumerRoot)) {
  throw new Error('--consumer-root requires a path and exactly one target')
}

const root = new URL('../', import.meta.url)
const checkout = name => consumerRoot ? pathToFileURL(resolve(consumerRoot) + sep) : new URL(`../${name}/`, root)
const source = readFileSync(new URL('plain-rs/src/features/app_feature.rs', root), 'utf8')
const names = [...source.matchAll(/^    (\w+),$/gm)].map(([, name]) =>
  name.replace(/([a-z])([A-Z])/g, '$1_$2').toUpperCase())
const header = '// Enum values generated from plain-core AppFeatureType; do not edit.\n'
const outputs = []
if (target !== 'android') {
  const desktop = new URL('src/lib/app-feature.generated.ts', checkout('plain-desktop'))
  outputs.push([desktop, header + 'export enum AppFeatureType {\n' + names.map(name => `  ${name} = '${name}',\n`).join('') + '}\n'])
}
if (target !== 'desktop') {
  const kotlin = new URL('shared/src/commonMain/kotlin/com/ismartcoding/plain/platform/system/AppFeatureType.kt', checkout('plain-app'))
  outputs.push([kotlin, header + readFileSync(kotlin, 'utf8').replace(/^\/\/ Enum values generated.*\n/, '').replace(
    /enum class AppFeatureType \{[^;]+;/,
    'enum class AppFeatureType {\n' + names.map(name => `    ${name}`).join(',\n') + ';',
  )])
}
for (const [file, content] of outputs) {
  if (write) writeFileSync(file, content)
  else if (readFileSync(file, 'utf8') !== content) throw new Error(`Stale generated AppFeatureType: ${file.pathname}`)
}
