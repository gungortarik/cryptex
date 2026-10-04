import { test } from 'node:test';
import assert from 'node:assert/strict';
import * as fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { describePackage, digest, initialize, PackageError, recover, selected, transition } from './package-transaction.mjs';
import { killTransactionAt } from './package-transaction-crash.mjs';

function setup(t) {
  const parent = fs.mkdtempSync(path.join(os.tmpdir(), 'cryptex-m0-pair-'));
  const sources = {};
  for (const component of ['desktop', 'daemon']) {
    sources[component] = path.join(parent, `${component}.fixture`);
    fs.writeFileSync(sources[component], `synthetic ${component} payload\n`);
  }
  const fixture = path.join(parent, 'data-fixture'); fs.mkdirSync(fixture);
  fs.writeFileSync(path.join(fixture, 'sealed-events.fixture'), 'synthetic immutable bytes\n');
  fs.writeFileSync(path.join(fixture, 'blob.fixture'), Buffer.from([0, 255, 10, 42]));
  const before = inventory(fixture);
  const root = path.join(parent, 'transaction');
  const baseline = describePackage('baseline', sources.desktop, sources.daemon);
  const candidate = describePackage('candidate', sources.desktop, sources.daemon);
  initialize(root, baseline, sources);
  t.after(() => {
    try { assert.deepEqual(inventory(fixture), before, 'untouched fixture inventory and bytes'); }
    finally { fs.rmSync(parent, { recursive: true, force: true }); }
  });
  return { parent, root, sources, baseline, candidate };
}
function inventory(root) {
  return fs.readdirSync(root).sort().map(name => [name, digest(path.join(root, name))]);
}
function code(expected) { return error => error instanceof PackageError && error.code === expected; }
const healthy = { health: async () => true };

test('compatible pair commit retains complete previous pair', async t => {
  const { root, sources, candidate } = setup(t);
  assert.deepEqual(await transition(root, candidate, sources, healthy), { schema: 1, active: 'candidate', previous: 'baseline' });
  assert.equal(recover(root).recovered, false);
});
test('declared incompatible pair rejected before any staging', async t => {
  const { root, sources, candidate } = setup(t);
  candidate.desktop.protocol = 2;
  await assert.rejects(transition(root, candidate, sources, healthy), code('INCOMPATIBLE_PAIR'));
  assert.equal(selected(root).active, 'baseline');
  assert.equal(fs.existsSync(path.join(root, 'slots', 'candidate')), false);
});
test('payload digest mismatch rejected before staging', async t => {
  const { root, sources, candidate } = setup(t);
  fs.appendFileSync(sources.daemon, 'corruption');
  await assert.rejects(transition(root, candidate, sources, healthy), code('PAYLOAD_MISMATCH'));
  assert.equal(selected(root).active, 'baseline');
});
for (const healthyResult of [false, undefined, 'true']) {
  test(`non-true health (${String(healthyResult)}) automatically restores old pair`, async t => {
    const { root, sources, candidate } = setup(t);
    await assert.rejects(transition(root, candidate, sources, { health: async () => healthyResult }), code('HEALTH_FAILED'));
    assert.equal(selected(root).active, 'baseline');
    assert.equal(recover(root).recovered, false);
    assert.equal(fs.existsSync(path.join(root, 'slots', 'candidate')), true);
  });
}
test('health exception restores old pair and exposes typed failure', async t => {
  const { root, sources, candidate } = setup(t);
  await assert.rejects(transition(root, candidate, sources, { health: async () => { throw new Error('fixture failure'); } }), code('HEALTH_FAILED'));
  assert.equal(selected(root).active, 'baseline');
});
test('health-time payload tampering restores baseline', async t => {
  const { root, sources, candidate } = setup(t);
  await assert.rejects(transition(root, candidate, sources, { health: async slot => { fs.appendFileSync(path.join(slot, 'daemon.bin'), 'bad'); return true; } }), code('PAYLOAD_MISMATCH'));
  assert.equal(selected(root).active, 'baseline');
});
test('health callback is mandatory', async t => {
  const { root, sources, candidate } = setup(t);
  await assert.rejects(transition(root, candidate, sources), code('HEALTH_REQUIRED'));
});
test('path traversal and unsupported schema rejected', async t => {
  const { root, sources, candidate } = setup(t);
  await assert.rejects(transition(root, { ...candidate, id: '../escape' }, sources, healthy), code('INVALID_ID'));
  await assert.rejects(transition(root, { ...candidate, schema: 2 }, sources, healthy), code('UNSUPPORTED_SCHEMA'));
});
test('existing experiment never overwritten', t => {
  const { root, sources, baseline } = setup(t);
  assert.throws(() => initialize(root, baseline, sources), error => error.code === 'EEXIST');
  assert.equal(selected(root).active, 'baseline');
});
test('existing slot never reused, even if staging was partial', async t => {
  const { root, sources, candidate } = setup(t);
  fs.mkdirSync(path.join(root, 'slots', 'candidate'));
  await assert.rejects(transition(root, candidate, sources, healthy), error => error.code === 'EEXIST');
  assert.equal(selected(root).active, 'baseline');
});
test('malformed selection is an explicit error', t => {
  const { root } = setup(t);
  fs.writeFileSync(path.join(root, 'selection.json'), '{');
  assert.throws(() => recover(root), code('CORRUPT_METADATA'));
});
test('malformed journal retained for diagnosis', t => {
  const { root } = setup(t);
  fs.writeFileSync(path.join(root, 'pending.json'), '{');
  assert.throws(() => recover(root), code('CORRUPT_METADATA'));
  assert.equal(fs.readFileSync(path.join(root, 'pending.json'), 'utf8'), '{');
});
test('foreign journal relationship refused without changing selection', t => {
  const { root } = setup(t);
  const before = selected(root);
  fs.writeFileSync(path.join(root, 'pending.json'), JSON.stringify({ schema: 1, before, after: { schema: 1, active: 'candidate', previous: 'foreign' } }));
  assert.throws(() => recover(root), code('CORRUPT_JOURNAL'));
  assert.deepEqual(selected(root), before);
});
test('source symlink or junction rejected', async t => {
  const { root, sources, candidate, parent } = setup(t);
  const link = path.join(parent, 'source-link');
  // Directory junctions do not require Windows Developer Mode or elevation.
  fs.symlinkSync(parent, link, process.platform === 'win32' ? 'junction' : 'dir');
  await assert.rejects(transition(root, candidate, { ...sources, desktop: path.join(link, 'desktop.fixture') }, healthy), code('UNSAFE_PATH'));
});
test('repository experiment path rejected before creation', () => {
  const inside = fileURLToPath(new URL('./forbidden-runtime', import.meta.url));
  assert.throws(() => initialize(inside, {}, {}), code('REPOSITORY_ROOT'));
  assert.equal(fs.existsSync(inside), false);
});

