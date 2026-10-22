const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const vm = require('node:vm');
const { EventEmitter } = require('node:events');
const source = fs.readFileSync(path.join(__dirname, 'main.js'), 'utf8');
const tick = () => new Promise(resolve => setImmediate(resolve));

async function fixture(onWrite, layoutReady = true, runtimeOverrides = {}, vaultRoot = 'C:/test/Cryptex-Dev/synthetic/vault') {
  const commands = [], evidence = [], notices = [], listeners = [], layouts = [], sockets = [];
  let noteIo = 0;
  const knownVault = {
    on(name, callback) { const ref = { name, callback }; listeners.push(ref); return ref; },
    adapter: { getBasePath() { return vaultRoot; }, async write(target, text) {
      assert.match(target, /^\.obsidian\/plugins\/cryptex-m7-spike\/(runtime-inventory|event-counts|startup-events-\d+|ipc-result|ipc-unload-obsidian-m7-ipc-unload-\d+)\.json$/);
      evidence.push({ target, value: JSON.parse(text) });
    } },
  };
  const vault = new Proxy(knownVault, { get(object, key) {
    if (Object.hasOwn(object, key)) return object[key];
    return () => { noteIo += 1; throw new Error('Forbidden managed-note IO'); };
  } });
  class Plugin {
    constructor() { this.manifest = { minAppVersion: '1.5.0' }; this.app = { vault, workspace: { onLayoutReady(callback) { layouts.push(callback); } } }; }
    addCommand(command) { commands.push(command); }
    registerEvent() {}
    addStatusBarItem() { return { setText(value) { this.text = value; } }; }
  }
  class Socket extends EventEmitter {
    constructor() { super(); this.destroyed = false; this.sent = []; }
    write(frame) { this.sent.push(JSON.parse(frame)); if (onWrite) onWrite(this, this.sent.at(-1)); }
    end() { this.destroy(); }
    destroy(error) {
      if (this.destroyed) return;
      this.destroyed = true;
      queueMicrotask(() => { if (error) this.emit('error', error); this.emit('close'); });
    }
  }
  const runtime = { platform: 'win32', arch: 'x64', versions: { electron: 'fixture', node: 'fixture' }, ...runtimeOverrides };
  const net = { createConnection(options) {
    assert.equal(options.path, runtime.platform === 'darwin' ? `/tmp/cpx-cryptex-${runtime.getuid()}/s` : '\\\\.\\pipe\\cryptex-cryptexd-v1');
    const socket = new Socket(); sockets.push(socket);
    queueMicrotask(() => { if (!socket.destroyed) socket.emit('connect'); });
    return socket;
  } };
  const sandbox = { module: { exports: {} }, require(name) {
    assert.equal(name, 'obsidian');
    return { Plugin, Notice: class { constructor(text) { notices.push(text); } }, apiVersion: '1.14.4' };
  }, window: { require(name) { if (name === 'process') return runtime; assert.equal(name, 'net'); return net; } }, Buffer, setTimeout, clearTimeout,
    console: { info() {}, warn() {}, error() {} } };
  vm.runInNewContext(source, sandbox, { filename: 'm7-observation-only.js' });
  const plugin = new sandbox.module.exports();
  await plugin.onload();
  if (layoutReady) { for (const callback of layouts) callback(); }
  await tick();
  return { plugin, commands, evidence, notices, listeners, layouts, sockets, noteIo: () => noteIo };
}
function healthResult(f) { return f.evidence.filter(x => x.target.endsWith('/ipc-result.json')).at(-1)?.value; }
function respond(socket, request, changes = {}) {
  const frame = { protocol_version: 1, request_id: request.request_id, operation: 'health.pong', accepted: true, daemon_pid: 123, ...changes };
  queueMicrotask(() => socket.emit('data', Buffer.from(JSON.stringify(frame) + '\n')));
}

