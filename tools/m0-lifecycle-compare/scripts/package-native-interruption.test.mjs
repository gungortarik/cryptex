import { test } from 'node:test';
import assert from 'node:assert/strict';
import * as fs from 'node:fs';
import path from 'node:path';
import { spawn, spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import { once } from 'node:events';
import { setTimeout as pause } from 'node:timers/promises';
import { createFixtureRoot } from './package-transaction-fixtures.mjs';
import { describeNativeTree } from './package-native-tree.mjs';
import { acquireNativeHandoverGate } from './package-handover-gate.mjs';
import { prepareNativeSlots, recoverNativeSelected, readNativeSelection, startNativeSelected } from './package-native-selection.mjs';

const helper = fileURLToPath(new URL(`../../../target/debug/m0-lifecycle-compare${process.platform === 'win32' ? '.exe' : ''}`, import.meta.url));
function fixture(t) {
  const directory = createFixtureRoot(); t.after(() => fs.rmSync(directory, { recursive: true, force: true }));
  const root = path.join(directory, 'proof'); fs.mkdirSync(root);
  fs.writeFileSync(path.join(root, 'experiment.json'), JSON.stringify({ schema: 1, scope: 'm0-native-handover-pilot' }));
  const inputs = ['baseline', 'candidate'].map(name => {
    const source = path.join(directory, name); fs.mkdirSync(source);
    for (const role of ['desktop', 'daemon']) { fs.writeFileSync(path.join(source, role), name); fs.chmodSync(path.join(source, role), 0o755); }
    return { source, manifest: describeNativeTree(source, { desktop: 'desktop', daemon: 'daemon' }) };
  });
  const lease = { root, assertHeld() {} }; prepareNativeSlots(lease, ...inputs);
  const exited = spawnSync(process.execPath, ['-e', ''], { windowsHide: true }); assert.equal(exited.status, 0);
  const intent = { schema: 2, scope: 'm0-native-selection', before: { schema: 1, scope: 'm0-native-selection', slot: 'baseline' },
    after: { schema: 1, scope: 'm0-native-selection', slot: 'candidate' }, owner_pid: exited.pid, runtime_prefix: 'interrupted' };
  const intentFile = path.join(root, 'native-intent.json'); fs.writeFileSync(intentFile, JSON.stringify(intent));
  return { root, lease, helper, runtimeName: 'recovered', intent, intentFile,
    start: async () => ({ stop: async () => ({ exit_code: 0, lock_released: true, endpoint_absent: true }) }) };
}
test('live native owner refuses recovery and preserves intent', async t => {
  const input = fixture(t); input.intent.owner_pid = process.pid; fs.writeFileSync(input.intentFile, JSON.stringify(input.intent));
  await assert.rejects(recoverNativeSelected(input), { code: 'NATIVE_PROCESS_LIVE' });
  assert.equal(fs.existsSync(input.intentFile), true);
});
test('legacy or malformed native intent cannot be silently adopted', async t => {
  const input = fixture(t); input.intent.schema = 1; fs.writeFileSync(input.intentFile, JSON.stringify(input.intent));
  await assert.rejects(recoverNativeSelected(input), { code: 'UNSUPPORTED_NATIVE_INTENT' });
  fs.writeFileSync(input.intentFile, '{'); await assert.rejects(recoverNativeSelected(input), { code: 'CORRUPT_NATIVE_RECORD' });
});
test('runtime without complete PID evidence refuses recovery before selection mutation', async t => {
  const input = fixture(t); const runtime = path.join(input.root, 'interrupted-candidate'); fs.mkdirSync(runtime);
  fs.writeFileSync(path.join(runtime, 'pilot-runtime.marker'), 'm0-native-desktop-runtime-v1\n');
  await assert.rejects(recoverNativeSelected(input), { code: 'NATIVE_CHILD_ABSENCE_UNPROVEN' });
  assert.equal(fs.existsSync(input.intentFile), true);
});
test('live Desktop PID refuses recovery even with absent IPC endpoint', async t => {
  const input = fixture(t); const runtime = path.join(input.root, 'interrupted-candidate'); fs.mkdirSync(runtime);
  fs.writeFileSync(path.join(runtime, 'pilot-runtime.marker'), 'm0-native-desktop-runtime-v1\n');
  fs.writeFileSync(path.join(runtime, 'ready'), String(input.intent.owner_pid));
  fs.writeFileSync(path.join(runtime, 'gui-proof.snapshot'), `desktop_pid=${process.pid}\nupdated_unix_ms=1\n`);
  await assert.rejects(recoverNativeSelected(input), { code: 'NATIVE_PROCESS_LIVE' });
});
test('interrupted recovery claim blocks recovery and ordinary startup', async t => {
  const input = fixture(t); fs.writeFileSync(path.join(input.root, 'native-recovery.json'), '{}');
  await assert.rejects(recoverNativeSelected(input), { code: 'NATIVE_RECOVERY_CLAIM_PRESENT' });
  fs.unlinkSync(input.intentFile);
  await assert.rejects(startNativeSelected(input), { code: 'NATIVE_RECOVERY_CLAIM_PRESENT' });
});
test('failed baseline restart retains recovery claim and intent', async t => {
  const input = fixture(t); input.start = async () => { throw new Error('injected restart failure'); };
  await assert.rejects(recoverNativeSelected(input), { code: 'NATIVE_RECOVERY_FAILED' });
  assert.equal(fs.existsSync(input.intentFile), true); assert.equal(fs.existsSync(path.join(input.root, 'native-recovery.json')), true);
});
test('actual killed owner at selected boundary releases gate and recovers baseline', async t => {
  const input = fixture(t); fs.unlinkSync(input.intentFile);
  const worker = fileURLToPath(new URL('./native-selection-interruption-worker.mjs', import.meta.url));
  const child = spawn(process.execPath, [worker, input.root, helper], { windowsHide: true, stdio: ['ignore', 'pipe', 'pipe'] });
  let closed = false; const exit = once(child, 'close').then(() => { closed = true; });
  t.after(async () => { if (!closed) child.kill('SIGKILL'); await exit; });
  await new Promise((resolve, reject) => {
    const timer = setTimeout(() => reject(new Error('interruption worker readiness timeout')), 5000);
    let output = ''; child.stdout.on('data', chunk => { output += chunk; if (output.includes('\n')) { clearTimeout(timer); resolve(); } });
    child.once('error', error => { clearTimeout(timer); reject(error); });
    child.once('close', () => { clearTimeout(timer); if (!output) reject(new Error('worker exited before boundary')); });
  });
  assert.equal(readNativeSelection(input.lease).slot, 'candidate');
  child.kill('SIGKILL'); await exit;
  let lease;
  for (let attempt = 0; attempt < 20; attempt++) {
    try { lease = await acquireNativeHandoverGate(input.root, helper); break; }
    catch (cause) { if (cause.code !== 'HANDOVER_BUSY') throw cause; await pause(50); }
  }
  assert.ok(lease, 'native gate must release after owner exit');
  try {
    const result = await recoverNativeSelected({ ...input, lease });
    assert.equal(result.outcome, 'recovered-baseline'); assert.equal(readNativeSelection(lease).slot, 'baseline');
    assert.equal(fs.existsSync(input.intentFile), false); assert.equal(fs.existsSync(path.join(input.root, 'native-recovery.json')), false);
    await result.session.stop();
  } finally { await lease.release(); }
});
