// Actual Desktop sessions, same source bytes: selection/rollback proof, not upgrade.
import * as fs from 'node:fs';
import path from 'node:path';
import { PackageError, assertExperimentLocation } from './package-transaction.mjs';
import { describeNativeTree } from './package-native-tree.mjs';
import { withNativeHandoverGate } from './package-handover-gate.mjs';
import { prepareNativeSlots, startNativeSelected, handoverNativeSelected } from './package-native-selection.mjs';

async function main() {
  const flags = ['--source', '--root', '--helper', '--desktop', '--daemon']; const values = {}; const args = process.argv.slice(2);
  for (let index = 0; index < args.length; index += 2) {
    if (!flags.includes(args[index]) || !args[index + 1] || Object.hasOwn(values, args[index])) throw new PackageError('INVALID_SELECTION_PROOF_ARGS', 'Five unique flags required');
    values[args[index]] = args[index + 1];
  }
  if (flags.some(flag => !values[flag]) || ['--source', '--root', '--helper'].some(flag => !path.isAbsolute(values[flag]))) throw new PackageError('INVALID_SELECTION_PROOF_ARGS', 'Absolute input, fresh root and helper required');
  const root = values['--root']; const source = values['--source']; assertExperimentLocation(root);
  for (const [parent, child] of [[root, source], [source, root]]) {
    const relative = path.relative(parent, child);
    if (!relative || (!relative.startsWith(`..${path.sep}`) && relative !== '..' && !path.isAbsolute(relative))) throw new PackageError('OVERLAPPING_NATIVE_ROOTS', 'Disjoint input and proof root required');
  }
  const roles = { desktop: values['--desktop'], daemon: values['--daemon'] }; const manifest = describeNativeTree(source, roles);
  fs.mkdirSync(root); fs.writeFileSync(path.join(root, 'experiment.json'), JSON.stringify({ schema: 1, scope: 'm0-native-handover-pilot' }), { flag: 'wx' });
  const scenarios = [];
  for (const [name, protocol, expected] of [['compatible', 1, 'committed'], ['protocol-reject', 2, 'rolled-back']]) {
    const scenarioRoot = path.join(root, name); fs.mkdirSync(scenarioRoot);
    fs.writeFileSync(path.join(scenarioRoot, 'experiment.json'), JSON.stringify({ schema: 1, scope: 'm0-native-handover-pilot' }), { flag: 'wx' });
    await withNativeHandoverGate(scenarioRoot, values['--helper'], async lease => {
      prepareNativeSlots(lease, { source, manifest }, { source, manifest: describeNativeTree(source, roles, protocol) });
      let active = await startNativeSelected({ lease, helper: values['--helper'], runtimeName: 'baseline-first' });
      const baseline = { desktop_pid: active.session.desktop_pid, daemon_pid: active.session.daemon_pid, health: active.session.health };
      try {
        active = await handoverNativeSelected({ lease, helper: values['--helper'], active, runtimePrefix: name });
        if (active.outcome !== expected) throw new PackageError('UNEXPECTED_HANDOVER_OUTCOME', 'Scenario outcome mismatch');
        scenarios.push({ name, outcome: active.outcome, selection: active.selection, baseline, baseline_cleanup: active.baseline_cleanup,
          rejection: active.rejection ?? null, desktop_pid: active.session.desktop_pid, daemon_pid: active.session.daemon_pid,
          desktop_identity: active.session.desktop_identity, daemon_identity: active.session.daemon_identity, health: active.session.health });
      } finally {
        const cleanup = await active.session.stop();
        if (scenarios.at(-1)?.name === name) scenarios.at(-1).cleanup = cleanup;
      }
    });
  }
  const report = { schema: 1, scope: 'm0-native-selection-proof', platform: process.platform, arch: process.arch, node: process.version, scenarios,
    same_source_bytes: true, version_upgrade_verified: false, interrupted_recovery_verified: false, publisher_authenticated: false, canonical_data_acceptance: false };
  fs.writeFileSync(path.join(root, 'native-selection-report.json'), `${JSON.stringify(report, null, 2)}\n`, { flag: 'wx' });
  console.log(JSON.stringify(report));
}
try { await main(); }
catch (error) { console.error(JSON.stringify({ code: error.code ?? 'NATIVE_SELECTION_PROOF_FAILED', diagnostics: error.diagnostics ?? null, cause: error.cause?.code ?? null })); process.exitCode = 2; }
