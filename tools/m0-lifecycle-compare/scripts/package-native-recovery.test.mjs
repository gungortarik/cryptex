import { test } from 'node:test';
import assert from 'node:assert/strict';
import * as fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { spawn } from 'node:child_process';
import { randomUUID } from 'node:crypto';
import { createFixtureRoot } from './package-transaction-fixtures.mjs';
import { describePackage, initialize, digest, recover } from './package-transaction.mjs';
import { killTransactionAt } from './package-transaction-crash.mjs';
import { startRecovery } from './package-transaction-process.mjs';
import { clearInterruptedClaim, recoverAfterInterruption } from './package-native-recovery.mjs';

const helper = fileURLToPath(new URL('../../../target/debug/m0-lifecycle-compare' + (process.platform === 'win32' ? '.exe' : ''), import.meta.url));
function fixture(t) {
  const parent = createFixtureRoot();
  const canary = path.join(parent, 'immutable-fixture');
  fs.writeFileSync(canary, 'synthetic immutable fixture\n');
  const before = digest(canary);
  t.after(() => {
    try { assert.equal(digest(canary), before); }
    finally { fs.rmSync(parent, { recursive: true, force: true }); }
  });
  const sources = { desktop: path.join(parent, 'desktop'), daemon: path.join(parent, 'daemon') };
  fs.writeFileSync(sources.desktop, 'synthetic desktop'); fs.writeFileSync(sources.daemon, 'synthetic daemon');
  const root = path.join(parent, 'experiment');
  initialize(root, describePackage('baseline', sources.desktop, sources.daemon), sources);
  return { root, sources, candidate: describePackage('candidate', sources.desktop, sources.daemon) };
}
function claim(root, pid = process.pid) {
  const file = path.join(root, 'recovery-claim.json');
  fs.writeFileSync(file, JSON.stringify({ schema: 1, pid, token: randomUUID() }), { flag: 'wx' });
  return file;
}
async function interrupted(t) {
  const context = fixture(t);
  await killTransactionAt(context.root, context.candidate, context.sources, 'selected');
  const child = startRecovery(context.root, 'restore-selection');
  try { await child.checkpoint(); }
  finally { await child.stop(); }
  return context;
}
test('native cleaner refuses live recovery claim without changing its bytes', async t => {
  const { root } = fixture(t); const file = claim(root); const before = fs.readFileSync(file);
  await assert.rejects(clearInterruptedClaim(root, helper), error => error.code === 'RECOVERY_OWNER_ACTIVE');
  assert.deepEqual(fs.readFileSync(file), before);
});
test('native cleaner reclaims killed recovery and baseline retry is idempotent', { timeout: 20000 }, async t => {
  const { root } = await interrupted(t);
  const result = await recoverAfterInterruption(root, helper);
  assert.equal(result.cleanup.reclaimed, true);
  assert.equal(result.recovery.selection.active, 'baseline');
  assert.equal((await recoverAfterInterruption(root, helper)).cleanup.reclaimed, false);
  assert.equal(recover(root).recovered, false);
});
test('two native cleaners remove one stale claim without deleting a new live claim', { timeout: 20000 }, async t => {
  const { root } = await interrupted(t);
  const results = await Promise.allSettled([clearInterruptedClaim(root, helper), clearInterruptedClaim(root, helper)]);
  assert.equal(results.filter(r => r.status === 'fulfilled' && r.value.reclaimed === true).length, 1);
  for (const result of results) if (result.status === 'rejected') assert.equal(result.reason.code, 'NATIVE_RECOVERY_BUSY');
  const liveFile = claim(root); const liveBytes = fs.readFileSync(liveFile);
  await assert.rejects(clearInterruptedClaim(root, helper), error => error.code === 'RECOVERY_OWNER_ACTIVE');
  assert.deepEqual(fs.readFileSync(liveFile), liveBytes);
});
test('native cleaner refuses malformed claim and retains evidence', async t => {
  const { root } = fixture(t); const file = claim(root);
  fs.writeFileSync(file, '{');
  await assert.rejects(clearInterruptedClaim(root, helper), error => error.code === 'INVALID_RECOVERY_CLAIM');
  assert.equal(fs.readFileSync(file, 'utf8'), '{');
});
test('native cleanup OS lock is released after its holder is killed', { timeout: 20000 }, async t => {
  const { root } = await interrupted(t);
  const child = spawn(helper, ['clear-interrupted-recovery-claim', '--root', root, '--node', process.execPath, '--hold-ms', '10000'], { windowsHide: true, stdio: ['ignore', 'pipe', 'pipe'] });
  const closed = new Promise((resolve, reject) => { child.once('error', reject); child.once('close', (code, signal) => resolve({ code, signal })); });
  closed.catch(() => {});
  try {
    await new Promise((resolve, reject) => {
      const timer = setTimeout(() => reject(new Error('Native lock readiness timed out')), 5000);
      let output = '';
      child.stdout.on('data', chunk => { output += chunk; if (output.includes('"locked":true')) { clearTimeout(timer); resolve(); } });
      child.once('error', e => { clearTimeout(timer); reject(e); });
      child.once('close', () => { clearTimeout(timer); reject(new Error('Native holder exited before readiness')); });
    });
    await assert.rejects(clearInterruptedClaim(root, helper), e => e.code === 'NATIVE_RECOVERY_BUSY');
    assert.equal(child.kill('SIGKILL'), true); await closed;
    assert.equal((await clearInterruptedClaim(root, helper)).reclaimed, true);
    assert.equal(recover(root).selection.active, 'baseline');
  } finally {
    if (child.exitCode === null && child.signalCode === null) { child.kill('SIGKILL'); await closed; }
  }
});
test('native recovery adapter rejects repository root before mutation', async () => {
  const repository = fileURLToPath(new URL('../../../', import.meta.url));
  await assert.rejects(clearInterruptedClaim(repository, helper), e => e.code === 'REPOSITORY_ROOT');
});
