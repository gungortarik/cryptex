// Actual Desktop sessions with same or explicitly changed native package inputs.
import * as fs from 'node:fs';
import path from 'node:path';
import { PackageError } from './package-transaction.mjs';
import { withNativeHandoverGate } from './package-handover-gate.mjs';
import { prepareNativeSlots, startNativeSelected, handoverNativeSelected } from './package-native-selection.mjs';
import { selectionProofInputs } from './native-selection-proof-inputs.mjs';
import { createGoldenFoundation, inventoryGolden } from './m0-foundation.mjs';

async function main() {
  const inputs = selectionProofInputs(process.argv.slice(2));
  const { root, source, candidateSource, helper, baselineManifest, candidateManifest } = inputs;
  fs.mkdirSync(root); fs.writeFileSync(path.join(root, 'experiment.json'), JSON.stringify({ schema: 1, scope: 'm0-native-handover-pilot' }), { flag: 'wx' });
  const goldenRoot = path.join(root, 'golden-brain');
  const benchmark = path.join(path.dirname(helper), `m0-storage-bench${process.platform === 'win32' ? '.exe' : ''}`);
  await createGoldenFoundation(goldenRoot, benchmark); const goldenBefore = inventoryGolden(goldenRoot);
  const scenarios = [];
  for (const [name, protocol, expected] of [['compatible', 1, 'committed'], ['protocol-reject', 2, 'rolled-back']]) {
    const scenarioRoot = path.join(root, name); fs.mkdirSync(scenarioRoot);
    fs.writeFileSync(path.join(scenarioRoot, 'experiment.json'), JSON.stringify({ schema: 1, scope: 'm0-native-handover-pilot' }), { flag: 'wx' });
    await withNativeHandoverGate(scenarioRoot, helper, async lease => {
      prepareNativeSlots(lease, { source, manifest: baselineManifest }, { source: candidateSource, manifest: { ...candidateManifest, protocol } });
      let active = await startNativeSelected({ lease, helper, runtimeName: 'baseline-first' });
      const baseline = { desktop_pid: active.session.desktop_pid, daemon_pid: active.session.daemon_pid, health: active.session.health,
        desktop_identity: active.session.desktop_identity, daemon_identity: active.session.daemon_identity };
      try {
        active = await handoverNativeSelected({ lease, helper, active, runtimePrefix: name });
        if (active.outcome !== expected) throw new PackageError('UNEXPECTED_HANDOVER_OUTCOME', 'Scenario outcome mismatch');
        scenarios.push({ name, outcome: active.outcome, selection: active.selection, baseline, baseline_cleanup: active.baseline_cleanup,
          rejection: active.rejection ?? null, desktop_pid: active.session.desktop_pid, daemon_pid: active.session.daemon_pid,
          desktop_identity: active.session.desktop_identity, daemon_identity: active.session.daemon_identity, health: active.session.health });
      } finally {
        const cleanup = await active.session.stop();
        if (scenarios.at(-1)?.name === name) scenarios.at(-1).cleanup = cleanup;
      }
    });
    if (JSON.stringify(inventoryGolden(goldenRoot)) !== JSON.stringify(goldenBefore)) throw new PackageError('GOLDEN_FIXTURE_CHANGED', 'Native selection/rollback changed synthetic sealed fixture');
  }
  const report = { schema: 1, scope: 'm0-native-selection-proof', platform: process.platform, arch: process.arch, node: process.version, scenarios,
    same_source_bytes: !Object.values(inputs.changedRoles).some(Boolean), changed_native_package_verified: Object.values(inputs.changedRoles).some(Boolean),
    changed_roles: inputs.changedRoles, input_digests: { baseline: inputs.baselineDigests, candidate: inputs.candidateDigests },
    synthetic_sealed_fixture_preserved: true, version_upgrade_verified: false, interrupted_recovery_verified: false, publisher_authenticated: false, canonical_data_acceptance: false };
  fs.writeFileSync(path.join(root, 'native-selection-report.json'), `${JSON.stringify(report, null, 2)}\n`, { flag: 'wx' });
  console.log(JSON.stringify(report));
}
try { await main(); }
catch (error) { console.error(JSON.stringify({ code: error.code ?? 'NATIVE_SELECTION_PROOF_FAILED', diagnostics: error.diagnostics ?? null, cause: error.cause?.code ?? null })); process.exitCode = 2; }
