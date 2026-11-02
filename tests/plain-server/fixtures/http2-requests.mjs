import http2 from 'node:http2'
import { readFileSync } from 'node:fs'
const input = JSON.parse(readFileSync(process.argv[2], 'utf8'))
const connection = http2.connect(input.origin, { ca: input.certificate })
connection.on('error', error => { console.error(error.message); process.exitCode = 1 })
try {
  const responses = await Promise.all(input.requests.map(body => new Promise((resolve, reject) => {
    const stream = connection.request({ ':method': 'POST', ':path': '/graphql', 'c-id': input.client })
    let status
    const chunks = []
    stream.on('response', headers => { status = headers[':status'] })
    stream.on('data', chunk => chunks.push(chunk))
    stream.on('error', reject)
    stream.on('end', () => resolve({ status, body: Buffer.concat(chunks).toString('base64') }))
    stream.end(Buffer.from(body, 'base64'))
  })))
  process.stdout.write(JSON.stringify(responses))
} finally { connection.close() }
