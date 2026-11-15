import { readFileSync, writeFileSync } from 'node:fs'

const root = new URL('../', import.meta.url)
const source = readFileSync(new URL('plain-rs/src/features/app_feature.rs', root), 'utf8')
const names = [...source.matchAll(/^    (\w+),$/gm)].map(([, name]) =>
  name.replace(/([a-z])([A-Z])/g, '$1_$2').toUpperCase())
const desktop = new URL('../plain-desktop/src/lib/app-feature.generated.ts', root)
const kotlin = new URL('../plain-app/shared/src/commonMain/kotlin/com/ismartcoding/plain/platform/system/AppFeatureType.kt', root)
const header = '// Enum values generated from plain-core AppFeatureType; do not edit.\n'
const outputs = [
  [desktop, header + 'export enum AppFeatureType {\n' + names.map(name => `  ${name} = '${name}',\n`).join('') + '}\n'],
  [kotlin, header + readFileSync(kotlin, 'utf8').replace(/^\/\/ Enum values generated.*\n/, '').replace(
    /enum class AppFeatureType \{[^;]+;/,
    'enum class AppFeatureType {\n' + names.map(name => `    ${name}`).join(',\n') + ';',
  )],
]
for (const [file, content] of outputs) {
  if (process.argv.includes('--write')) writeFileSync(file, content)
  else if (readFileSync(file, 'utf8') !== content) throw new Error('Run node scripts/gen-app-features.mjs --write in plain-core')
}
