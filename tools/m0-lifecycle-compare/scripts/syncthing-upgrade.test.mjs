import { test } from 'node:test';
import assert from 'node:assert/strict';
import * as fs from 'node:fs';
import path from 'node:path';
import { execFileSync } from 'node:child_process';
import { createFixtureRoot } from './package-transaction-fixtures.mjs';
import { assertSyncthingUpgradeVersion, assertSyncthingVersion, startSyncthingUpgradePilot } from './syncthing-pilot.mjs';
import { snapshotSyncthingProfile, restoreSyncthingProfile, assertColdSyncthingProfile, assertSyncthingUpgradeHealth } from './syncthing-upgrade.mjs';

function fixture(t) {
  const root = createFixtureRoot(); t.after(() => fs.rmSync(root, { recursive: true, force: true }));
  const home = path.join(root, 'home'); fs.mkdirSync(home);
  fs.writeFileSync(path.join(home, 'pilot-owned.json'), JSON.stringify({ schema: 1, scope: 'm0-syncthing-upgrade-pilot', pin: '2.1.2' }));
  fs.writeFileSync(path.join(home, 'config.xml'), 'synthetic-config\n', { mode: 0o600 });
  fs.mkdirSync(path.join(home, 'db')); fs.writeFileSync(path.join(home, 'db', 'synthetic'), 'synthetic-old-db');
  const pid = Number(execFileSync(process.execPath, ['-e', 'process.stdout.write(String(process.pid))'], { windowsHide: true }));
  const receipt = { home, pid, exit_code: 0, process_absent: true };
  return { root, home, receipt, backup: path.join(root, 'backup') };
}
test('upgrade fixture versions exact and ordinary baseline remains pinned', async () => {
  for (const version of ['2.1.2', '2.1.5']) assertSyncthingUpgradeVersion(`syncthing v${version} x`, version);
  for (const [actual, expected] of [['syncthing v2.1.2-rc.1 x', '2.1.2'], ['syncthing v2.1.5 x', '2.1.2'], ['syncthing v2.1.50 x', '2.1.5'], ['syncthing v2.1.4 x', '2.1.4']]) assert.throws(() => assertSyncthingUpgradeVersion(actual, expected), { code: 'SYNCTHING_UPGRADE_PIN_MISMATCH' });
  assert.throws(() => assertSyncthingVersion('syncthing v2.1.2 x'), { code: 'SYNCTHING_PIN_MISMATCH' });
  await assert.rejects(startSyncthingUpgradePilot('/unused', '/unused', '2.1.5'), { code: 'SYNCTHING_UPGRADE_PIN_MISMATCH' });
});
test('candidate health rejects incompatible and malformed REST versions', () => {
  assertSyncthingUpgradeHealth({ version: 'v2.1.5' });
  assert.throws(() => assertSyncthingUpgradeHealth({ version: 'v2.1.5' }, 'v2.1.999'), { code: 'SYNCTHING_UPGRADE_HEALTH_REJECTED' });
  assert.throws(() => assertSyncthingUpgradeHealth({ version: 'v2.1.2' }), { code: 'SYNCTHING_UPGRADE_HEALTH_REJECTED' });
  assert.throws(() => assertSyncthingUpgradeHealth({}), { code: 'INVALID_SYNCTHING_UPGRADE_HEALTH' });
});
test('cold snapshot includes exact config and database with file modes', t => {
  const f = fixture(t); const snapshot = snapshotSyncthingProfile(f.home, f.backup, f.receipt);
  assert.equal(fs.readFileSync(path.join(f.backup, 'db', 'synthetic'), 'utf8'), 'synthetic-old-db');
  assert.ok(snapshot.entries.some(e => e.path === 'config.xml'));
  if (process.platform !== 'win32') assert.equal(fs.statSync(path.join(f.backup, 'config.xml')).mode & 0o777, 0o600);
});
test('snapshot refuses live or unclean process and mismatched receipt home', t => {
  const f = fixture(t);
  for (const receipt of [{ ...f.receipt, home: f.backup }, { ...f.receipt, exit_code: 1 }, { ...f.receipt, process_absent: false }]) assert.throws(() => snapshotSyncthingProfile(f.home, f.backup, receipt), { code: 'SYNCTHING_COLD_RECEIPT_REQUIRED' });
  assert.throws(() => assertColdSyncthingProfile(f.home, { ...f.receipt, pid: process.pid }), { code: 'SYNCTHING_PROFILE_PROCESS_PRESENT' });
  assert.equal(fs.existsSync(f.backup), false);
});
test('cold snapshot refuses unowned and ordinary managed profile', t => {
  const f = fixture(t);
  for (const scope of ['foreign', 'm0-managed-syncthing-pilot']) {
    fs.writeFileSync(path.join(f.home, 'pilot-owned.json'), JSON.stringify({ schema: 1, scope, pin: '2.1.2' }));
    assert.throws(() => snapshotSyncthingProfile(f.home, f.backup, f.receipt), { code: 'UNOWNED_SYNCTHING_UPGRADE_PROFILE' });
  }
});
test('unreadable or malformed ownership metadata fails before creating backup', t => {
  const f = fixture(t);
  fs.writeFileSync(path.join(f.home, 'pilot-owned.json'), '{');
  assert.throws(() => snapshotSyncthingProfile(f.home, f.backup, f.receipt), { code: 'CORRUPT_SYNCTHING_UPGRADE_PROFILE' });
  fs.writeFileSync(path.join(f.home, 'pilot-owned.json'), 'null');
  assert.throws(() => snapshotSyncthingProfile(f.home, f.backup, f.receipt), { code: 'UNOWNED_SYNCTHING_UPGRADE_PROFILE' });
  fs.unlinkSync(path.join(f.home, 'pilot-owned.json'));
  assert.throws(() => snapshotSyncthingProfile(f.home, f.backup, f.receipt), { code: 'CORRUPT_SYNCTHING_UPGRADE_PROFILE' });
  assert.equal(fs.existsSync(f.backup), false);
});
test('snapshot rejects overlap and existing destination without overwrite', t => {
  const f = fixture(t);
  assert.throws(() => snapshotSyncthingProfile(f.home, path.join(f.home, 'backup'), f.receipt), { code: 'OVERLAPPING_SYNCTHING_PROFILES' });
  fs.mkdirSync(f.backup); assert.throws(() => snapshotSyncthingProfile(f.home, f.backup, f.receipt), { code: 'SYNCTHING_PROFILE_DESTINATION_EXISTS' });
});
test('rollback retains rejected database and restores exact cold baseline', t => {
  const f = fixture(t); const snapshot = snapshotSyncthingProfile(f.home, f.backup, f.receipt);
  fs.writeFileSync(path.join(f.home, 'db', 'synthetic'), 'synthetic-migrated-db');
  const quarantine = path.join(f.root, 'rejected');
  const result = restoreSyncthingProfile(snapshot, f.receipt, quarantine);
  assert.equal(result.cold_baseline_restored, true);
  assert.equal(fs.readFileSync(path.join(quarantine, 'db', 'synthetic'), 'utf8'), 'synthetic-migrated-db');
  assert.equal(fs.readFileSync(path.join(f.home, 'db', 'synthetic'), 'utf8'), 'synthetic-old-db');
});
test('corrupt cold backup fails before moving candidate profile', t => {
  const f = fixture(t); const snapshot = snapshotSyncthingProfile(f.home, f.backup, f.receipt);
  fs.writeFileSync(path.join(f.backup, 'config.xml'), 'tampered');
  const quarantine = path.join(f.root, 'rejected');
  assert.throws(() => restoreSyncthingProfile(snapshot, f.receipt, quarantine), { code: 'SYNCTHING_COLD_SNAPSHOT_CHANGED' });
  assert.equal(fs.existsSync(quarantine), false); assert.equal(fs.readFileSync(path.join(f.home, 'config.xml'), 'utf8'), 'synthetic-config\n');
});
test('rollback rejects live candidate and existing quarantine', t => {
  const f = fixture(t); const snapshot = snapshotSyncthingProfile(f.home, f.backup, f.receipt);
  const quarantine = path.join(f.root, 'rejected');
  assert.throws(() => restoreSyncthingProfile(snapshot, { ...f.receipt, pid: process.pid }, quarantine), { code: 'SYNCTHING_PROFILE_PROCESS_PRESENT' });
  fs.mkdirSync(quarantine); assert.throws(() => restoreSyncthingProfile(snapshot, f.receipt, quarantine), { code: 'SYNCTHING_PROFILE_DESTINATION_EXISTS' });
});
if (process.platform !== 'win32') test('linked profile file refused before copying secret material', t => {
  const f = fixture(t); fs.symlinkSync(path.join(f.home, 'config.xml'), path.join(f.home, 'linked'));
  assert.throws(() => snapshotSyncthingProfile(f.home, f.backup, f.receipt), { code: 'UNSAFE_SYNCTHING_PROFILE_ENTRY' });
});
