import { spawn } from 'node:child_process'
import { performance } from 'node:perf_hooks'
import { pathToFileURL } from 'node:url'

const [binary, workspace, cache] = process.argv.slice(2)
if (!binary || !workspace || !cache) {
  console.error('usage: node scripts/measure-lsp.mjs <spec-binary> <workspace> <cache>')
  process.exit(2)
}

const child = spawn(binary, ['--specs-dir', `${workspace}/.specs`, 'lsp', '--stdio'], {
  cwd: workspace,
  env: { ...process.env, FORGE_SPEC_CACHE_PATH: cache },
  stdio: ['pipe', 'pipe', 'inherit'],
})

let buffer = Buffer.alloc(0)
const pending = new Map()
child.stdout.on('data', chunk => {
  buffer = Buffer.concat([buffer, chunk])
  while (true) {
    const separator = buffer.indexOf('\r\n\r\n')
    if (separator < 0) return
    const header = buffer.subarray(0, separator).toString('utf8')
    const length = Number(header.match(/Content-Length:\s*(\d+)/i)?.[1])
    if (!Number.isFinite(length) || buffer.length < separator + 4 + length) return
    const start = separator + 4
    const message = JSON.parse(buffer.subarray(start, start + length).toString('utf8'))
    buffer = buffer.subarray(start + length)
    if (message.id !== undefined) pending.get(message.id)?.(message)
  }
})

function send(message) {
  const body = JSON.stringify(message)
  child.stdin.write(`Content-Length: ${Buffer.byteLength(body)}\r\n\r\n${body}`)
}

function request(id, method, params) {
  return new Promise((resolve, reject) => {
    const timeout = setTimeout(() => reject(new Error(`timed out waiting for ${method}`)), 30_000)
    pending.set(id, message => {
      clearTimeout(timeout)
      pending.delete(id)
      if (message.error) reject(new Error(JSON.stringify(message.error)))
      else resolve(message.result)
    })
    send({ jsonrpc: '2.0', id, method, params })
  })
}

const started = performance.now()
await request(1, 'initialize', {
  processId: null,
  rootUri: pathToFileURL(workspace).toString(),
  capabilities: {},
})
const initialized = performance.now()
send({ jsonrpc: '2.0', method: 'initialized', params: {} })
const snapshotStarted = performance.now()
const snapshot = await request(2, 'forgeSpec/explorerSnapshot', {})
const snapshotFinished = performance.now()
await request(3, 'shutdown', null)
send({ jsonrpc: '2.0', method: 'exit', params: null })
child.stdin.end()

console.log(JSON.stringify({
  initializeMs: Math.round((initialized - started) * 100) / 100,
  snapshotMs: Math.round((snapshotFinished - snapshotStarted) * 100) / 100,
  documents: snapshot.documents.length,
  stats: snapshot.stats,
}, null, 2))
