import { strict as assert } from 'node:assert'
import { test } from 'node:test'
import { schemaStatements } from '../gen-db-schema-sql.mjs'

test('generated defaults match Rust string values for quoted JSON and raw SQL', () => {
  const sql = `CREATE TABLE IF NOT EXISTS sample (value TEXT DEFAULT '{"code":"NONE"}');`
  const escaped = sql.replaceAll('"', '\\"')
  assert.deepEqual(schemaStatements(`conn.execute_batch("${escaped}")`), [sql])
  assert.deepEqual(schemaStatements(`conn.execute_batch(r#"${sql}"#)`), [sql])
})
