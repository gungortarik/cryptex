// Headless real daemon health integration. Desktop bytes are staged, never launched.
import * as fs from 'node:fs';
import path from 'node:path';
import assert from 'node:assert/strict';
import { assertExperimentLocation, describePackage, digest, initialize, selected, transition } from './package-transaction.mjs';
import { probeDaemon } from './package-daemon-health.mjs';

async function main() {
  const flags = ['--root', '--health-helper', '--baseline-desktop', '--baseline-daemon', '--candidate-desktop', '--candidate-daemon'];
  const opts = {}; const args = process.argv.slice(2);
  for (let index = 0; index < args.length; index += 2) {
    const flag = args[index]; const value = args[index + 1];
    if (!flags.includes(flag) || !value || !path.isAbsolute(value) || Object.hasOwn(opts, flag)) throw new Error('Each documented flag needs one absolute path');
    opts[flag] = value;
  }
  if (flags.some(flag => !opts[flag])) throw new Error(`Required: ${flags.join(' ')}`);
  const root = opts['--root']; assertExperimentLocation(root);
  const helper = opts['--health-helper'];
  const oldSources = { desktop: opts['--baseline-desktop'], daemon: opts['--baseline-daemon'] };
  const newSources = { desktop: opts['--candidate-desktop'], daemon: opts['--candidate-daemon'] };
  const oldManifest = describePackage('baseline', oldSources.desktop, oldSources.daemon);
  const newManifest = describePackage('candidate', newSources.desktop, newSources.daemon);
  fs.mkdirSync(root);
  const fixture = path.join(root, 'synthetic-data.fixture');
  fs.writeFileSync(fixture, 'isolated immutable bytes, not production canonical events\n');
  const before = digest(fixture); const cases = [];
  async function health(directory, slot, protocol, label) {
    return probeDaemon({ helper, daemon: path.join(slot, 'daemon.bin'), root: path.join(directory, label), protocol });
  }
  const good = path.join(root, 'compatible'); initialize(good, oldManifest, oldSources);
  const initial = await health(good, path.join(good, 'slots', 'baseline'), 1, 'baseline-runtime');
  let candidateHealth;
  await transition(good, newManifest, newSources, { health: async (slot, manifest) => { candidateHealth = await health(good, slot, manifest.daemon.protocol, 'candidate-runtime'); return true; } });
  assert.equal(selected(good).active, 'candidate');
  cases.push({ case: 'compatible', initial, candidate: candidateHealth, active: 'candidate' });
  const brokenDaemon = path.join(root, 'invalid-daemon.fixture'); fs.writeFileSync(brokenDaemon, 'not an executable\n');
  const failures = [
    { name: 'invalid-binary', sources: { ...newSources, daemon: brokenDaemon }, manifest: describePackage('broken', newSources.desktop, brokenDaemon) },
    { name: 'actual-protocol-mismatch', sources: newSources, manifest: describePackage('wrong-protocol', newSources.desktop, newSources.daemon, 2, 2) },
  ];
  for (const entry of failures) {
    const directory = path.join(root, entry.name); initialize(directory, oldManifest, oldSources);
    let failure = null;
    await assert.rejects(transition(directory, entry.manifest, entry.sources, { health: async (slot, manifest) => {
      try { await health(directory, slot, manifest.daemon.protocol, 'candidate-runtime'); return true; }
      catch (error) { failure = { code: error.code, message: error.message, stderr: error.stderr ?? null, cleanup: error.cleanup ?? null }; throw error; }
    } }), error => error.code === 'HEALTH_FAILED');
    assert.notEqual(failure, null);
    assert.equal(selected(directory).active, 'baseline');
    const restored = await health(directory, path.join(directory, 'slots', 'baseline'), 1, 'restored-runtime');
    cases.push({ case: entry.name, failure, active: 'baseline', restored });
    assert.equal(digest(fixture), before);
  }
  const report = { schema: 1, scope: 'package-candidate-real-daemon-shared-desktop-ipc', node: process.version, platform: process.platform,
    baseline: oldManifest, candidate: newManifest, cases, fixture_before: before, fixture_after: digest(fixture), desktop_gui_executed: false,
    installed_updater_integrated: false, canonical_data_acceptance: false };
  assert.equal(report.fixture_before, report.fixture_after);
  fs.writeFileSync(path.join(root, 'real-health-report.json'), `${JSON.stringify(report, null, 2)}\n`);
  console.log(JSON.stringify(report, null, 2));
}
try { await main(); }
catch (error) { console.error(JSON.stringify({ scope: 'm0-package-real-health', code: error.code ?? 'EXPERIMENT_FAILED', message: error.message, stderr: error.stderr ?? null })); process.exitCode = 2; }