async function killAt(t, context, boundary) {
  const result = await killTransactionAt(context.root, context.candidate, context.sources, boundary);
  assert.equal(result.checkpoint, boundary);
  assert.ok(result.pid > 0);
}
for (const boundary of ['staged', 'intent', 'selected', 'healthy', 'committed']) {
  test(`actual updater-process kill at ${boundary}: recover complete pair`, { timeout: 15000 }, async t => {
    const context = setup(t);
    await killAt(t, context, boundary);
    const result = recover(context.root);
    assert.equal(result.selection.active, boundary === 'committed' ? 'candidate' : 'baseline');
    assert.equal(result.recovered, ['intent', 'selected', 'healthy'].includes(boundary));
    assert.deepEqual(recover(context.root).selection, result.selection, 'recovery is idempotent');
  });
}
test('corrupt baseline during pending recovery fails closed and keeps journal', async t => {
  const context = setup(t);
  await killAt(t, context, 'selected');
  fs.appendFileSync(path.join(context.root, 'slots', 'baseline', 'daemon.bin'), 'bad');
  assert.throws(() => recover(context.root), code('PAYLOAD_MISMATCH'));
  assert.equal(fs.existsSync(path.join(context.root, 'pending.json')), true);
});
test('successive failed update restores most recent good pair', async t => {
  const { root, sources, candidate } = setup(t);
  await transition(root, candidate, sources, healthy);
  await assert.rejects(transition(root, { ...candidate, id: 'third' }, sources, { health: async () => false }), code('HEALTH_FAILED'));
  assert.deepEqual(selected(root), { schema: 1, active: 'candidate', previous: 'baseline' });
});
test('unknown manifest protocol and missing component rejected', async t => {
  const { root, sources, candidate } = setup(t);
  await assert.rejects(transition(root, { ...candidate, desktop: { ...candidate.desktop, protocol: 0 } }, sources, healthy), code('INVALID_MANIFEST'));
  await assert.rejects(transition(root, { ...candidate, daemon: null }, sources, healthy), code('INVALID_MANIFEST'));
});
test('incomplete active slot cannot be selected', t => {
  const { root } = setup(t);
  fs.unlinkSync(path.join(root, 'slots', 'baseline', 'daemon.bin'));
  assert.throws(() => selected(root), code('INCOMPLETE_SLOT'));
});
test('root ownership marker cannot be silently adopted', t => {
  const { root } = setup(t);
  fs.writeFileSync(path.join(root, 'experiment.json'), JSON.stringify({ schema: 1, scope: 'foreign' }));
  assert.throws(() => selected(root), code('UNOWNED_ROOT'));
});
test('new transition refuses unresolved intent', async t => {
  const context = setup(t);
  await killAt(t, context, 'intent');
  await assert.rejects(transition(context.root, context.candidate, context.sources, healthy), code('RECOVERY_REQUIRED'));
  assert.equal(recover(context.root).selection.active, 'baseline');
});