test('only three observation/status commands are exposed', async () => {
  const f = await fixture();
  assert.deepEqual(f.commands.map(x => x.id).sort(), ['m7-spike-counts', 'm7-spike-ipc-health', 'm7-spike-ipc-unload']);
  assert.equal(f.sockets.length, 0);
  assert.match(f.plugin.status.text, /observation only; note IO disabled/);
});
test('runtime inventory records versions without selecting a product baseline', async () => {
  const f = await fixture();
  const inventory = f.evidence.find(x => x.target.endsWith('/runtime-inventory.json')).value;
  assert.equal(inventory.obsidian_api_version, '1.14.4');
  assert.equal(inventory.api_types_package, 'none: plain CommonJS runtime probe');
  assert.equal(inventory.product_baseline_selected, false);
  assert.equal(f.noteIo(), 0);
  assert.equal('vault_path' in inventory, false);
});
test('probe refuses a vault outside the explicit synthetic-scope parent', async () => {
  const f = await fixture(undefined, true, {}, 'C:/personal-vault');
  assert.equal(f.commands.length, 0);
  assert.equal(f.evidence.length, 0);
  await f.plugin.healthProbe();
  await f.plugin.ipcUnloadProbe();
  assert.equal(f.sockets.length, 0);
});
test('macOS health uses the existing daemon UID socket endpoint', async () => {
  const f = await fixture(respond, true, { platform: 'darwin', arch: 'arm64', getuid: () => 501 });
  await f.plugin.healthProbe(); await tick();
  assert.equal(healthResult(f).ok, true);
  assert.equal(f.sockets.length, 1);
});
test('missing macOS UID fails before attempting any socket connection', async () => {
  const f = await fixture(respond, true, { platform: 'darwin' });
  await f.plugin.healthProbe();
  assert.equal(healthResult(f).ok, false);
  assert.equal(f.sockets.length, 0);
});
test('all former mutation and path probes refuse even direct invocation', async () => {
  const f = await fixture();
  for (const name of ['processProbe', 'manualConflictProbe', 'cleanupManualMarker', 'junctionBoundaryProbe', 'junctionMetadataProbe', 'lifecycleProbe', 'concurrentProcessProbe']) {
    await f.plugin[name]();
  }
  assert.equal(f.noteIo(), 0);
  assert.equal(f.notices.length, 7);
});
test('observers count metadata without reading or classifying note contents', async () => {
  const f = await fixture();
  for (const name of ['modify', 'create', 'rename', 'delete']) {
    for (const event of f.listeners.filter(x => x.name === name)) await event.callback({ path: 'Cryptex/Entities/synthetic-entity.md' });
  }
  assert.equal(f.noteIo(), 0);
  assert.equal(f.plugin.stats.modify, 1);
  assert.equal(f.plugin.stats.create, 1);
  assert.equal(f.plugin.stats.rename, 1);
  assert.equal(f.plugin.stats.delete, 1);
  assert.equal('userModify' in f.plugin.stats, false);
  f.plugin.onunload();
  await f.plugin.onModify({ path: 'Cryptex/Entities/synthetic-entity.md' });
  assert.equal(f.plugin.stats.modify, 1);
});
test('late layout callback after unload adds no listeners or evidence', async () => {
  const f = await fixture(undefined, false);
  const count = f.evidence.length, listeners = f.listeners.length;
  f.plugin.onunload(); f.layouts[0](); await tick();
  assert.equal(f.evidence.length, count);
  assert.equal(f.listeners.length, listeners);
});
test('health uses only status.read and validates a matching response', async () => {
  const f = await fixture(respond); await f.plugin.healthProbe(); await tick();
  assert.equal(healthResult(f).ok, true);
  const request = f.sockets[0].sent[0];
  assert.equal(request.operation, 'health.ping');
  assert.deepEqual(request.capabilities, ['status.read']);
  assert.equal('body' in request, false);
  assert.equal(f.plugin.activeIpcSockets.size, 0);
});
test('health refuses malformed and mismatched responses', async () => {
  for (const mode of ['malformed', 'mismatch']) {
    const f = await fixture((socket, request) => {
      if (mode === 'malformed') queueMicrotask(() => socket.emit('data', Buffer.from('{not-json}\n')));
      else respond(socket, request, { request_id: 'wrong' });
    });
    await f.plugin.healthProbe(); await tick();
    assert.equal(healthResult(f).ok, false);
    assert.equal(f.plugin.activeIpcSockets.size, 0);
  }
});
test('health caps received UTF-8 bytes', async () => {
  const f = await fixture(socket => queueMicrotask(() => socket.emit('data', Buffer.from('é'.repeat(32769)))));
  await f.plugin.healthProbe(); await tick();
  assert.equal(healthResult(f).ok, false);
  assert.equal(f.sockets[0].destroyed, true);
});
test('health settles on close before response', async () => {
  const f = await fixture(socket => socket.destroy());
  await f.plugin.healthProbe(); assert.equal(healthResult(f).ok, false);
});
test('unload closes ordinary in-flight health and settles without late notice', async () => {
  const f = await fixture(); const pending = f.plugin.healthProbe(); await tick();
  assert.equal(f.plugin.activeIpcSockets.size, 1);
  f.plugin.onunload(); await pending; await tick();
  assert.equal(f.sockets[0].destroyed, true);
  assert.equal(f.plugin.activeIpcSockets.size, 0);
  assert.equal(healthResult(f).ok, false);
  assert.equal(f.notices.length, 0);
});
test('explicit pending fixture cleanup also releases tracked socket', async () => {
  const f = await fixture(); const pending = f.plugin.ipcUnloadProbe(); await tick();
  f.plugin.onunload(); await pending; await tick();
  assert.equal(f.plugin.activeIpcSockets.size, 0);
  assert.equal(f.evidence.at(-1).value.closed_on_unload, true);
  assert.equal(f.notices.length, 0);
});
test('health timeout closes transport and clears tracking', async () => {
  const f = await fixture(); await f.plugin.healthProbe(); await tick();
  assert.equal(healthResult(f).ok, false);
  assert.equal(f.plugin.activeIpcSockets.size, 0);
  assert.equal(f.sockets[0].destroyed, true);
});
