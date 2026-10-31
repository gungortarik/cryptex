import { test } from 'node:test';
import assert from 'node:assert/strict';
import * as fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { spawn, spawnSync } from 'node:child_process';
import { once } from 'node:events';
import { createFixtureRoot } from './package-transaction-fixtures.mjs';
import { describeNativeTree } from './package-native-tree.mjs';
import { startOwnedDesktop, isDesktopLaunchSnapshot } from './package-desktop-session.mjs';
import { verifyOwnedProcess } from './package-process-identity.mjs';

function fixture(t) {
  const root = createFixtureRoot(); t.after(() => fs.rmSync(root, { recursive: true, force: true }));
  const payload = path.join(root, 'source'); fs.mkdirSync(payload);
  const roles = { desktop: 'desktop', daemon: 'daemon' };
  for (const name of Object.values(roles)) { fs.writeFileSync(path.join(payload, name), 'synthetic fixture'); fs.chmodSync(path.join(payload, name), 0o755); }
  return { root, payload, roles, manifest: describeNativeTree(payload, roles), lease: { root, assertHeld() {} }, helper: process.execPath, runtimeName: 'runtime' };
}
test('session refuses malformed inputs before creating runtime', async t => {
  const input = fixture(t);
  for (const changes of [{ lease: null }, { helper: 'relative' }, { runtimeName: '../escape' }, { runtimeName: 'runtime/name' }]) await assert.rejects(startOwnedDesktop({ ...input, ...changes }), { code: 'INVALID_DESKTOP_SESSION' });
  assert.equal(fs.existsSync(path.join(input.root, 'runtime')), false);
});
test('lost lease refuses launch before any runtime mutation', async t => {
  const input = fixture(t); const failure = Object.assign(new Error('lost test lease'), { code: 'HANDOVER_LEASE_LOST' });
  input.lease.assertHeld = () => { throw failure; };
  await assert.rejects(startOwnedDesktop(input), error => error === failure);
  assert.equal(fs.existsSync(path.join(input.root, 'runtime')), false);
});
test('resource mutation refuses session before launching anything', async t => {
  const input = fixture(t); fs.writeFileSync(path.join(input.payload, 'extra'), 'extra');
  await assert.rejects(startOwnedDesktop(input), { code: 'NATIVE_TREE_MISMATCH' });
  assert.equal(fs.existsSync(path.join(input.root, 'runtime')), false);
});
test('platform adapter proves own executable and parent without command-line capture', async () => {
  // Keep a child alive via an open stdin handle until identity assertions finish.
  // Querying a short-lived PID (or racing CIM against process exit) is not deterministic on Windows.
  const child = spawn(process.execPath, ['-e', 'process.stdin.resume();'], {
    windowsHide: true,
    stdio: ['pipe', 'ignore', 'ignore'],
  });
  try {
    if (!child.pid) await once(child, 'spawn');
    assert.ok(Number.isSafeInteger(child.pid) && child.pid > 0);
    process.kill(child.pid, 0);
    const result = await verifyOwnedProcess(child.pid, process.pid, process.execPath);
    assert.equal(result.pid, child.pid);
    assert.equal(result.parent, process.pid);
    assert.equal(Object.hasOwn(result, 'command_line'), false);
    process.kill(child.pid, 0);
  } finally {
    try { child.stdin.end(); } catch { /* ignore */ }
    if (child.exitCode === null && child.signalCode === null) child.kill('SIGKILL');
    await once(child, 'close');
  }
});
test('platform adapter rejects mismatched or invalid PID identity', async () => {
  await assert.rejects(verifyOwnedProcess(0, process.ppid, process.execPath), { code: 'INVALID_PROCESS_IDENTITY' });
  await assert.rejects(verifyOwnedProcess(process.pid, process.ppid + 1, process.execPath), { code: 'PROCESS_IDENTITY_MISMATCH' });
});
test('session runner refuses overlapping roots before marker or executable launch', t => {
  const input = fixture(t); const script = fileURLToPath(new URL('./run-desktop-session-proof.mjs', import.meta.url));
  const result = spawnSync(process.execPath, [script, '--source', input.payload, '--root', path.join(input.payload, 'nested'), '--helper', input.helper, '--desktop', input.roles.desktop, '--daemon', input.roles.daemon], { windowsHide: true, encoding: 'utf8', timeout: 5000 });
  assert.equal(result.status, 2); assert.equal(JSON.parse(result.stderr).code, 'OVERLAPPING_NATIVE_ROOTS');
  assert.equal(fs.existsSync(path.join(input.payload, 'nested')), false);
});
test('readiness ignores intermediate daemon snapshot and incomplete final writes', () => {
  assert.equal(isDesktopLaunchSnapshot('note=daemon_started_with_desktop\nupdated_unix_ms=1\n'), false);
  assert.equal(isDesktopLaunchSnapshot('note=desktop_launched\n'), false);
  assert.equal(isDesktopLaunchSnapshot('note=desktop_launched\nupdated_unix_ms=1'), false);
  assert.equal(isDesktopLaunchSnapshot('note=desktop_launched\nupdated_unix_ms=1\n'), true);
});
