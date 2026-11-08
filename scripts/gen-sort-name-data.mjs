import { mkdirSync, readFileSync, writeFileSync } from 'node:fs'
import { join } from 'node:path'

function array(text, name) {
  const match = text.match(new RegExp(`\\b${name}\\s*=\\s*byteArrayOf\\(([\\s\\S]*?)\\)`))
  if (!match) throw new Error(`Missing character table: ${name}`)
  return [...match[1].matchAll(/-?\d+/g)].map(([value]) => Number(value) & 255)
}

const [source, ...extra] = process.argv.slice(2)
if (!source || extra.length) throw new Error('Usage: node gen-sort-name-data.mjs <Kotlin pinyin source directory>')

const text = readFileSync(join(source, 'PinyinData.kt'), 'utf8')
const start = text.indexOf('arrayOf(')
if (start < 0) throw new Error('Missing syllable table')
const syllables = [...text.slice(start).matchAll(/"([^"\n]*)"/g)].map(match => match[1])
if (syllables[0] !== '') throw new Error('Invalid syllable table')

const indices = []
for (const part of [1, 2, 3]) {
  const text = readFileSync(join(source, `PinyinCode${part}.kt`), 'utf8')
  const padding = array(text, 'PINYIN_CODE_PADDING')
  const values = array(text, 'PINYIN_CODE')
  if (values.length !== 7000 || padding.length !== 875) throw new Error('Invalid character table size')
  for (const [index, value] of values.entries()) {
    indices.push(value | (((padding[index >> 3] >> (index & 7)) & 1) << 8))
  }
}

indices.length = 0x9fa5 - 0x4e00 + 1
if (indices.some(value => value >= syllables.length)) throw new Error('Invalid character table index')
const table = Buffer.alloc(indices.length * 2)
for (const [index, value] of indices.entries()) table.writeUInt16LE(value, index * 2)

const rows = []
for (let index = 0; index < syllables.length; index += 10) {
  rows.push('    ' + syllables.slice(index, index + 10).map(word => JSON.stringify(word)).join(', ') + ',')
}
const target = new URL('../plain-libs/src/utils/sort_name/', import.meta.url)
mkdirSync(target, { recursive: true })
writeFileSync(new URL('characters.bin', target), table)
writeFileSync(new URL('syllables.rs', target), `pub(super) const SYLLABLES: &[&str] = &[\n${rows.join('\n')}\n];\n`)
