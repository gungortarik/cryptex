import { test } from 'node:test';
import assert from 'node:assert/strict';
import * as fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { describePackage, digest, initialize, PackageError, recover, selected, transition } from './package-transaction.mjs';
import { killTransactionAt } from './package-transaction-crash.mjs';
import { createFixtureRoot } from './package-transaction-fixtures.mjs';
import { startContender, startRecovery } from './package-transaction-process.mjs';

function setup(t) {
  const parent = createFixtureRoot();
  const fixture = path.join(parent, 'data-fixture');
  let before = null;
  // Register immediately: initialization failures must not leave owned roots.
  t.after(() => {
    try {
      if (before !== null) assert.deepEqual(inventory(fixture), before, 'untouched fixture inventory and bytes');
    } finally { fs.rmSync(parent, { recursive: true, force: true }); }
  });
  const sources = {};
  for (const component of ['desktop', 'daemon']) {
    sources[component] = path.join(parent, `${component}.fixture`);
    fs.writeFileSync(sources[component], `synthetic ${component} payload\n`);
  }
  fs.mkdirSync(fixture);
  fs.writeFileSync(path.join(fixture, 'sealed-events.fixture'), 'synthetic immutable bytes\n');
  fs.writeFileSync(path.join(fixture, 'blob.fixture'), Buffer.from([0, 255, 10, 42]));
  before = inventory(fixture);
  const root = path.join(parent, 'transaction');
  const baseline = describePackage('baseline', sources.desktop, sources.daemon);
  const candidate = describePackage('candidate', sources.desktop, sources.daemon);
  initialize(root, baseline, sources);
  return { parent, root, sources, baseline, candidate };
}
function inventory(root) {
  return fs.readdirSync(root).sort().map(name => [name, digest(path.join(root, name))]);
}
function code(expected) { return error => error instanceof PackageError && error.code === expected; }
const healthy = { health: async () => true };

test('temporary directory alias resolved before allocation; engine still rejects alias', t => {
  const { parent, sources, baseline } = setup(t);
  const alias = path.join(parent, 'temporary-alias');
  fs.symlinkSync(parent, alias, process.platform === 'win32' ? 'junction' : 'dir');
  const allocated = createFixtureRoot(alias);
  assert.equal(path.dirname(allocated), fs.realpathSync(parent));
  assert.equal(allocated.startsWith(`${alias}${path.sep}`), false);
  const experiment = path.join(allocated, 'experiment');
  assert.equal(initialize(experiment, baseline, sources).active, 'baseline');
  const aliasedExperiment = path.join(alias, path.basename(allocated), 'experiment');
  assert.throws(() => selected(aliasedExperiment), code('UNSAFE_PATH'));
});

