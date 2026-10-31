// Synthetic M0 fixtures and actual IPC diagnostics; no product event writer.
import * as fs from 'node:fs';
import path from 'node:path';
import { execFile } from 'node:child_process';
import { promisify } from 'node:util';
import { assertExperimentLocation, digest, PackageError } from './package-transaction.mjs';
const execute = promisify(execFile);
const options = { windowsHide: true, timeout: 5000, maxBuffer: 16384 };

export function inventoryGolden(root) {
  assertExperimentLocation(root); const entries = [];
  function visit(directory, prefix) {
    for (const name of fs.readdirSync(directory).sort()) {
      const file = path.join(directory, name); const stat = fs.lstatSync(file); const relative = prefix ? `${prefix}/${name}` : name;
      if (stat.isSymbolicLink() || (!stat.isDirectory() && !stat.isFile())) throw new PackageError('UNSAFE_GOLDEN_ENTRY', 'Golden fixture contains unsafe entry');
      if (stat.isDirectory()) { entries.push({ path: relative, kind: 'directory' }); visit(file, relative); }
      else entries.push({ path: relative, kind: 'file', size: stat.size, sha256: digest(file) });
    }
  }
  visit(root, ''); return entries;
}
export async function createGoldenFoundation(root, benchmark) {
  if (!path.isAbsolute(root ?? '') || !path.isAbsolute(benchmark ?? '')) throw new PackageError('INVALID_GOLDEN_ARGS', 'Absolute fresh root and benchmark required');
  assertExperimentLocation(root); fs.mkdirSync(root);
  const data = path.join(root, 'data');
  await execute(benchmark, ['generate', 'segments', '4', '128', data, '2'], options);
  const entries = inventoryGolden(data);
  if (entries.filter(entry => entry.kind === 'file' && entry.path.endsWith('.seg')).length !== 2 || entries.some(entry => entry.path.endsWith('.open'))) throw new PackageError('GOLDEN_LAYOUT_MISMATCH', 'Two complete sealed synthetic packs required');
  const manifest = { schema: 1, scope: 'm0-golden-brain-foundation', synthetic_only: true, production_event_schema: false,
    layout_decision: 'ADR-0013', benchmark_profile: { events: 4, payload_bytes: 128, segment_events: 2 }, entries };
  fs.writeFileSync(path.join(root, 'golden-manifest.json'), `${JSON.stringify(manifest, null, 2)}\n`, { flag: 'wx' });
  return manifest;
}
export async function doctorFoundation({ helper, runtimeRoot }) {
  if (!path.isAbsolute(helper ?? '') || !path.isAbsolute(runtimeRoot ?? '')) throw new PackageError('INVALID_DOCTOR_ARGS', 'Absolute helper and runtime required');
  assertExperimentLocation(runtimeRoot);
  const ready = path.join(runtimeRoot, 'ready'); let expectedPid = null;
  try {
    const stat = fs.lstatSync(ready);
    if (stat.isSymbolicLink() || !stat.isFile()) throw new PackageError('INVALID_DOCTOR_READY', 'Ready record must be ordinary');
    const text = fs.readFileSync(ready, 'utf8').trim(); expectedPid = Number(text);
    if (!/^\d+$/.test(text) || !Number.isSafeInteger(expectedPid) || expectedPid < 1) throw new PackageError('INVALID_DOCTOR_READY', 'Ready PID malformed');
  } catch (cause) { if (cause.code !== 'ENOENT') throw cause; }
  const args = expectedPid === null ? ['probe-package-health', '--expect-absent']
    : ['probe-package-health', '--expected-pid', String(expectedPid), '--expected-protocol', '1', '--runtime-root', runtimeRoot];
  try {
    const result = JSON.parse((await execute(helper, args, options)).stdout);
    if (expectedPid === null ? result.endpoint_absent !== true
      : result.daemon_pid !== expectedPid || result.protocol_version !== 1 || result.operation !== 'health.pong' || result.lock_held !== true) throw new PackageError('DOCTOR_RESPONSE_MISMATCH', 'Actual health observation mismatched');
    return { schema: 1, scope: 'm0-doctor-foundation', state: expectedPid === null ? 'daemon-absent' : 'healthy', actual_ipc_probe: true,
      daemon_pid: expectedPid, protocol: expectedPid === null ? null : 1, canonical_store_check: 'not-implemented', syncthing_check: 'not-implemented' };
  } catch (cause) {
    return { schema: 1, scope: 'm0-doctor-foundation', state: 'attention', actual_ipc_probe: true,
      code: typeof cause.code === 'string' ? cause.code : 'DOCTOR_IPC_FAILED', exit_code: typeof cause.code === 'number' ? cause.code : null,
      canonical_store_check: 'not-implemented', syncthing_check: 'not-implemented' };
  }
}
