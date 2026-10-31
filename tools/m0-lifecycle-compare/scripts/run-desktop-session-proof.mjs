// Pilot-enabled current-source packages only. No legacy binary capability inference.
import * as fs from 'node:fs';
import path from 'node:path';
import { assertExperimentLocation, PackageError } from './package-transaction.mjs';
import { describeNativeTree, stageNativeTree } from './package-native-tree.mjs';
import { withNativeHandoverGate } from './package-handover-gate.mjs';
import { startOwnedDesktop } from './package-desktop-session.mjs';

async function main() {
  const values = {}; const args = process.argv.slice(2);
  const flags = ['--source', '--root', '--helper', '--desktop', '--daemon'];
  for (let i = 0; i < args.length; i += 2) {
    if (!flags.includes(args[i]) || !args[i + 1] || Object.hasOwn(values, args[i])) throw new PackageError('INVALID_SESSION_PROOF_ARGS', 'Five unique documented flags required');
    values[args[i]] = args[i + 1];
  }
  if (flags.some(flag => !values[flag]) || ['--source', '--root', '--helper'].some(flag => !path.isAbsolute(values[flag]))) throw new PackageError('INVALID_SESSION_PROOF_ARGS', 'Absolute package, fresh root and helper required');
  const source = values['--source']; const root = values['--root']; assertExperimentLocation(root);
  const manifest = describeNativeTree(source, { desktop: values['--desktop'], daemon: values['--daemon'] });
  for (const [parent, child] of [[source, root], [root, source]]) {
    const relative = path.relative(parent, child);
    if (!relative || (!relative.startsWith(`..${path.sep}`) && relative !== '..' && !path.isAbsolute(relative))) throw new PackageError('OVERLAPPING_NATIVE_ROOTS', 'Package and pilot root must be disjoint');
  }
  fs.mkdirSync(root);
  fs.writeFileSync(path.join(root, 'experiment.json'), JSON.stringify({ schema: 1, scope: 'm0-native-handover-pilot' }), { flag: 'wx' });
  const payload = path.join(root, 'payload'); stageNativeTree(source, payload, manifest);
  const sessions = [];
  await withNativeHandoverGate(root, values['--helper'], async lease => {
    for (const runtimeName of ['baseline-first', 'baseline-reopen']) {
      const session = await startOwnedDesktop({ lease, helper: values['--helper'], payload, manifest, runtimeName });
      try { lease.assertHeld(); }
      finally {
        const cleanup = await session.stop();
        sessions.push({ runtime: session.root, desktop_pid: session.desktop_pid, daemon_pid: session.daemon_pid,
          health: session.health, desktop_identity: session.desktop_identity, daemon_identity: session.daemon_identity, cleanup });
      }
      lease.assertHeld();
    }
  });
  const report = { schema: 1, scope: 'm0-owned-desktop-session', platform: process.platform, arch: process.arch, node: process.version, manifest, sessions,
    same_package_reopened: true, version_upgrade_verified: false, slot_activation_verified: false, publisher_authenticated: false, canonical_data_acceptance: false };
  fs.writeFileSync(path.join(root, 'desktop-session-report.json'), `${JSON.stringify(report, null, 2)}\n`, { flag: 'wx' });
  console.log(JSON.stringify(report));
}
try { await main(); }
catch (error) {
  const failures = error.cause instanceof AggregateError ? error.cause.errors.map(cause => cause.code ?? 'UNKNOWN') : null;
  console.error(JSON.stringify({ scope: 'm0-owned-desktop-session', code: error.code ?? 'SESSION_PROOF_FAILED', diagnostics: error.diagnostics ?? null, failures })); process.exitCode = 2;
}
