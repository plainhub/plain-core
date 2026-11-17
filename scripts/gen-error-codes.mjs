import { readdirSync, readFileSync, writeFileSync } from 'node:fs'
import { fileURLToPath } from 'node:url'

const root = new URL('../plain-rs/src/', import.meta.url)
const sources = readdirSync(root, { recursive: true }).filter(path => path.endsWith('.rs')).sort()
const kebab = value => value.replace(/([a-z])([A-Z])/g, '$1-$2').toLowerCase()
let count = 0
for (const path of sources) {
  const source = readFileSync(new URL(path, root), 'utf8')
  const match = source.match(/crate::feature_error!\s*\{\s*(\w+)\s*\{([^}]+)\}/)
  if (!match) continue
  const [, name, body] = match
  const codes = [...body.matchAll(/(\w+)\s*=>\s*"([^"\n]+)"/g)].map(([, variant, wire]) => ({ variant, wire }))
  if (!codes.length) throw new Error(`No error codes in ${path}`)
  for (const { variant, wire } of codes) {
    const expected = variant.replace(/([A-Z]+)([A-Z][a-z])/g, '$1_$2').replace(/([a-z0-9])([A-Z])/g, '$1_$2').toUpperCase()
    if (wire !== expected) throw new Error(`Invalid enum value ${name}.${variant}: ${wire}; expected ${expected}`)
  }
  const declaration = `export enum ${name} {\n${codes.map(({ variant, wire }) => `  ${variant} = '${wire}',`).join('\n')}\n}\n`
  const typescript = `${declaration}\nconst codes = new Set<string>(Object.values(${name}))\n\nexport function parse${name}(value: unknown): ${name} | undefined {\n  return typeof value === 'string' && codes.has(value) ? value as ${name} : undefined\n}\n`
  const kotlin = `package com.ismartcoding.plain.models\n\nimport kotlinx.serialization.Serializable\n\n@Serializable\nenum class ${name} {\n${codes.map(({ wire }, index) => `    ${wire}${index === codes.length - 1 ? ';' : ','}`).join('\n')}\n\n    val wire: String get() = name\n\n    companion object {\n        fun fromWire(value: String?): ${name}? = entries.firstOrNull { it.name == value }\n    }\n}\n`
  const file = kebab(name)
  const outputs = new Map([
    [`../../plain-desktop/src/lib/api/${file}.ts`, typescript],
    [`../../plain-app/shared/src/commonMain/kotlin/com/ismartcoding/plain/models/${name}.kt`, kotlin],
    [`../../plainapp-app/app/data/api-docs/${file}.ts`, declaration],
    [`../../plainapp-app/app/data/api-docs/${file}.mjs`, `export const ${name} = Object.freeze(${JSON.stringify(Object.fromEntries(codes.map(({ variant, wire }) => [variant, wire])), null, 2)})\n`],
  ])
  for (const [target, output] of outputs) {
    const url = new URL(target, import.meta.url)
    if (process.argv.includes('--write')) writeFileSync(url, output)
    else if (readFileSync(url, 'utf8') !== output) throw new Error(`Stale error codes: ${fileURLToPath(url)}`)
  }
  count++
}
if (!count) throw new Error('No feature error enums found')
console.log(`Verified ${count} feature error enums`)
