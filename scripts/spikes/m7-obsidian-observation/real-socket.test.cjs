const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const vm = require('node:vm');
const net = require('node:net');
const { randomUUID } = require('node:crypto');
const source = fs.readFileSync(path.join(__dirname, 'main.js'), 'utf8');
const turn = () => new Promise(resolve => setTimeout(resolve, 5));
async function within(check, label) {
  const until = Date.now() + 700;
  while (!check()) { if (Date.now() >= until) throw new Error(label); await turn(); }
}
async function fixture(t, reply) {
  const endpoint = process.platform === 'win32'
    ? '\\\\.\\pipe\\m7-observation-review-' + randomUUID()
    : path.join('/tmp', 'm7-review-' + randomUUID() + '.sock');
  const peers = new Set(), clients = [], requests = [], evidence = [], notices = [];
  const server = net.createServer({ allowHalfOpen: true }, socket => {
    peers.add(socket); socket.on('error', () => {});
    socket.on('close', () => peers.delete(socket));
    let buffer = '';
    socket.on('data', chunk => {
      buffer += chunk.toString('utf8');
      if (!buffer.includes('\n')) return;
      const request = JSON.parse(buffer.slice(0, buffer.indexOf('\n')));
      requests.push(request);
      if (reply) socket.write(JSON.stringify({ protocol_version: 1, request_id: request.request_id, operation: 'health.pong', accepted: true, daemon_pid: 123 }) + '\n');
      // Keep the peer open even after the client's FIN. Client must destroy it.
    });
  });
  await new Promise((resolve, reject) => { server.once('error', reject); server.listen(endpoint, resolve); });
  class Plugin {
    constructor() { this.app = { vault: {
      on() { return {}; }, adapter: { getBasePath() { return '/tmp/Cryptex-Dev/synthetic/vault'; }, async write(target, text) {
        assert.match(target, /^\.obsidian\/plugins\/cryptex-m7-spike\//);
        evidence.push(JSON.parse(text));
      } },
      read() { throw new Error('Forbidden note read'); },
      process() { throw new Error('Forbidden note write'); }
    }, workspace: { onLayoutReady(callback) { callback(); } } }; }
    addCommand() {} registerEvent() {}
    addStatusBarItem() { return { setText() {} }; }
  }
  const sandbox = { module: { exports: {} }, Buffer, setTimeout, clearTimeout,
    console: { info() {}, warn() {}, error() {} },
    require(name) { assert.equal(name, 'obsidian'); return { Plugin, Notice: class { constructor(text) { notices.push(text); } }, apiVersion: '1.14.4' }; },
    window: { require(name) {
      if (name === 'process') return process;
      assert.equal(name, 'net');
      return { createConnection(options) {
        assert.equal(options.path, process.platform === 'win32' ? '\\\\.\\pipe\\cryptex-cryptexd-v1' : `/tmp/cpx-cryptex-${process.getuid()}/s`);
        const client = net.createConnection({ path: endpoint }); clients.push(client); return client;
      } };
    } }
  };
  vm.runInNewContext(source, sandbox);
  const plugin = new sandbox.module.exports();
  await plugin.onload();
  t.after(async () => {
    plugin.onunload();
    for (const socket of clients) socket.destroy();
    for (const socket of peers) socket.destroy();
    await new Promise(resolve => server.close(resolve));
  });
  return { plugin, clients, peers, requests, evidence, notices };
}

test('real held-open peer cannot retain an ordinary health socket after reply', async t => {
  const f = await fixture(t, true);
  await f.plugin.healthProbe();
  await within(() => f.clients.every(socket => socket.destroyed) && f.plugin.activeIpcSockets.size === 0, 'client retained a half-open health socket');
  assert.equal(f.requests.length, 1);
  assert.equal(f.evidence.at(-1).ok, true);
});
test('unload closes a real pending local health socket with no late notice', async t => {
  const f = await fixture(t, false);
  const pending = f.plugin.healthProbe();
  await within(() => f.requests.length === 1, 'fixture received no health request');
  f.plugin.onunload(); await pending;
  await within(() => f.clients.every(socket => socket.destroyed) && f.plugin.activeIpcSockets.size === 0, 'pending socket did not close');
  assert.equal(f.notices.length, 0);
  assert.equal(f.evidence.at(-1).ok, false);
});
test('dedicated unload probe also finishes when replying peer keeps its end open', async t => {
  const f = await fixture(t, true);
  let settled = false;
  const pending = f.plugin.ipcUnloadProbe().then(() => { settled = true; });
  await within(() => settled, 'unload probe retained a half-open response socket');
  await pending;
  await within(() => f.plugin.activeIpcSockets.size === 0, 'probe socket remained tracked');
  assert.equal(f.evidence.at(-1).response_received, true);
});
