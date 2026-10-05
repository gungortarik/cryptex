import { test } from 'node:test';
import assert from 'node:assert/strict';
import * as fs from 'node:fs';
import path from 'node:path';
import { createFixtureRoot } from './package-transaction-fixtures.mjs';
import { stageSyncthingBundle, verifySyncthingBundle } from './syncthing-bundle.mjs';
function fixture(t) {
  const root = createFixtureRoot();
  t.after(() => fs.rmSync(root, { recursive: true, force: true }));
  const binary = path.join(root, 'native-input'); fs.writeFileSync(binary, 'synthetic-native-bytes', { mode: 0o700 });
  return { root, binary, bundle: path.join(root, 'bundle') };
}
test('bundle retains exact native bytes, license and attribution outside source tree', t => {
  const input = fixture(t); const staged = stageSyncthingBundle(input.binary, input.bundle);
  assert.equal(fs.readFileSync(staged.binary, 'utf8'), 'synthetic-native-bytes');
  assert.equal(staged.manifest.entries.length, 3); assert.equal(staged.manifest.publisher_authenticated, false);
  assert.deepEqual(verifySyncthingBundle(input.bundle), staged);
});
test('bundle refuses reused and relative roots', t => {
  const input = fixture(t); stageSyncthingBundle(input.binary, input.bundle);
  assert.throws(() => stageSyncthingBundle(input.binary, input.bundle), { code: 'EEXIST' });
  assert.throws(() => stageSyncthingBundle(input.binary, 'relative'), { code: 'INVALID_SYNCTHING_BUNDLE_ARGS' });
});
test('changed bundled binary refuses launch resolution', t => {
  const input = fixture(t); const staged = stageSyncthingBundle(input.binary, input.bundle);
  fs.appendFileSync(staged.binary, 'tampered');
  assert.throws(() => verifySyncthingBundle(input.bundle), { code: 'SYNCTHING_BUNDLE_DIGEST_MISMATCH' });
});
test('changed notice and unexpected member refuse resolution', t => {
  const input = fixture(t); stageSyncthingBundle(input.binary, input.bundle);
  const license = path.join(input.bundle, 'LICENSE.txt'); const original = fs.readFileSync(license);
  fs.appendFileSync(license, 'tampered');
  assert.throws(() => verifySyncthingBundle(input.bundle), { code: 'SYNCTHING_BUNDLE_DIGEST_MISMATCH' });
  fs.writeFileSync(license, original); fs.writeFileSync(path.join(input.bundle, 'extra'), 'unexpected');
  assert.throws(() => verifySyncthingBundle(input.bundle), { code: 'INVALID_SYNCTHING_BUNDLE' });
});
test('foreign platform or binary role cannot be silently adopted', t => {
  const input = fixture(t); const staged = stageSyncthingBundle(input.binary, input.bundle);
  const manifest = path.join(input.bundle, 'bundle.json');
  staged.manifest.binary = '../escape'; fs.writeFileSync(manifest, JSON.stringify(staged.manifest));
  assert.throws(() => verifySyncthingBundle(input.bundle), { code: 'INVALID_SYNCTHING_BUNDLE' });
});
test('directory input and linked bundle root are refused', t => {
  const input = fixture(t);
  assert.throws(() => stageSyncthingBundle(input.root, input.bundle), { code: 'UNSAFE_SYNCTHING_BUNDLE_FILE' });
  stageSyncthingBundle(input.binary, input.bundle);
  const linked = path.join(input.root, 'linked-bundle');
  fs.symlinkSync(input.bundle, linked, process.platform === 'win32' ? 'junction' : 'dir');
  assert.throws(() => verifySyncthingBundle(linked), { code: 'UNSAFE_PATH' });
});
test('null and malformed entry metadata report typed corruption refusal', t => {
  const input = fixture(t); stageSyncthingBundle(input.binary, input.bundle);
  const file = path.join(input.bundle, 'bundle.json'); const original = JSON.parse(fs.readFileSync(file, 'utf8'));
  for (const value of [null, { ...original, entries: [null, null, null] }]) {
    fs.writeFileSync(file, JSON.stringify(value));
    assert.throws(() => verifySyncthingBundle(input.bundle), { code: 'INVALID_SYNCTHING_BUNDLE' });
  }
  fs.writeFileSync(file, '{broken');
  assert.throws(() => verifySyncthingBundle(input.bundle), { code: 'CORRUPT_SYNCTHING_BUNDLE' });
});
if (process.platform !== 'win32') test('POSIX input and copied executable modes are enforced', t => {
  const input = fixture(t); fs.chmodSync(input.binary, 0o600);
  assert.throws(() => stageSyncthingBundle(input.binary, input.bundle), { code: 'SYNCTHING_BINARY_NOT_EXECUTABLE' });
  fs.chmodSync(input.binary, 0o700); const staged = stageSyncthingBundle(input.binary, input.bundle);
  fs.chmodSync(staged.binary, 0o600);
  assert.throws(() => verifySyncthingBundle(input.bundle), { code: 'SYNCTHING_BUNDLE_MODE_MISMATCH' });
});
