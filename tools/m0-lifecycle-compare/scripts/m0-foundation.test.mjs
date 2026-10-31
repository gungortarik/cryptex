import { test } from 'node:test';
import assert from 'node:assert/strict';
import * as fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { createFixtureRoot } from './package-transaction-fixtures.mjs';
import { createGoldenFoundation, inventoryGolden, doctorFoundation } from './m0-foundation.mjs';
const binary = name => fileURLToPath(new URL(`../../../target/debug/${name}${process.platform === 'win32' ? '.exe' : ''}`, import.meta.url));
function fixture(t) { const root = createFixtureRoot(); t.after(() => fs.rmSync(root, { recursive: true, force: true })); return root; }
test('Golden foundation creates deterministic sealed synthetic packs', async t => {
  const root = fixture(t); const first = await createGoldenFoundation(path.join(root, 'first'), binary('m0-storage-bench'));
  const second = await createGoldenFoundation(path.join(root, 'second'), binary('m0-storage-bench'));
  assert.deepEqual(first, second); assert.equal(first.synthetic_only, true); assert.equal(first.production_event_schema, false);
  assert.equal(first.entries.filter(entry => entry.kind === 'file').length, 2);
});
test('Golden foundation refuses reused or repository roots', async t => {
  const root = fixture(t); await assert.rejects(createGoldenFoundation(root, binary('m0-storage-bench')), { code: 'EEXIST' });
  await assert.rejects(createGoldenFoundation(fileURLToPath(new URL('../../../forbidden-fixture', import.meta.url)), binary('m0-storage-bench')), { code: 'REPOSITORY_ROOT' });
});
test('Golden inventory detects byte and membership changes', t => {
  const root = fixture(t); fs.writeFileSync(path.join(root, 'pack'), 'original'); const before = inventoryGolden(root);
  fs.writeFileSync(path.join(root, 'pack'), 'changed'); assert.notDeepEqual(inventoryGolden(root), before);
  fs.mkdirSync(path.join(root, 'empty')); assert.equal(inventoryGolden(root).some(entry => entry.kind === 'directory'), true);
});
test('Doctor reports actual absent IPC without claiming unimplemented checks pass', async t => {
  const root = fixture(t); const result = await doctorFoundation({ helper: binary('m0-lifecycle-compare'), runtimeRoot: root });
  assert.equal(result.state, 'daemon-absent'); assert.equal(result.actual_ipc_probe, true);
  assert.equal(result.canonical_store_check, 'not-implemented'); assert.equal(result.syncthing_check, 'not-implemented');
});
test('Doctor rejects malformed ready PID rather than trusting file presence', async t => {
  const root = fixture(t); fs.writeFileSync(path.join(root, 'ready'), 'invalid');
  await assert.rejects(doctorFoundation({ helper: binary('m0-lifecycle-compare'), runtimeRoot: root }), { code: 'INVALID_DOCTOR_READY' });
});
test('Doctor reports failed actual probe as attention and omits raw subprocess output', async t => {
  const root = fixture(t); const result = await doctorFoundation({ helper: process.execPath, runtimeRoot: root });
  assert.equal(result.state, 'attention'); assert.equal(Object.hasOwn(result, 'stderr'), false);
});
