import { test } from 'node:test';
import assert from 'node:assert/strict';
import * as fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { spawn } from 'node:child_process';
import { createFixtureRoot } from './package-transaction-fixtures.mjs';
import { acquireNativeHandoverGate, withNativeHandoverGate } from './package-handover-gate.mjs';

const helper = fileURLToPath(new URL('../../../target/debug/m0-lifecycle-compare' + (process.platform === 'win32' ? '.exe' : ''), import.meta.url));
function fixture(t) {
  const root = createFixtureRoot();
  fs.writeFileSync(path.join(root, 'experiment.json'), JSON.stringify({ schema: 1, scope: 'm0-native-handover-pilot' }));
  t.after(() => fs.rmSync(root, { recursive: true, force: true }));
  return root;
}
test('whole-operation gate excludes startup/handover contender until release', async t => {
  const root = fixture(t); let called = false;
  await withNativeHandoverGate(root, helper, async lease => {
    lease.assertHeld();
    await assert.rejects(withNativeHandoverGate(root, helper, () => { called = true; }), { code: 'HANDOVER_BUSY' });
    lease.assertHeld(); assert.equal(called, false);
  });
  await withNativeHandoverGate(root, helper, lease => lease.assertHeld());
  assert.equal(fs.statSync(path.join(root, 'handover-native.lock')).isFile(), true);
});
test('operation error releases OS gate and preserves original failure', async t => {
  const root = fixture(t); const failure = new Error('synthetic operation error');
  await assert.rejects(withNativeHandoverGate(root, helper, () => { throw failure; }), error => error === failure);
  await withNativeHandoverGate(root, helper, lease => lease.assertHeld());
});
test('SIGKILL releases OS gate and invalidates killed lease before further phase', async t => {
  const root = fixture(t); const lease = await acquireNativeHandoverGate(root, helper);
  const aborted = new Promise(resolve => lease.signal.addEventListener('abort', resolve, { once: true }));
  process.kill(lease.pid, 'SIGKILL'); await aborted;
  assert.throws(lease.assertHeld, { code: 'HANDOVER_LEASE_LOST' });
  await assert.rejects(lease.release(), { code: 'HANDOVER_LEASE_LOST' });
  await withNativeHandoverGate(root, helper, next => next.assertHeld());
});
test('wrong ownership marker and linked lock are refused', async t => {
  const root = fixture(t); const marker = path.join(root, 'experiment.json');
  fs.writeFileSync(marker, JSON.stringify({ schema: 1, scope: 'm0-native-tree-proof' }));
  await assert.rejects(acquireNativeHandoverGate(root, helper), { code: 'UNOWNED_ROOT' });
  fs.writeFileSync(marker, JSON.stringify({ schema: 1, scope: 'm0-native-handover-pilot' }));
  const external = path.join(root, 'external'); fs.mkdirSync(external);
  fs.symlinkSync(external, path.join(root, 'handover-native.lock'), process.platform === 'win32' ? 'junction' : 'dir');
  await assert.rejects(acquireNativeHandoverGate(root, helper), { code: 'UNSAFE_HANDOVER_LOCK' });
  fs.rmdirSync(external); // Dangling links must also be rejected before open/create.
  await assert.rejects(acquireNativeHandoverGate(root, helper), { code: 'UNSAFE_HANDOVER_LOCK' });
  assert.equal(fs.existsSync(external), false);
});
test('repository and relative helper refused before creating a gate', async () => {
  await assert.rejects(acquireNativeHandoverGate(process.cwd(), helper), { code: 'REPOSITORY_ROOT' });
  await assert.rejects(acquireNativeHandoverGate(process.cwd(), 'relative'), { code: 'INVALID_HANDOVER_INPUT' });
  await assert.rejects(acquireNativeHandoverGate(null, helper), { code: 'INVALID_HANDOVER_INPUT' });
});
test('persistent lock file is reused and explicit release is idempotent', async t => {
  const root = fixture(t); const file = path.join(root, 'handover-native.lock');
  fs.writeFileSync(file, 'persistent sentinel');
  const lease = await acquireNativeHandoverGate(root, helper);
  assert.deepEqual(await lease.release(), { released: true, exit_code: 0 });
  assert.deepEqual(await lease.release(), { released: true, exit_code: 0 });
  await withNativeHandoverGate(root, helper, next => next.assertHeld());
  assert.equal(fs.readFileSync(file, 'utf8'), 'persistent sentinel');
});
test('stdin EOF releases gate but reports abnormal control completion', { timeout: 10000 }, async t => {
  const root = fixture(t); const child = spawn(helper, ['hold-native-handover-gate', '--root', root], { windowsHide: true, stdio: ['pipe', 'pipe', 'pipe'] });
  let output = ''; child.stdout.on('data', chunk => { output += chunk; }); child.stderr.resume();
  const closed = new Promise((resolve, reject) => { child.once('error', reject); child.once('close', resolve); });
  child.stdin.end(); assert.equal(await closed, 2);
  assert.match(output, /HANDOVER_CONTROL_FAILED/);
  await withNativeHandoverGate(root, helper, next => next.assertHeld());
});
