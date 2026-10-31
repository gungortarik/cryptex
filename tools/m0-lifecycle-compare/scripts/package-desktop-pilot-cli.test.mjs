// Negative pre-Tauri checks only: no visible window or daemon is started.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import * as fs from 'node:fs';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import { createFixtureRoot } from './package-transaction-fixtures.mjs';

const binary = process.env.CRYPTEX_PILOT_DESKTOP_TEST_BINARY;
function run(args) {
  assert.equal(typeof binary, 'string', 'Set CRYPTEX_PILOT_DESKTOP_TEST_BINARY to a current-source release Desktop');
  assert.equal(path.isAbsolute(binary), true);
  const result = spawnSync(binary, args, { windowsHide: true, encoding: 'utf8', timeout: 5000 });
  assert.equal(result.error, undefined);
  assert.equal(result.status, 2, result.stderr);
  assert.match(result.stderr, /PILOT_RUNTIME_REJECTED/);
}
function fixture(t) {
  const parent = createFixtureRoot(); t.after(() => fs.rmSync(parent, { recursive: true, force: true }));
  const root = path.join(parent, 'runtime'); fs.mkdirSync(root);
  fs.writeFileSync(path.join(root, 'pilot-runtime.marker'), 'm0-native-desktop-runtime-v1\n');
  return { parent, root };
}
test('Desktop pilot rejects missing and duplicate runtime flags before Tauri startup', t => {
  const { root } = fixture(t);
  run(['--cryptex-pilot-runtime']);
  run(['--cryptex-pilot-runtime', root, '--cryptex-pilot-runtime', root]);
  assert.deepEqual(fs.readdirSync(root), ['pilot-runtime.marker']);
});
test('Desktop pilot rejects reused runtime and wrong ownership marker', t => {
  const { root } = fixture(t); fs.writeFileSync(path.join(root, 'ready'), 'old');
  run(['--cryptex-pilot-runtime', root]);
  fs.unlinkSync(path.join(root, 'ready')); fs.writeFileSync(path.join(root, 'pilot-runtime.marker'), 'wrong');
  run(['--cryptex-pilot-runtime', root]);
  assert.equal(fs.readFileSync(path.join(root, 'pilot-runtime.marker'), 'utf8'), 'wrong');
});
test('Desktop pilot rejects actual directory junction/symlink root', t => {
  const { parent, root } = fixture(t); const linked = path.join(parent, 'linked');
  fs.symlinkSync(root, linked, process.platform === 'win32' ? 'junction' : 'dir');
  run(['--cryptex-pilot-runtime', linked]);
  assert.deepEqual(fs.readdirSync(root), ['pilot-runtime.marker']);
});
test('Desktop pilot rejects repository and relative runtime roots', () => {
  run(['--cryptex-pilot-runtime', process.cwd()]);
  run(['--cryptex-pilot-runtime', 'relative']);
});
