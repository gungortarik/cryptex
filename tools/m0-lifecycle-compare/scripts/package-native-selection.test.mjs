import { test } from 'node:test';
import assert from 'node:assert/strict';
import * as fs from 'node:fs';
import path from 'node:path';
import { createFixtureRoot } from './package-transaction-fixtures.mjs';
import { describeNativeTree } from './package-native-tree.mjs';
import { prepareNativeSlots, readNativeSelection, startNativeSelected, handoverNativeSelected } from './package-native-selection.mjs';

function fixture(t) {
  const directory = createFixtureRoot(); t.after(() => fs.rmSync(directory, { recursive: true, force: true }));
  const root = path.join(directory, 'proof'); fs.mkdirSync(root);
  fs.writeFileSync(path.join(root, 'experiment.json'), JSON.stringify({ schema: 1, scope: 'm0-native-handover-pilot' }));
  const inputs = ['baseline', 'candidate'].map(name => {
    const source = path.join(directory, name); fs.mkdirSync(source);
    for (const role of ['desktop', 'daemon']) { fs.writeFileSync(path.join(source, role), `${name}-${role}`); fs.chmodSync(path.join(source, role), 0o755); }
    return { source, manifest: describeNativeTree(source, { desktop: 'desktop', daemon: 'daemon' }) };
  });
  const lease = { root, assertHeld() {} }; prepareNativeSlots(lease, ...inputs);
  const clean = { exit_code: 0, lock_released: true, endpoint_absent: true };
  const session = { stop: async () => clean };
  const active = { selection: { slot: 'baseline' }, session };
  const input = { lease, active, helper: process.execPath, runtimePrefix: 'test', start: async () => session };
  const intent = path.join(root, 'native-intent.json');
  return { ...input, root, intent, clean, session };
}
function rejected(cleanup) { return Object.assign(new Error('injected startup error'), { code: 'DESKTOP_SESSION_START_FAILED', diagnostics: { startup_code: 'TEST_REJECTED', cleanup } }); }
test('native selection commits after baseline stop and verified candidate start', async t => {
  const input = fixture(t); const events = [];
  input.active.session = { stop: async () => { events.push('stop'); return input.clean; } };
  input.start = async args => { events.push('start'); assert.equal(readNativeSelection(input.lease).slot, 'candidate'); assert.equal(fs.existsSync(input.intent), true); assert.equal(args.runtimeName, 'test-candidate'); return input.session; };
  const result = await handoverNativeSelected(input);
  assert.equal(result.outcome, 'committed'); assert.deepEqual(events, ['stop', 'start']); assert.equal(fs.existsSync(input.intent), false);
});
test('cleanly rejected candidate restores selection and starts baseline before removing intent', async t => {
  const input = fixture(t); const attempts = [];
  input.start = async args => { attempts.push(args.runtimeName); if (attempts.length === 1) throw rejected(input.clean); assert.equal(readNativeSelection(input.lease).slot, 'baseline'); assert.equal(fs.existsSync(input.intent), true); return input.session; };
  const result = await handoverNativeSelected(input);
  assert.equal(result.outcome, 'rolled-back'); assert.deepEqual(attempts, ['test-candidate', 'test-rollback']); assert.equal(fs.existsSync(input.intent), false);
});
test('candidate cleanup uncertainty retains candidate selection and journal without baseline launch', async t => {
  const input = fixture(t); let attempts = 0;
  input.start = async () => { attempts++; throw rejected({ endpoint_absent: false }); };
  await assert.rejects(handoverNativeSelected(input), { code: 'NATIVE_CANDIDATE_CLEANUP_UNPROVEN' });
  assert.equal(attempts, 1); assert.equal(readNativeSelection(input.lease).slot, 'candidate'); assert.equal(fs.existsSync(input.intent), true);
});
test('baseline stop failure preserves selection without intent or candidate launch', async t => {
  const input = fixture(t); input.active.session = { stop: async () => { throw new Error('stop failed'); } };
  input.start = async () => assert.fail('must not launch');
  await assert.rejects(handoverNativeSelected(input), { code: 'NATIVE_BASELINE_STOP_FAILED' });
  assert.equal(readNativeSelection(input.lease).slot, 'baseline'); assert.equal(fs.existsSync(input.intent), false);
});
test('unproven baseline cleanup refuses activation', async t => {
  const input = fixture(t); input.active.session = { stop: async () => ({ exit_code: 0 }) };
  await assert.rejects(handoverNativeSelected(input), { code: 'NATIVE_BASELINE_STOP_UNPROVEN' });
  assert.equal(fs.existsSync(input.intent), false);
});
test('damaged candidate resources reject before baseline stop', async t => {
  const input = fixture(t); input.active.session.stop = async () => assert.fail('must not stop');
  fs.writeFileSync(path.join(input.root, 'native-slots/candidate/payload/extra'), 'extra');
  await assert.rejects(handoverNativeSelected(input), { code: 'NATIVE_TREE_MISMATCH' });
  assert.equal(readNativeSelection(input.lease).slot, 'baseline');
});
test('pending intent blocks selected startup and handover even if malformed', async t => {
  const input = fixture(t); fs.writeFileSync(input.intent, 'broken');
  await assert.rejects(startNativeSelected(input), { code: 'NATIVE_RECOVERY_REQUIRED' });
  await assert.rejects(handoverNativeSelected(input), { code: 'NATIVE_RECOVERY_REQUIRED' });
  assert.equal(fs.readFileSync(input.intent, 'utf8'), 'broken');
});
test('rollback start failure retains journal and restored selection without claiming success', async t => {
  const input = fixture(t); input.start = async () => { throw rejected(input.clean); };
  await assert.rejects(handoverNativeSelected(input), { code: 'NATIVE_ROLLBACK_FAILED' });
  assert.equal(readNativeSelection(input.lease).slot, 'baseline'); assert.equal(fs.existsSync(input.intent), true);
});
test('lost native lease refuses before stop or selection mutation', async t => {
  const input = fixture(t); input.lease.assertHeld = () => { throw Object.assign(new Error('lease lost'), { code: 'HANDOVER_LEASE_LOST' }); };
  await assert.rejects(handoverNativeSelected(input), { code: 'HANDOVER_LEASE_LOST' }); assert.equal(fs.existsSync(input.intent), false);
});
test('unknown selection schema and unsafe candidate ID are refused', async t => {
  const input = fixture(t);
  await assert.rejects(handoverNativeSelected({ ...input, candidate: '../escape' }), { code: 'INVALID_NATIVE_SLOT' });
  fs.writeFileSync(path.join(input.root, 'native-selection.json'), JSON.stringify({ schema: 2, scope: 'm0-native-selection', slot: 'baseline' }));
  assert.throws(() => readNativeSelection(input.lease), { code: 'UNSUPPORTED_NATIVE_SELECTION' });
});
test('candidate post-start mutation is stopped and baseline restarted', async t => {
  const input = fixture(t); let starts = 0; let stops = 0;
  input.start = async () => { if (++starts === 1) { fs.writeFileSync(path.join(input.root, 'native-slots/candidate/payload/extra'), 'mutation'); return { stop: async () => { stops++; return input.clean; } }; } return input.session; };
  assert.equal((await handoverNativeSelected(input)).outcome, 'rolled-back'); assert.equal(starts, 2); assert.equal(stops, 1);
});
test('changed intent is retained and candidate cleaned without successful commit', async t => {
  const input = fixture(t); let stops = 0;
  input.start = async () => { fs.writeFileSync(input.intent, JSON.stringify({ schema: 9 })); return { stop: async () => { stops++; return input.clean; } }; };
  await assert.rejects(handoverNativeSelected(input), { code: 'NATIVE_COMMIT_FAILED' });
  assert.equal(stops, 1); assert.deepEqual(JSON.parse(fs.readFileSync(input.intent)), { schema: 9 });
});
test('nonzero cleanup exit cannot authorize rollback', async t => {
  const input = fixture(t); input.start = async () => { throw rejected({ ...input.clean, exit_code: 2 }); };
  await assert.rejects(handoverNativeSelected(input), { code: 'NATIVE_CANDIDATE_CLEANUP_UNPROVEN' }); assert.equal(fs.existsSync(input.intent), true);
});
