import { test } from 'node:test';
import assert from 'node:assert/strict';
import * as fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { spawnSync } from 'node:child_process';
import { createFixtureRoot } from './package-transaction-fixtures.mjs';
import { describeNativeTree, verifyNativeTree, stageNativeTree } from './package-native-tree.mjs';

function fixture(t) {
  const parent = createFixtureRoot();
  t.after(() => fs.rmSync(parent, { recursive: true, force: true }));
  const root = path.join(parent, 'source'); fs.mkdirSync(root);
  fs.mkdirSync(path.join(root, 'Resources')); fs.mkdirSync(path.join(root, 'Resources', 'empty'));
  const roles = { desktop: 'desktop', daemon: 'daemon' };
  for (const name of Object.values(roles)) { fs.writeFileSync(path.join(root, name), `synthetic ${name}\n`); fs.chmodSync(path.join(root, name), 0o755); }
  fs.writeFileSync(path.join(root, 'Resources', 'asset.txt'), 'synthetic resource\n');
  return { root, parent, roles, manifest: describeNativeTree(root, roles), destination: path.join(parent, 'staged') };
}
test('stages complete tree including resources and empty directories without source changes', t => {
  const { root, destination, manifest } = fixture(t);
  assert.equal(stageNativeTree(root, destination, manifest).staged, true);
  assert.deepEqual(describeNativeTree(destination, manifest.roles), manifest);
  assert.equal(verifyNativeTree(root, manifest).publisher_authenticated, false);
});
for (const mutation of ['missing', 'extra', 'modified', 'directory']) test(`refuses ${mutation} resource before staging`, t => {
  const { root, destination, manifest } = fixture(t);
  const asset = path.join(root, 'Resources', 'asset.txt');
  if (mutation === 'missing') fs.unlinkSync(asset);
  if (mutation === 'extra') fs.writeFileSync(path.join(root, 'extra'), 'extra');
  if (mutation === 'modified') fs.writeFileSync(asset, 'modified');
  if (mutation === 'directory') fs.rmdirSync(path.join(root, 'Resources', 'empty'));
  assert.throws(() => stageNativeTree(root, destination, manifest), { code: 'NATIVE_TREE_MISMATCH' });
  assert.equal(fs.existsSync(destination), false);
});
test('refuses target and schema mismatch', t => {
  const { root, manifest } = fixture(t);
  assert.throws(() => verifyNativeTree(root, { ...manifest, target: { ...manifest.target, arch: 'wrong' } }), { code: 'NATIVE_TARGET_MISMATCH' });
  assert.throws(() => verifyNativeTree(root, { ...manifest, schema: 2 }), { code: 'UNSUPPORTED_NATIVE_SCHEMA' });
});
test('refuses path traversal, Windows special names and duplicate case identities', t => {
  const { root, manifest } = fixture(t);
  for (const name of ['../outside', '/absolute', 'a\\b', 'NUL.txt', 'trailing.', 'a:b']) {
    const bad = structuredClone(manifest); bad.entries[0].path = name;
    assert.throws(() => verifyNativeTree(root, bad), { code: 'INVALID_NATIVE_PATH' });
  }
  const duplicate = structuredClone(manifest); duplicate.entries.push({ ...duplicate.entries[0], path: duplicate.entries[0].path.toUpperCase() });
  assert.throws(() => verifyNativeTree(root, duplicate), { code: 'NATIVE_PATH_COLLISION' });
});
test('refuses missing or identical binary roles and invalid protocol', t => {
  const { root, roles } = fixture(t);
  for (const bad of [{ desktop: 'missing', daemon: roles.daemon }, { desktop: roles.daemon, daemon: roles.daemon }]) assert.throws(() => describeNativeTree(root, bad), { code: 'INVALID_NATIVE_ROLES' });
  assert.throws(() => describeNativeTree(root, roles, 0), { code: 'INVALID_NATIVE_MANIFEST' });
});
test('refuses linked directory/junction including external payload', t => {
  const { root, parent, roles } = fixture(t);
  const external = path.join(parent, 'external'); fs.mkdirSync(external);
  fs.symlinkSync(external, path.join(root, 'linked'), process.platform === 'win32' ? 'junction' : 'dir');
  assert.throws(() => describeNativeTree(root, roles), { code: 'UNSAFE_NATIVE_ENTRY' });
});
test('refuses existing destination without changing its bytes', t => {
  const { root, destination, manifest } = fixture(t); fs.mkdirSync(destination);
  const canary = path.join(destination, 'canary'); fs.writeFileSync(canary, 'keep');
  assert.throws(() => stageNativeTree(root, destination, manifest), { code: 'NATIVE_DESTINATION_EXISTS' });
  assert.equal(fs.readFileSync(canary, 'utf8'), 'keep');
});
test('refuses overlapping source/destination and repository destination', t => {
  const { root, manifest } = fixture(t);
  assert.throws(() => stageNativeTree(root, path.join(root, 'nested'), manifest), { code: 'OVERLAPPING_NATIVE_ROOTS' });
  assert.throws(() => stageNativeTree(root, process.cwd(), manifest), { code: 'REPOSITORY_ROOT' });
});
test('manifest requires parent directories and valid file digests/modes', t => {
  const { root, manifest } = fixture(t);
  const bad = structuredClone(manifest); bad.entries = bad.entries.filter(e => e.path !== 'Resources');
  assert.throws(() => verifyNativeTree(root, bad), { code: 'INVALID_NATIVE_MANIFEST' });
  for (const change of [{ sha256: 'bad' }, { size: -1 }, { mode: process.platform === 'win32' ? 0o755 : 0o7777 }]) {
    const invalid = structuredClone(manifest); Object.assign(invalid.entries.find(e => e.kind === 'file'), change);
    assert.throws(() => verifyNativeTree(root, invalid), { code: 'INVALID_NATIVE_MANIFEST' });
  }
});
test('POSIX execution permissions are retained and required', t => {
  const { root, destination, roles, manifest } = fixture(t);
  stageNativeTree(root, destination, manifest);
  if (process.platform !== 'win32') {
    assert.equal(fs.statSync(path.join(destination, roles.desktop)).mode & 0o777, 0o755);
    fs.chmodSync(path.join(root, roles.desktop), 0o644);
    assert.throws(() => describeNativeTree(root, roles), { code: 'NATIVE_NOT_EXECUTABLE' });
  } else assert.equal(manifest.entries.every(e => e.mode === null), true);
});
test('copy I/O failure retains partial tree with typed diagnostics', t => {
  const { root, destination, manifest } = fixture(t);
  const failure = Object.assign(new Error('injected copy failure'), { code: 'EIO' });
  assert.throws(() => stageNativeTree(root, destination, manifest, { copyFileSync() { throw failure; } }), error => {
    assert.equal(error.code, 'NATIVE_STAGE_FAILED'); assert.equal(error.cause, failure);
    assert.deepEqual(error.diagnostics, { phase: 'stage', destination_retained: true, cause_code: 'EIO' }); return true;
  });
  assert.equal(fs.existsSync(destination), true);
  assert.equal(verifyNativeTree(root, manifest).verified, true);
  assert.throws(() => stageNativeTree(root, destination, manifest), { code: 'NATIVE_DESTINATION_EXISTS' });
});
test('source mutation during copy refuses acceptance and retains partial tree', t => {
  const { root, destination, manifest } = fixture(t); let changed = false;
  assert.throws(() => stageNativeTree(root, destination, manifest, { copyFileSync(...args) {
    if (!changed) { changed = true; fs.writeFileSync(path.join(root, 'Resources', 'asset.txt'), 'changed during staging'); }
    fs.copyFileSync(...args);
  } }), error => error.code === 'NATIVE_STAGE_FAILED' && error.cause.code === 'NATIVE_TREE_MISMATCH');
  assert.equal(fs.existsSync(destination), true);
});
function runProof(root, source, roles) {
  const script = fileURLToPath(new URL('./run-native-tree-proof.mjs', import.meta.url));
  return spawnSync(process.execPath, [script, '--source', source, '--root', root, '--desktop', roles.desktop, '--daemon', roles.daemon], { windowsHide: true, encoding: 'utf8', timeout: 5000 });
}
test('proof runner records complete inventory and explicit unverified handover scope', t => {
  const { root, parent, roles, manifest } = fixture(t); const proof = path.join(parent, 'proof');
  const result = runProof(proof, root, roles); assert.equal(result.status, 0, result.stderr);
  const report = JSON.parse(fs.readFileSync(path.join(proof, 'native-tree-report.json'), 'utf8'));
  assert.equal(report.handover_verified, false); assert.equal(report.binaries_executed, false);
  assert.equal(report.publisher_authenticated, false); assert.deepEqual(report.manifest, manifest);
  assert.equal(verifyNativeTree(root, manifest).verified, true);
});
test('proof runner refuses existing root without overwriting files', t => {
  const { root, parent, roles } = fixture(t); const proof = path.join(parent, 'proof'); fs.mkdirSync(proof);
  const canary = path.join(proof, 'experiment.json'); fs.writeFileSync(canary, 'keep');
  const result = runProof(proof, root, roles); assert.equal(result.status, 2);
  assert.equal(JSON.parse(result.stderr).code, 'EEXIST'); assert.equal(fs.readFileSync(canary, 'utf8'), 'keep');
});
test('proof runner rejects overlap before writing marker or payload', t => {
  const { root, roles, manifest } = fixture(t); const proof = path.join(root, 'proof');
  const result = runProof(proof, root, roles); assert.equal(result.status, 2);
  assert.equal(JSON.parse(result.stderr).code, 'OVERLAPPING_NATIVE_ROOTS');
  assert.equal(fs.existsSync(proof), false); assert.equal(verifyNativeTree(root, manifest).verified, true);
});
