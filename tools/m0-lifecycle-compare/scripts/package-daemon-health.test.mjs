import { test } from 'node:test';
import assert from 'node:assert/strict';
import * as fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { createFixtureRoot } from './package-transaction-fixtures.mjs';
import { probeDaemon } from './package-daemon-health.mjs';

function input(t) {
  const parent = createFixtureRoot();
  t.after(() => fs.rmSync(parent, { recursive: true, force: true }));
  return { helper: path.join(parent, 'unused-helper'), daemon: path.join(parent, 'unused-daemon'), root: path.join(parent, 'runtime'), protocol: 1 };
}
test('relative health helper rejected before process launch', async t => {
  const value = input(t);
  await assert.rejects(probeDaemon({ ...value, helper: 'relative' }), error => error.code === 'INVALID_HEALTH_INPUT');
  assert.equal(fs.existsSync(value.root), false);
});
test('relative runtime rejected before process launch', async t => {
  const value = input(t);
  await assert.rejects(probeDaemon({ ...value, root: 'relative' }), error => error.code === 'INVALID_HEALTH_INPUT');
});
test('protocol range rejected before process launch', async t => {
  const value = input(t);
  for (const protocol of [0, -1, 1.5, 0x100000000]) await assert.rejects(probeDaemon({ ...value, protocol }), error => error.code === 'INVALID_HEALTH_INPUT');
  assert.equal(fs.existsSync(value.root), false);
});
test('repository runtime rejected before process launch', async t => {
  const value = input(t);
  const root = fileURLToPath(new URL('./forbidden-health-runtime', import.meta.url));
  await assert.rejects(probeDaemon({ ...value, root }), error => error.code === 'REPOSITORY_ROOT');
  assert.equal(fs.existsSync(root), false);
});
