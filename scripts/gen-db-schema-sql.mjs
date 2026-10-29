import { readFileSync, readdirSync, writeFileSync } from 'node:fs'
import { dirname, join } from 'node:path'
import { fileURLToPath } from 'node:url'

const ROOT = dirname(dirname(fileURLToPath(import.meta.url)))

function sharedDbSource() {
  const schemaDir = join(ROOT, 'plain-rs/src/db/schema')
  return readdirSync(schemaDir)
    .filter((name) => name.endsWith('.rs') && name !== 'mod.rs')
    .sort()
    .map((name) => join(schemaDir, name))
}

export function generateDatabaseSql() {
  const src = sharedDbSource().map((path) => readFileSync(path, 'utf8')).join('\n')
  const stmts = [...src.matchAll(/(CREATE (?:VIRTUAL TABLE|TABLE|UNIQUE INDEX|INDEX) IF NOT EXISTS [\s\S]*?;|CREATE TRIGGER IF NOT EXISTS [\s\S]*?\nEND;)/g)].map((m) =>
    m[1]
      .split('\n')
      .map((l) => l.trimEnd())
      .filter((l, i) => l.trim() !== '' || i > 0)
      .map((l, i, lines) => i > 0 && i < lines.length - 1 ? `    ${l.trim()}` : l.trim())
      .join('\n')
      .trim(),
  )
  const schemaSource = readFileSync(join(ROOT, 'plain-rs/src/db/schema/mod.rs'), 'utf8')
  const version = schemaSource.match(/const VERSION: i32 = (\d+);/)[1]
  const header = [
    '-- plain-core SQLite DDL (SQLite, WAL mode)',
    '-- Generated; do not edit manually. Source: plain-rs/src/db/schema/*.rs',
    '-- Generate: node scripts/gen-db-schema-sql.mjs --write',
    '-- Check: node scripts/gen-db-schema-sql.mjs',
    '',
    `PRAGMA user_version = ${version};`,
    '',
  ]
  return header.join('\n') + '\n' + stmts.join('\n\n') + '\n'
}

const FILE = 'schema/database.sql'

if (process.argv[1] && process.argv[1].endsWith('gen-db-schema-sql.mjs')) {
  if (process.argv.includes('--write')) {
    writeFileSync(join(ROOT, FILE), generateDatabaseSql())
    console.log(`wrote ${FILE}`)
  } else {
    const committed = readFileSync(join(ROOT, FILE), 'utf8')
    if (committed !== generateDatabaseSql()) {
      console.error(`${FILE} is stale. Run \`node scripts/gen-db-schema-sql.mjs --write\` and commit.`)
      process.exit(1)
    }
    console.log(`${FILE} is up to date`)
  }
}
