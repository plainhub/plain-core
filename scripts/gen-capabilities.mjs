import { readFileSync, writeFileSync } from 'node:fs'

const root = new URL('../', import.meta.url)
const source = readFileSync(new URL('plain-rs/src/features/capability.rs', root), 'utf8')
const names = [...source.matchAll(/Self::\w+ => "([A-Z_]+)"/g)].map(([, name]) => name)
if (!names.length || new Set(names).size !== names.length) throw new Error('Invalid Rust Capability wire names')
const header = '// Enum values generated from plain-core Capability; do not edit.\n'
const kotlin = new URL('../plain-app/shared/src/commonMain/kotlin/com/ismartcoding/plain/platform/system/Capability.kt', root)
const outputs = [
  [new URL('../plain-desktop/src/lib/capability.generated.js', root), header + 'export const Capability = Object.freeze({\n' + names.map(name => `  ${name}: '${name}',\n`).join('') + '})\n'],
  [new URL('../plain-desktop/src/lib/capability.generated.d.ts', root), header + 'export enum Capability {\n' + names.map(name => `  ${name} = '${name}',\n`).join('') + '}\n'],
  [kotlin, header + readFileSync(kotlin, 'utf8').replace(/^\/\/ Enum values generated.*\n/, '').replace(/enum class Capability \{[^}]+\}/, 'enum class Capability {\n' + names.map(name => `    ${name},\n`).join('') + '}')],
]
for (const [file, content] of outputs) {
  if (process.argv.includes('--write')) writeFileSync(file, content)
  else if (readFileSync(file, 'utf8') !== content) throw new Error('Run node scripts/gen-capabilities.mjs --write in plain-core')
}
