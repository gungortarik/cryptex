// Headless retained-binary filesystem experiment. Never executes Desktop or daemon.
import * as fs from 'node:fs';
import path from 'node:path';
import assert from 'node:assert/strict';
import { assertExperimentLocation, describePackage, digest, initialize, recover, selected, transition } from './package-transaction.mjs';
import { killTransactionAt } from './package-transaction-crash.mjs';

async function main() {
  const flags = ['--root', '--baseline-desktop', '--baseline-daemon', '--candidate-desktop', '--candidate-daemon'];
  const opts = {};
  const args = process.argv.slice(2);
  for (let index = 0; index < args.length; index += 2) {
    const flag = args[index]; const value = args[index + 1];
    if (!flags.includes(flag) || !value || value.startsWith('--') || Object.hasOwn(opts, flag)) throw new Error('Provide each documented path flag exactly once');
    if (!path.isAbsolute(value)) throw new Error('Use absolute paths');
    opts[flag] = value;
  }
  if (flags.some(flag => !opts[flag])) throw new Error(`Required: ${flags.join(' ')}`);
  const root = opts['--root']; assertExperimentLocation(root);
  const baselineSources = { desktop: opts['--baseline-desktop'], daemon: opts['--baseline-daemon'] };
  const candidateSources = { desktop: opts['--candidate-desktop'], daemon: opts['--candidate-daemon'] };
  const baseline = describePackage('baseline', baselineSources.desktop, baselineSources.daemon);
  const candidate = describePackage('candidate', candidateSources.desktop, candidateSources.daemon);
  fs.mkdirSync(root); // Fresh retained experiment required, never reuse/delete an earlier run.
  const fixture = path.join(root, 'data-fixture'); fs.mkdirSync(fixture);
  fs.writeFileSync(path.join(fixture, 'sealed-events.fixture'), 'synthetic immutable data: not production canonical events\n');
  fs.writeFileSync(path.join(fixture, 'blob.fixture'), Buffer.from([0, 255, 10, 42]));
  function snapshot() { return fs.readdirSync(fixture).sort().map(name => ({ name, sha256: digest(path.join(fixture, name)) })); }
  const fixtureBefore = snapshot();
  const results = [];
  function newCase(name) { const directory = path.join(root, name); initialize(directory, baseline, baselineSources); return directory; }
  const good = newCase('compatible');
  await transition(good, candidate, candidateSources, { health: async () => true });
  assert.equal(selected(good).active, 'candidate');
  results.push({ case: 'compatible', active: 'candidate' });
  const rejected = newCase('incompatible');
  await assert.rejects(transition(rejected, { ...candidate, desktop: { ...candidate.desktop, protocol: 2 } }, candidateSources, { health: async () => true }), error => error.code === 'INCOMPATIBLE_PAIR');
  assert.equal(selected(rejected).active, 'baseline');
  results.push({ case: 'incompatible', active: 'baseline' });
  const failed = newCase('failed-health');
  await assert.rejects(transition(failed, candidate, candidateSources, { health: async () => false }), error => error.code === 'HEALTH_FAILED');
  assert.equal(selected(failed).active, 'baseline');
  results.push({ case: 'failed-health', active: 'baseline' });
  for (const boundary of ['staged', 'intent', 'selected', 'healthy', 'committed']) {
    const directory = newCase(`crash-${boundary}`);
    const crash = await killTransactionAt(directory, candidate, candidateSources, boundary);
    const result = recover(directory);
    const expected = boundary === 'committed' ? 'candidate' : 'baseline';
    assert.equal(result.selection.active, expected);
    assert.deepEqual(recover(directory).selection, result.selection);
    results.push({ case: `crash-${boundary}`, ...crash, active: expected, recovered: result.recovered });
    assert.deepEqual(snapshot(), fixtureBefore);
  }
  assert.deepEqual(snapshot(), fixtureBefore);
  const report = { schema: 1, scope: 'serialized-filesystem-candidate-with-retained-binary-copies', platform: process.platform, node: process.version, baseline, candidate, health_scope: 'injected boolean, no application executed', data_scope: 'isolated synthetic fixtures, not production canonical events', fixture_before: fixtureBefore, fixture_after: snapshot(), cases: results, power_loss_durability_verified: false, production_integration_verified: false };
  fs.writeFileSync(path.join(root, 'report.json'), `${JSON.stringify(report, null, 2)}\n`);
  console.log(JSON.stringify(report, null, 2));
}

try { await main(); }
catch (error) {
  console.error(JSON.stringify({ scope: 'm0-package-filesystem-candidate', error: error.code ?? 'EXPERIMENT_FAILED', message: error.message }));
  process.exitCode = 2;
}
