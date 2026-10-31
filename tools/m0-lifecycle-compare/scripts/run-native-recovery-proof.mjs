// Actual baseline quiescence, updater SIGKILL before successor launch, restart.
import * as fs from 'node:fs';
import path from 'node:path';
import { spawn } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import { once } from 'node:events';
import { setTimeout as pause } from 'node:timers/promises';
import { PackageError, assertExperimentLocation } from './package-transaction.mjs';
import { describeNativeTree } from './package-native-tree.mjs';
import { acquireNativeHandoverGate } from './package-handover-gate.mjs';
import { recoverNativeSelected } from './package-native-selection.mjs';
import { createGoldenFoundation, inventoryGolden, doctorFoundation } from './m0-foundation.mjs';

async function main() {
  const flags = ['--source', '--root', '--helper', '--desktop', '--daemon']; const args = process.argv.slice(2); const values = {};
  for (let index = 0; index < args.length; index += 2) {
    if (!flags.includes(args[index]) || !args[index + 1] || Object.hasOwn(values, args[index])) throw new PackageError('INVALID_RECOVERY_PROOF_ARGS', 'Five unique flags required');
    values[args[index]] = args[index + 1];
  }
  if (flags.some(flag => !values[flag]) || ['--source', '--root', '--helper'].some(flag => !path.isAbsolute(values[flag]))) throw new PackageError('INVALID_RECOVERY_PROOF_ARGS', 'Absolute source/root/helper required');
  const root = values['--root']; const source = values['--source']; const helper = values['--helper']; assertExperimentLocation(root);
  describeNativeTree(source, { desktop: values['--desktop'], daemon: values['--daemon'] });
  for (const [parent, child] of [[source, root], [root, source]]) {
    const relative = path.relative(parent, child);
    if (!relative || (!relative.startsWith(`..${path.sep}`) && relative !== '..' && !path.isAbsolute(relative))) throw new PackageError('OVERLAPPING_NATIVE_ROOTS', 'Disjoint input/root required');
  }
  fs.mkdirSync(root); fs.writeFileSync(path.join(root, 'experiment.json'), JSON.stringify({ schema: 1, scope: 'm0-native-handover-pilot' }), { flag: 'wx' });
  const goldenRoot = path.join(root, 'golden-brain');
  const benchmark = path.join(path.dirname(helper), `m0-storage-bench${process.platform === 'win32' ? '.exe' : ''}`);
  const golden = await createGoldenFoundation(goldenRoot, benchmark);
  const goldenBefore = inventoryGolden(goldenRoot);
  const worker = fileURLToPath(new URL('./native-selection-interruption-worker.mjs', import.meta.url));
  const child = spawn(process.execPath, [worker, root, helper, source, values['--desktop'], values['--daemon']], { windowsHide: true, stdio: ['ignore', 'pipe', 'pipe'] });
  let closed = false; const exited = once(child, 'close').then(outcome => { closed = true; return outcome; });
  let boundary;
  try {
    boundary = await new Promise((resolve, reject) => {
      const timer = setTimeout(() => reject(new PackageError('RECOVERY_BOUNDARY_TIMEOUT', 'Updater did not reach selected boundary')), 20000);
      let output = ''; child.stdout.on('data', chunk => {
        output += chunk;
        if (output.length > 8192) { clearTimeout(timer); reject(new PackageError('INVALID_RECOVERY_BOUNDARY', 'Oversized boundary')); return; }
        if (output.includes('\n')) {
          clearTimeout(timer);
          try {
            const value = JSON.parse(output.split('\n')[0]);
            if (value.phase !== 'selected-before-candidate-launch' || value.pid !== child.pid) throw new Error('boundary mismatch');
            resolve(value);
          } catch (cause) { reject(new PackageError('INVALID_RECOVERY_BOUNDARY', 'Invalid updater boundary', { cause })); }
        }
      });
      child.stderr.on('data', () => {});
      child.once('error', error => { clearTimeout(timer); reject(error); });
      child.once('close', () => { clearTimeout(timer); if (!output) reject(new PackageError('RECOVERY_WORKER_EXITED', 'Updater exited before boundary')); });
    });
    child.kill('SIGKILL'); await exited;
  } finally {
    if (!closed) { child.kill('SIGKILL'); await exited; }
  }
  let lease;
  for (let attempt = 0; attempt < 40; attempt++) {
    try { lease = await acquireNativeHandoverGate(root, helper); break; }
    catch (cause) { if (cause.code !== 'HANDOVER_BUSY') throw cause; await pause(50); }
  }
  if (!lease) throw new PackageError('RECOVERY_GATE_BUSY', 'Killed updater gate did not release');
  let result; let cleanup; let doctor;
  try {
    result = await recoverNativeSelected({ lease, helper, runtimeName: 'recovered-baseline' });
    try {
      lease.assertHeld();
      doctor = await doctorFoundation({ helper, runtimeRoot: result.session.root });
      if (doctor.state !== 'healthy') throw new PackageError('RECOVERY_DOCTOR_FAILED', 'Restored actual health not verified');
    }
    finally { cleanup = await result.session.stop(); }
  } finally { await lease.release(); }
  if (JSON.stringify(inventoryGolden(goldenRoot)) !== JSON.stringify(goldenBefore)) throw new PackageError('GOLDEN_FIXTURE_CHANGED', 'Synthetic sealed fixture changed across handover');
  const report = { schema: 1, scope: 'm0-native-recovery-proof', boundary, outcome: result.outcome, selection: result.selection,
    desktop_pid: result.session.desktop_pid, daemon_pid: result.session.daemon_pid, desktop_identity: result.session.desktop_identity,
    daemon_identity: result.session.daemon_identity, health: result.session.health, cleanup, doctor, golden,
    synthetic_sealed_fixture_preserved: true,
    actual_updater_killed: true, boundary_before_successor_launch: true, live_child_termination_verified: false,
    power_loss_verified: false, version_upgrade_verified: false, canonical_data_acceptance: false };
  fs.writeFileSync(path.join(root, 'native-recovery-report.json'), `${JSON.stringify(report, null, 2)}\n`, { flag: 'wx' });
  console.log(JSON.stringify(report));
}
try { await main(); }
catch (error) { console.error(JSON.stringify({ code: error.code ?? 'NATIVE_RECOVERY_PROOF_FAILED', cause: error.cause?.code ?? null })); process.exitCode = 2; }