test('compatible pair commit retains complete previous pair', async t => {
  const { root, sources, candidate } = setup(t);
  assert.deepEqual(await transition(root, candidate, sources, healthy), { schema: 1, active: 'candidate', previous: 'baseline' });
  assert.equal(recover(root).recovered, false);
});
test('stale concurrent transition fails closed after another candidate commits', async t => {
  const { root, sources, candidate } = setup(t);
  let releaseFirst;
  let stagedFirst;
  const firstIsStaged = new Promise(resolve => { stagedFirst = resolve; });
  const firstMayContinue = new Promise(resolve => { releaseFirst = resolve; });
  const first = transition(root, candidate, sources, {
    health: healthy.health,
    checkpoint: async point => {
      if (point === 'staged') {
        stagedFirst();
        await firstMayContinue;
      }
    },
  });
  await firstIsStaged;
  const second = await transition(root, { ...candidate, id: 'candidate-two' }, sources, healthy);
  assert.deepEqual(second, { schema: 1, active: 'candidate-two', previous: 'baseline' });
  releaseFirst();
  await assert.rejects(first, code('CONCURRENT_TRANSITION'));
  assert.deepEqual(selected(root), second);
  assert.equal(fs.existsSync(path.join(root, 'pending.json')), false);
});
test('concurrent commit between selection check and intent is detected', async t => {
  const { root, sources, candidate } = setup(t);
  let releaseFirst;
  let validatedFirst;
  const firstIsValidated = new Promise(resolve => { validatedFirst = resolve; });
  const firstMayContinue = new Promise(resolve => { releaseFirst = resolve; });
  const first = transition(root, candidate, sources, {
    health: healthy.health,
    checkpoint: async point => {
      if (point === 'selection-validated') {
        validatedFirst();
        await firstMayContinue;
      }
    },
  });
  await firstIsValidated;
  const second = await transition(root, { ...candidate, id: 'candidate-two' }, sources, healthy);
  releaseFirst();
  await assert.rejects(first, code('CONCURRENT_TRANSITION'));
  assert.deepEqual(selected(root), second);
  assert.equal(fs.existsSync(path.join(root, 'pending.json')), false);
});
for (const boundary of ['staged', 'selection-validated']) {
  test(`separate updater processes: competing commit at ${boundary} preserves winner`, { timeout: 20000 }, async t => {
    const { root, sources, candidate } = setup(t);
    const first = startContender(root, candidate, sources, boundary);
    let second;
    try {
      await first.checkpoint();
      second = startContender(root, { ...candidate, id: 'candidate-two' }, sources);
      assert.notEqual(first.pid, second.pid);
      const winner = await second.result();
      assert.equal(winner.ok, true);
      assert.equal(winner.exit_code, 0);
      first.release();
      const loser = await first.result();
      assert.equal(loser.code, 'CONCURRENT_TRANSITION');
      assert.equal(loser.exit_code, 2);
      assert.deepEqual(selected(root), winner.selection);
      assert.equal(fs.existsSync(path.join(root, 'pending.json')), false);
      assert.equal(recover(root).recovered, false);
    } finally {
      await first.stop();
      if (second) await second.stop();
    }
  });
}
test('separate updater processes: pending intent refuses a second updater', { timeout: 20000 }, async t => {
  const { root, sources, candidate } = setup(t);
  const first = startContender(root, candidate, sources, 'intent');
  let second;
  try {
    await first.checkpoint();
    const intent = fs.readFileSync(path.join(root, 'pending.json'));
    second = startContender(root, { ...candidate, id: 'candidate-two' }, sources);
    const refused = await second.result();
    assert.equal(refused.code, 'RECOVERY_REQUIRED');
    assert.equal(refused.exit_code, 2);
    assert.deepEqual(fs.readFileSync(path.join(root, 'pending.json')), intent);
    first.release();
    assert.equal((await first.result()).exit_code, 0);
    assert.equal(selected(root).active, 'candidate');
    assert.equal(recover(root).recovered, false);
  } finally {
    await first.stop();
    if (second) await second.stop();
  }
});
test('separate updater processes: simultaneous validated contenders yield one winner', { timeout: 20000 }, async t => {
  const { root, sources, candidate } = setup(t);
  const first = startContender(root, candidate, sources, 'selection-validated');
  const second = startContender(root, { ...candidate, id: 'candidate-two' }, sources, 'selection-validated');
  try {
    await Promise.all([first.checkpoint(), second.checkpoint()]);
    first.release();
    second.release();
    const results = await Promise.all([first.result(), second.result()]);
    assert.equal(results.filter(result => result.ok === true && result.exit_code === 0).length, 1);
    assert.equal(results.filter(result => result.code === 'CONCURRENT_TRANSITION' && result.exit_code === 2).length, 1);
    const winner = results.find(result => result.ok === true);
    assert.deepEqual(selected(root), winner.selection);
    assert.equal(winner.selection.previous, 'baseline');
    assert.equal(recover(root).recovered, false);
  } finally { await Promise.all([first.stop(), second.stop()]); }
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
for (const [operation, ioCode, expectedActive, phase] of [
  ['writeFileSync', 'ENOSPC', 'candidate', 'restore-selection'],
  ['fsyncSync', 'EIO', 'candidate', 'restore-selection'],
  ['renameSync', 'EACCES', 'candidate', 'restore-selection'],
  ['unlinkSync', 'EACCES', 'baseline', 'remove-intent'],
]) {
  test(`recovery ${operation} failure retains intent and allows safe retry`, async t => {
    const context = setup(t);
    await killAt(t, context, 'selected');
    const intent = path.join(context.root, 'pending.json');
    const journal = fs.readFileSync(intent);
    const slotBytes = ['baseline', 'candidate'].map(slot => inventory(path.join(context.root, 'slots', slot)));
    const injected = Object.assign(new Error('injected recovery I/O failure'), { code: ioCode });
    const io = { ...fs, [operation]: () => { throw injected; } };
    assert.throws(() => recover(context.root, { io }), error => {
      assert.equal(error.code, 'RECOVERY_IO_FAILED');
      assert.equal(error.cause, injected);
      assert.deepEqual(error.diagnostics, { phase, io_code: ioCode, intent_present: true });
      return true;
    });
    assert.deepEqual(fs.readFileSync(intent), journal, 'pending intent bytes retained');
    assert.equal(selected(context.root).active, expectedActive);
    assert.deepEqual(['baseline', 'candidate'].map(slot => inventory(path.join(context.root, 'slots', slot))), slotBytes);
    assert.equal(recover(context.root).selection.active, 'baseline');
    assert.equal(fs.existsSync(intent), false);
    assert.equal(recover(context.root).recovered, false, 'retry is idempotent');
  });
}
test('successive failed update restores most recent good pair', async t => {
  const { root, sources, candidate } = setup(t);
  await transition(root, candidate, sources, healthy);
  await assert.rejects(transition(root, { ...candidate, id: 'third' }, sources, { health: async () => false }), code('HEALTH_FAILED'));
  assert.deepEqual(selected(root), { schema: 1, active: 'candidate', previous: 'baseline' });
});
for (const healthFailure of ['rejected', 'throws']) {
  test(`automatic rollback I/O failure after health ${healthFailure} never reports restoration`, async t => {
    const { root, sources, candidate } = setup(t);
    const recoveryIo = { ...fs, renameSync: () => { throw Object.assign(new Error('injected rename refusal'), { code: 'EACCES' }); } };
    await assert.rejects(transition(root, candidate, sources, {
      recoveryIo,
      health: async () => {
        if (healthFailure === 'throws') throw new Error('injected health failure');
        return false;
      },
    }), error => {
      assert.equal(error.code, 'RECOVERY_IO_FAILED');
      assert.equal(error.diagnostics.io_code, 'EACCES');
      assert.equal(error.diagnostics.intent_present, true);
      assert.equal(error.message.includes('previous package restored'), false);
      return true;
    });
    assert.equal(selected(root).active, 'candidate');
    assert.throws(() => recover(root), code('RECOVERY_OWNER_ACTIVE'), 'external retry waits for owner process exit');
  });
}
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
for (const boundary of ['intent', 'selected', 'healthy']) {
  test(`separate recovery refuses live updater at ${boundary}; recovers after owner exit`, { timeout: 20000 }, async t => {
    const { root, sources, candidate } = setup(t);
    const updater = startContender(root, candidate, sources, boundary);
    let activeRecovery;
    let deadRecovery;
    try {
      await updater.checkpoint();
      const journal = fs.readFileSync(path.join(root, 'pending.json'));
      const selectionBytes = fs.readFileSync(path.join(root, 'selection.json'));
      activeRecovery = startRecovery(root);
      const refusal = await activeRecovery.result();
      assert.equal(refusal.code, 'RECOVERY_OWNER_ACTIVE');
      assert.equal(refusal.exit_code, 2);
      assert.deepEqual(fs.readFileSync(path.join(root, 'pending.json')), journal);
      assert.deepEqual(fs.readFileSync(path.join(root, 'selection.json')), selectionBytes);
      await updater.stop();
      deadRecovery = startRecovery(root);
      const recovery = await deadRecovery.result();
      assert.equal(recovery.ok, true);
      assert.equal(recovery.exit_code, 0);
      assert.equal(recovery.selection.active, 'baseline');
      assert.equal(fs.existsSync(path.join(root, 'pending.json')), false);
      assert.equal(recover(root).recovered, false);
    } finally {
      const cleanup = await Promise.allSettled([updater.stop(), activeRecovery?.stop(), deadRecovery?.stop()]);
      const errors = cleanup.filter(result => result.status === 'rejected').map(result => result.reason);
      if (errors.length) throw new AggregateError(errors, 'Owned recovery subprocess cleanup failed');
    }
  });
}
test('legacy ownerless intent is retained and never recovered automatically', async t => {
  const context = setup(t);
  await killAt(t, context, 'selected');
  const file = path.join(context.root, 'pending.json');
  const journal = JSON.parse(fs.readFileSync(file, 'utf8'));
  journal.schema = 1;
  delete journal.owner;
  fs.writeFileSync(file, JSON.stringify(journal));
  const before = fs.readFileSync(file);
  assert.throws(() => recover(context.root), code('RECOVERY_OWNER_UNKNOWN'));
  assert.deepEqual(fs.readFileSync(file), before);
  assert.equal(selected(context.root).active, 'candidate');
});
test('malformed updater owner fails closed and preserves intent', async t => {
  const context = setup(t);
  await killAt(t, context, 'selected');
  const file = path.join(context.root, 'pending.json');
  const journal = JSON.parse(fs.readFileSync(file, 'utf8'));
  journal.owner.pid = 0;
  fs.writeFileSync(file, JSON.stringify(journal));
  assert.throws(() => recover(context.root), code('CORRUPT_JOURNAL'));
  assert.equal(fs.existsSync(file), true);
});
test('refused recovery leaves live updater able to finish its commit', { timeout: 20000 }, async t => {
  const { root, sources, candidate } = setup(t);
  const updater = startContender(root, candidate, sources, 'selected');
  let recovery;
  try {
    await updater.checkpoint();
    recovery = startRecovery(root);
    assert.equal((await recovery.result()).code, 'RECOVERY_OWNER_ACTIVE');
    updater.release();
    const committed = await updater.result();
    assert.equal(committed.exit_code, 0);
    assert.equal(committed.selection.active, 'candidate');
    assert.equal(recover(root).recovered, false);
  } finally {
    const cleanup = await Promise.allSettled([updater.stop(), recovery?.stop()]);
    const errors = cleanup.filter(result => result.status === 'rejected').map(result => result.reason);
    if (errors.length) throw new AggregateError(errors, 'Owned updater/recovery cleanup failed');
  }
});
test('automatic rollback refuses an intent whose owner token changed', async t => {
  const { root, sources, candidate } = setup(t);
  await assert.rejects(transition(root, candidate, sources, {
    health: async () => {
      const file = path.join(root, 'pending.json');
      const journal = JSON.parse(fs.readFileSync(file, 'utf8'));
      journal.owner.token = '00000000-0000-0000-0000-000000000000';
      fs.writeFileSync(file, JSON.stringify(journal));
      return false;
    },
  }), code('RECOVERY_OWNER_CHANGED'));
  assert.equal(selected(root).active, 'candidate');
  assert.equal(fs.existsSync(path.join(root, 'pending.json')), true);
});
async function stopOwned(children) {
  const cleanup = await Promise.allSettled(children.map(child => child.stop()));
  const errors = cleanup.filter(result => result.status === 'rejected').map(result => result.reason);
  if (errors.length) throw new AggregateError(errors, 'Owned recovery race cleanup failed');
}
test('separate recovery claim refuses concurrent recovery and updater', { timeout: 20000 }, async t => {
  const context = setup(t);
  await killAt(t, context, 'selected');
  const children = [];
  const first = startRecovery(context.root, 'restore-selection'); children.push(first);
  try {
    await first.checkpoint();
    const journal = fs.readFileSync(path.join(context.root, 'pending.json'));
    const selectionBytes = fs.readFileSync(path.join(context.root, 'selection.json'));
    const second = startRecovery(context.root); children.push(second);
    assert.equal((await second.result()).code, 'RECOVERY_BUSY');
    const updater = startContender(context.root, { ...context.candidate, id: 'third' }, context.sources); children.push(updater);
    assert.equal((await updater.result()).code, 'RECOVERY_BUSY');
    assert.equal(fs.existsSync(path.join(context.root, 'slots', 'third')), false);
    assert.deepEqual(fs.readFileSync(path.join(context.root, 'pending.json')), journal);
    assert.deepEqual(fs.readFileSync(path.join(context.root, 'selection.json')), selectionBytes);
    first.release();
    const result = await first.result();
    assert.equal(result.exit_code, 0);
    assert.equal(result.selection.active, 'baseline');
    assert.equal(fs.existsSync(path.join(context.root, 'recovery-claim.json')), false);
    const successor = startContender(context.root, { ...context.candidate, id: 'third' }, context.sources); children.push(successor);
    assert.equal((await successor.result()).exit_code, 0);
    assert.equal(selected(context.root).active, 'third');
  } finally { await stopOwned(children); }
});
test('recovery claim blocks a prevalidated updater after old intent removal', { timeout: 20000 }, async t => {
  const context = setup(t);
  const children = [];
  const late = startContender(context.root, context.candidate, context.sources, 'selection-validated'); children.push(late);
  try {
    await late.checkpoint();
    const other = startContender(context.root, { ...context.candidate, id: 'candidate-two' }, context.sources, 'selected'); children.push(other);
    await other.checkpoint();
    await other.stop();
    const recovery = startRecovery(context.root, 'intent-removed'); children.push(recovery);
    await recovery.checkpoint();
    late.release();
    const refused = await late.result();
    assert.equal(refused.code, 'RECOVERY_BUSY');
    assert.equal(refused.exit_code, 2);
    const newIntent = fs.readFileSync(path.join(context.root, 'pending.json'));
    assert.equal(JSON.parse(newIntent).owner.pid, late.pid);
    recovery.release();
    assert.equal((await recovery.result()).exit_code, 0);
    assert.deepEqual(fs.readFileSync(path.join(context.root, 'pending.json')), newIntent, 'older recovery cannot delete newer intent');
    assert.equal(recover(context.root).selection.active, 'baseline');
    assert.equal(recover(context.root).recovered, false);
  } finally { await stopOwned(children); }
});
test('interrupted recovery retains claim and refuses automatic reclamation', { timeout: 20000 }, async t => {
  const context = setup(t);
  await killAt(t, context, 'selected');
  const children = [];
  const first = startRecovery(context.root, 'restore-selection'); children.push(first);
  try {
    await first.checkpoint();
    const journal = fs.readFileSync(path.join(context.root, 'pending.json'));
    const claim = fs.readFileSync(path.join(context.root, 'recovery-claim.json'));
    await first.stop();
    const retry = startRecovery(context.root); children.push(retry);
    assert.equal((await retry.result()).code, 'RECOVERY_BUSY');
    assert.deepEqual(fs.readFileSync(path.join(context.root, 'pending.json')), journal);
    assert.deepEqual(fs.readFileSync(path.join(context.root, 'recovery-claim.json')), claim);
    await assert.rejects(transition(context.root, { ...context.candidate, id: 'third' }, context.sources, healthy), code('RECOVERY_BUSY'));
  } finally { await stopOwned(children); }
});
