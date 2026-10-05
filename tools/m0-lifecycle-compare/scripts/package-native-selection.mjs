// Isolated M0 pilot only. Pending journals refuse reuse; recovery is a later gate.
import * as fs from 'node:fs';
import path from 'node:path';
import { randomUUID } from 'node:crypto';
import { assertExperimentLocation, PackageError } from './package-transaction.mjs';
import { stageNativeTree, verifyNativeTree } from './package-native-tree.mjs';
import { startOwnedDesktop } from './package-desktop-session.mjs';

const scope = 'm0-native-selection';
function fail(code, cause) { throw new PackageError(code, 'Native pilot selection refused; retained state requires inspection', { cause }); }
function id(value) { if (typeof value !== 'string' || !/^[a-z][a-z0-9-]{0,63}$/.test(value)) fail('INVALID_NATIVE_SLOT'); return value; }
function read(file) {
  const info = fs.lstatSync(file);
  if (!info.isFile() || info.isSymbolicLink()) fail('UNSAFE_NATIVE_RECORD');
  try { return JSON.parse(fs.readFileSync(file, 'utf8')); }
  catch (cause) { fail('CORRUPT_NATIVE_RECORD', cause); }
}
function root(lease) {
  if (!lease || typeof lease.assertHeld !== 'function' || !path.isAbsolute(lease.root ?? '')) fail('INVALID_NATIVE_LEASE');
  lease.assertHeld(); assertExperimentLocation(lease.root);
  const marker = read(path.join(lease.root, 'experiment.json'));
  if (marker.schema !== 1 || marker.scope !== 'm0-native-handover-pilot') fail('UNOWNED_NATIVE_ROOT');
  return lease.root;
}
function flush(directory) {
  if (process.platform === 'win32') return; // No Windows power-loss guarantee.
  const fd = fs.openSync(directory, 'r');
  try { fs.fsyncSync(fd); } finally { fs.closeSync(fd); }
}
function writeNew(file, value) {
  const fd = fs.openSync(file, 'wx');
  try { fs.writeFileSync(fd, `${JSON.stringify(value)}\n`); fs.fsyncSync(fd); }
  finally { fs.closeSync(fd); }
  flush(path.dirname(file));
}
function replace(lease, value) {
  const directory = root(lease); const file = path.join(directory, `native-selection-${randomUUID()}.tmp`);
  writeNew(file, value); lease.assertHeld();
  fs.renameSync(file, path.join(directory, 'native-selection.json')); flush(directory);
}
function selection(slot) { return { schema: 1, scope, slot: id(slot) }; }
function noIntent(directory) {
  try { fs.lstatSync(path.join(directory, 'native-intent.json')); }
  catch (error) { if (error.code === 'ENOENT') return; throw error; }
  fail('NATIVE_RECOVERY_REQUIRED');
}
function slot(lease, name) {
  const directory = root(lease); id(name);
  const base = path.join(directory, 'native-slots', name);
  const manifest = read(path.join(base, 'manifest.json'));
  const payload = path.join(base, 'payload'); verifyNativeTree(payload, manifest);
  return { payload, manifest };
}
export function readNativeSelection(lease) {
  const directory = root(lease); const record = read(path.join(directory, 'native-selection.json'));
  if (record.schema !== 1 || record.scope !== scope) fail('UNSUPPORTED_NATIVE_SELECTION');
  slot(lease, record.slot); return record;
}
export function prepareNativeSlots(lease, baseline, candidate) {
  const directory = root(lease); noIntent(directory);
  fs.mkdirSync(path.join(directory, 'native-slots'));
  for (const [name, input] of [['baseline', baseline], ['candidate', candidate]]) {
    lease.assertHeld();
    const base = path.join(directory, 'native-slots', name); fs.mkdirSync(base);
    stageNativeTree(input.source, path.join(base, 'payload'), input.manifest);
    writeNew(path.join(base, 'manifest.json'), input.manifest);
  }
  writeNew(path.join(directory, 'native-selection.json'), selection('baseline'));
  return readNativeSelection(lease);
}
export async function startNativeSelected({ lease, helper, runtimeName, start = startOwnedDesktop }) {
  noIntent(root(lease)); const selected = readNativeSelection(lease);
  const session = await start({ lease, helper, runtimeName, ...slot(lease, selected.slot) });
  return { selection: selected, session };
}
function clean(result) {
  return result?.endpoint_absent === true && ((result.exit_code === 0 && result.lock_released === true) || (result.spawn_failed === true && result.no_child_started === true));
}
function ownedIntent(file, expected) {
  if (JSON.stringify(read(file)) !== JSON.stringify(expected)) fail('NATIVE_INTENT_CHANGED');
}
export async function handoverNativeSelected({ lease, helper, active, candidate = 'candidate', runtimePrefix, start = startOwnedDesktop }) {
  const directory = root(lease); noIntent(directory);
  if (typeof runtimePrefix !== 'string' || !/^[a-z][a-z0-9-]{0,47}$/.test(runtimePrefix)) fail('INVALID_NATIVE_RUNTIME_PREFIX');
  const before = readNativeSelection(lease); const after = selection(candidate);
  if (before.slot === after.slot || active?.selection?.slot !== before.slot || typeof active?.session?.stop !== 'function') fail('NATIVE_ACTIVE_SELECTION_MISMATCH');
  const candidateSlot = slot(lease, after.slot); // Reject damaged resources before stopping baseline.
  let baselineCleanup;
  try { baselineCleanup = await active.session.stop(); }
  catch (cause) { fail('NATIVE_BASELINE_STOP_FAILED', cause); }
  if (!clean(baselineCleanup)) fail('NATIVE_BASELINE_STOP_UNPROVEN');
  root(lease);
  if (readNativeSelection(lease).slot !== before.slot) fail('NATIVE_SELECTION_CHANGED');
  const intentFile = path.join(directory, 'native-intent.json');
  const intent = { schema: 1, scope, before, after };
  try { writeNew(intentFile, intent); replace(lease, after); }
  catch (cause) { fail('NATIVE_ACTIVATION_IO_FAILED', cause); }
  let candidateSession; let rejection;
  try {
    candidateSession = await start({ lease, helper, runtimeName: `${runtimePrefix}-candidate`, ...candidateSlot });
    root(lease); slot(lease, after.slot);
  } catch (cause) {
    rejection = cause;
    if (candidateSession) {
      let cleanup;
      try { cleanup = await candidateSession.stop(); }
      catch (error) { fail('NATIVE_CANDIDATE_STOP_FAILED', new AggregateError([cause, error])); }
      if (!clean(cleanup)) fail('NATIVE_CANDIDATE_STOP_UNPROVEN', cause);
    } else if (cause.code !== 'DESKTOP_SESSION_START_FAILED' || !clean(cause.diagnostics?.cleanup)) {
      fail('NATIVE_CANDIDATE_CLEANUP_UNPROVEN', cause);
    }
  }
  if (rejection) {
    let baselineSession;
    try {
      root(lease); ownedIntent(intentFile, intent); slot(lease, before.slot); replace(lease, before);
      baselineSession = await start({ lease, helper, runtimeName: `${runtimePrefix}-rollback`, ...slot(lease, before.slot) });
      root(lease); slot(lease, before.slot); ownedIntent(intentFile, intent);
      fs.unlinkSync(intentFile); flush(directory);
    } catch (cause) {
      if (baselineSession) {
        try { await baselineSession.stop(); }
        catch (cleanup) { fail('NATIVE_ROLLBACK_FAILED', new AggregateError([cause, cleanup])); }
      }
      fail('NATIVE_ROLLBACK_FAILED', cause);
    }
    return { outcome: 'rolled-back', selection: before, session: baselineSession, rejection: rejection.diagnostics ?? { code: rejection.code }, baseline_cleanup: baselineCleanup };
  }
  try {
    root(lease); ownedIntent(intentFile, intent);
    if (readNativeSelection(lease).slot !== after.slot) fail('NATIVE_SELECTION_CHANGED');
    fs.unlinkSync(intentFile); flush(directory);
  }
  catch (cause) {
    try { await candidateSession.stop(); }
    catch (cleanup) { fail('NATIVE_COMMIT_FAILED', new AggregateError([cause, cleanup])); }
    fail('NATIVE_COMMIT_FAILED', cause);
  }
  return { outcome: 'committed', selection: after, session: candidateSession, baseline_cleanup: baselineCleanup };
}
