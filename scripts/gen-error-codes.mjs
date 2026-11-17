import { readFileSync, writeFileSync } from 'node:fs'
import { fileURLToPath } from 'node:url'

const source = readFileSync(new URL('../plain-rs/src/infrastructure/error_code.rs', import.meta.url), 'utf8')
const codes = [...source.matchAll(/Self::(\w+) => "([a-z_]+)"/g)].map(([, name, wire]) => ({ name, wire }))
if (!codes.length) throw new Error('No error codes found')

const typescriptEnum = `export enum ErrorCode {
${codes.map(({ name, wire }) => `  ${name} = '${wire}',`).join('\n')}
}
`
const typescript = `${typescriptEnum}
const codes = new Set<string>(Object.values(ErrorCode))

export function parseErrorCode(value: unknown): ErrorCode | undefined {
  if (typeof value !== 'string') return undefined
  const wire = value.toLowerCase()
  if (value !== wire && value !== wire.toUpperCase()) return undefined
  return codes.has(wire) ? wire as ErrorCode : undefined
}
`
const kotlin = `package com.ismartcoding.plain.models

import kotlinx.serialization.Serializable

@Serializable
enum class ErrorCode(val wire: String) {
${codes.map(({ wire }, index) => `    ${wire.toUpperCase()}("${wire}")${index === codes.length - 1 ? ';' : ','}`).join('\n')}

    companion object {
        fun fromWire(value: String?): ErrorCode? = entries.firstOrNull { it.wire == value || it.name == value }
    }
}
`
const outputs = new Map([
  ['../../plain-desktop/src/lib/api/error-code.ts', typescript],
  ['../../plain-app/shared/src/commonMain/kotlin/com/ismartcoding/plain/models/ErrorCode.kt', kotlin],
  ['../../plainapp-app/app/data/api-docs/error-code.ts', typescriptEnum],
  ['../../plainapp-app/app/data/api-docs/error-code.mjs', `export const ErrorCode = Object.freeze(${JSON.stringify(Object.fromEntries(codes.map(({ name, wire }) => [name, wire])), null, 2)})\n`],
])
for (const [path, output] of outputs) {
  const url = new URL(path, import.meta.url)
  if (process.argv.includes('--write')) writeFileSync(url, output)
  else if (readFileSync(url, 'utf8') !== output) throw new Error(`Stale error codes: ${fileURLToPath(url)}`)
}
console.log(`Verified ${codes.length} shared error codes`)
