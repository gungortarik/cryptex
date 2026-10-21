// Cold synthetic profile snapshot/restore experiment, not a shipped updater.
import * as fs from 'node:fs';
import path from 'node:path';
import { assertExperimentLocation, digest, PackageError } from './package-transaction.mjs';

function fail(code) { throw new PackageError(code, 'Synthetic cold profile transition refused'); }
export function assertSyncthingUpgradeHealth(observed, expected = 'v2.1.5') {
  if (typeof observed?.version !== 'string' || !/^v2\.1\.\d+$/.test(expected)) fail('INVALID_SYNCTHING_UPGRADE_HEALTH');
  if (observed.version !== expected) fail('SYNCTHING_UPGRADE_HEALTH_REJECTED');
}
export function assertColdSyncthingProfile(home, receipt) {
  assertExperimentLocation(home);
  if (receipt?.home !== home || receipt.exit_code !== 0 || receipt.process_absent !== true
    || !Number.isSafeInteger(receipt.pid) || receipt.pid < 1) fail('SYNCTHING_COLD_RECEIPT_REQUIRED');
  try { process.kill(receipt.pid, 0); }
  catch (cause) { if (cause.code === 'ESRCH') return; throw cause; }
  fail('SYNCTHING_PROFILE_PROCESS_PRESENT');
}
function inventory(root) {
  assertExperimentLocation(root);
  const entries = []; let size = 0; let totalBytes = 0;
  function visit(directory, prefix = '') {
    for (const name of fs.readdirSync(directory).sort()) {
      const file = path.join(directory, name); const relative = prefix ? `${prefix}/${name}` : name;
      const info = fs.lstatSync(file);
      if (info.isSymbolicLink() || (!info.isFile() && !info.isDirectory())) fail('UNSAFE_SYNCTHING_PROFILE_ENTRY');
      if (process.platform !== 'win32' && (info.mode & 0o7000)) fail('UNSAFE_SYNCTHING_PROFILE_ENTRY');
      if (++size > 10000) fail('SYNCTHING_PROFILE_LIMIT');
      const entry = { path: relative, directory: info.isDirectory(), mode: process.platform === 'win32' ? null : info.mode & 0o777 };
      if (info.isFile()) {
        totalBytes += info.size;
        if (info.size > 64 * 1024 * 1024 || totalBytes > 512 * 1024 * 1024) fail('SYNCTHING_PROFILE_LIMIT');
        entry.sha256 = digest(file);
      }
      entries.push(entry);
      if (info.isDirectory()) visit(file, relative);
    }
  }
  visit(root); return entries;
}
function equalInventory(root, expected) {
  if (JSON.stringify(inventory(root)) !== JSON.stringify(expected)) fail('SYNCTHING_COLD_SNAPSHOT_CHANGED');
}
function disjoint(source, destination) {
  assertExperimentLocation(source); assertExperimentLocation(destination);
  for (const [a, b] of [[source, destination], [destination, source]]) {
    const relative = path.relative(a, b);
    if (!relative || (!relative.startsWith(`..${path.sep}`) && relative !== '..' && !path.isAbsolute(relative))) fail('OVERLAPPING_SYNCTHING_PROFILES');
  }
  if (fs.existsSync(destination)) fail('SYNCTHING_PROFILE_DESTINATION_EXISTS');
}
function copyProfile(source, destination, expected) {
  disjoint(source, destination); equalInventory(source, expected);
  fs.mkdirSync(destination, { mode: 0o700 });
  for (const entry of expected) {
    const target = path.join(destination, ...entry.path.split('/'));
    if (entry.directory) fs.mkdirSync(target, { mode: 0o700 });
    else {
      fs.copyFileSync(path.join(source, ...entry.path.split('/')), target, fs.constants.COPYFILE_EXCL);
      if (entry.mode !== null) fs.chmodSync(target, entry.mode);
    }
  }
  if (process.platform !== 'win32') for (const entry of [...expected].reverse().filter(e => e.directory)) fs.chmodSync(path.join(destination, ...entry.path.split('/')), entry.mode);
  equalInventory(source, expected); equalInventory(destination, expected);
}
export function snapshotSyncthingProfile(home, backup, receipt) {
  assertColdSyncthingProfile(home, receipt);
  const entries = inventory(home);
  let marker;
  try { marker = JSON.parse(fs.readFileSync(path.join(home, 'pilot-owned.json'), 'utf8')); }
  catch (cause) { throw new PackageError('CORRUPT_SYNCTHING_UPGRADE_PROFILE', 'Synthetic ownership metadata unreadable', { cause }); }
  if (marker?.scope !== 'm0-syncthing-upgrade-pilot' || marker.schema !== 1 || marker.pin !== '2.1.2') fail('UNOWNED_SYNCTHING_UPGRADE_PROFILE');
  copyProfile(home, backup, entries);
  return { home, backup, entries };
}
export function restoreSyncthingProfile(snapshot, candidateReceipt, quarantine) {
  assertColdSyncthingProfile(snapshot.home, candidateReceipt);
  disjoint(snapshot.home, quarantine);
  disjoint(snapshot.backup, snapshot.home + '-restore');
  // Verify/copy the cold baseline before moving the rejected profile. Do not
  // downgrade the candidate's potentially migrated native database in place.
  const restored = snapshot.home + '-restore';
  copyProfile(snapshot.backup, restored, snapshot.entries);
  fs.renameSync(snapshot.home, quarantine);
  fs.renameSync(restored, snapshot.home);
  equalInventory(snapshot.home, snapshot.entries);
  return { cold_baseline_restored: true, rejected_profile_retained: true };
}
