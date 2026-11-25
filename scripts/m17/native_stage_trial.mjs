// Test driver for real upstream engines. The paired Cryptex journal has a
// separate release-reader test. This fixture has only Syncthing's own config.
import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import * as fs from 'node:fs';
import path from 'node:path';
import { digest } from '../../tools/m0-lifecycle-compare/scripts/package-transaction.mjs';
import { assertColdSyncthingProfile } from '../../tools/m0-lifecycle-compare/scripts/syncthing-upgrade.mjs';

function checkSnapshot(root, entries) {
  const actual = fs.readdirSync(root, { recursive: true }).map(name => name.split(path.sep).join('/')).sort();
  assert.deepEqual(actual, entries.map(entry => entry.path).sort());
  for (const entry of entries) {
    const name = path.join(root, entry.path);
    const stat = fs.lstatSync(name);
    if (entry.directory) assert.ok(stat.isDirectory());
    else {
      assert.ok(stat.isFile() && stat.nlink === 1);
      assert.equal(digest(name), entry.sha256);
    }
  }
}

function isSqlite(file) {
  const header = Buffer.alloc(16);
  const fd = fs.openSync(file, 'r');
  try {
    return fs.readSync(fd, header, 0, 16, 0) === 16 && header.toString('ascii') === 'SQLite format 3\0';
  } finally { fs.closeSync(fd); }
}

export function stageOwnerEngine(testBinary, snapshot) {
  const { home, entries } = snapshot;
  assert.ok(entries.some(entry => !entry.directory && entry.path.startsWith('index-')
    && isSqlite(path.join(home, entry.path))), 'expected an actual Syncthing SQLite index');
  const root = path.dirname(home);
  const staged = path.join(root, 'staged-engine');
  fs.mkdirSync(staged, { mode: 0o700 });
  const test = 'managed_profile::exchange_install::tests::paired_release_handoff::stage_stopped_engine';
  const output = execFileSync(testBinary, [test, '--exact', '--ignored', '--test-threads=1'], {
    env: { ...process.env, CRYPTEX_M17_ENGINE_SCENARIO: root }, encoding: 'utf8', timeout: 60000,
  });
  assert.ok(output.includes('test ' + test + ' ... ok'), 'expected the native staging test to run');
  fs.copyFileSync(path.join(home, 'config.xml'), path.join(staged, 'config.xml'), fs.constants.COPYFILE_EXCL);
  checkSnapshot(staged, entries);
  checkSnapshot(home, entries);
  fs.renameSync(home, path.join(root, 'working-engine'));
  fs.renameSync(staged, home);
}

export function restoreOwnerEngine(snapshot, stopped, quarantine) {
  const { home, entries } = snapshot;
  assertColdSyncthingProfile(home, stopped);
  const retained = path.join(path.dirname(home), 'working-engine');
  checkSnapshot(retained, entries);
  fs.renameSync(home, quarantine);
  fs.renameSync(retained, home);
  checkSnapshot(home, entries);
  return { cold_baseline_restored: true, rejected_profile_retained: true };
}
