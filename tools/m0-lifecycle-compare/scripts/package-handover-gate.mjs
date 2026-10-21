// Test-only lease. Every cooperating pilot startup/handover must use this gate.
import path from 'node:path';
import { spawn } from 'node:child_process';
import { assertExperimentLocation, PackageError } from './package-transaction.mjs';

function bounded(promise, milliseconds, code) {
  let timer;
  return Promise.race([promise, new Promise((_, reject) => { timer = setTimeout(() => reject(new PackageError(code, 'Native gate timed out')), milliseconds); })]).finally(() => clearTimeout(timer));
}
export async function acquireNativeHandoverGate(root, helper) {
  if (typeof root !== 'string' || typeof helper !== 'string' || !path.isAbsolute(root) || !path.isAbsolute(helper)) throw new PackageError('INVALID_HANDOVER_INPUT', 'Absolute root and helper required');
  assertExperimentLocation(root);
  const child = spawn(helper, ['hold-native-handover-gate', '--root', root], { windowsHide: true, stdio: ['pipe', 'pipe', 'pipe'] });
  const controller = new AbortController();
  let outcome = null; let buffer = ''; let diagnostic = null;
  let resolveReady; let rejectReady; let releasing = false; let releasePromise; let releaseConfirmed = false;
  const ready = new Promise((resolve, reject) => { resolveReady = resolve; rejectReady = reject; });
  const closed = new Promise(resolve => {
    child.once('error', error => { outcome = { spawn_error: error.code }; controller.abort(); rejectReady(new PackageError('HANDOVER_HELPER_FAILED', 'Native gate helper failed', { cause: error })); resolve(outcome); });
    child.once('close', (code, signal) => {
      outcome = { code, signal }; controller.abort();
      rejectReady(new PackageError(diagnostic?.code ?? 'HANDOVER_HELPER_FAILED', 'Native gate exited before acquisition'));
      resolve(outcome);
    });
  });
  child.stdin.on('error', () => { controller.abort(); });
  child.stderr.on('data', () => {}); // Drain; diagnostics are structured stdout only.
  child.stdout.on('data', chunk => {
    buffer += chunk;
    if (buffer.length > 8192) { controller.abort(); rejectReady(new PackageError('INVALID_HANDOVER_RESPONSE', 'Oversized native gate response')); return; }
    while (buffer.includes('\n')) {
      const end = buffer.indexOf('\n'); const line = buffer.slice(0, end); buffer = buffer.slice(end + 1);
      let value;
      try { value = JSON.parse(line); } catch { rejectReady(new PackageError('INVALID_HANDOVER_RESPONSE', 'Malformed native gate response')); controller.abort(); continue; }
      if (value.schema === 1 && value.ok === false && typeof value.code === 'string') diagnostic = value;
      else if (value.schema === 1 && value.locked === true && value.pid === child.pid) resolveReady();
      else if (releasing && value.schema === 1 && value.released === true) releaseConfirmed = true;
      else { controller.abort(); rejectReady(new PackageError('INVALID_HANDOVER_RESPONSE', 'Unexpected native gate response')); }
    }
  });
  async function stopAbnormal() {
    if (outcome === null) { child.stdin.destroy(); child.kill('SIGKILL'); }
    await bounded(closed, 3000, 'HANDOVER_CLEANUP_FAILED');
  }
  try { await bounded(ready, 5000, 'HANDOVER_ACQUIRE_TIMEOUT'); }
  catch (error) { await stopAbnormal(); throw error; }
  function assertHeld() {
    if (controller.signal.aborted || outcome !== null || releasing) throw new PackageError('HANDOVER_LEASE_LOST', 'Native gate is no longer held');
  }
  try { assertHeld(); } catch (error) { await stopAbnormal(); throw error; }
  return {
    root, pid: child.pid, signal: controller.signal, assertHeld,
    release() {
      if (releasePromise) return releasePromise;
      releasePromise = (async () => {
        try { assertHeld(); } catch (error) { await stopAbnormal(); throw error; }
        releasing = true; child.stdin.end('release\n');
        let result;
        try { result = await bounded(closed, 3000, 'HANDOVER_RELEASE_TIMEOUT'); }
        catch (error) { await stopAbnormal(); throw error; }
        if (result.code !== 0 || result.signal !== null || !releaseConfirmed) throw new PackageError('HANDOVER_RELEASE_FAILED', 'Native gate did not release cleanly');
        return { released: true, exit_code: 0 };
      })();
      return releasePromise;
    },
  };
}
export async function withNativeHandoverGate(root, helper, operation) {
  if (typeof operation !== 'function') throw new PackageError('INVALID_HANDOVER_OPERATION', 'A gated operation is required');
  const lease = await acquireNativeHandoverGate(root, helper);
  let failure;
  try { lease.assertHeld(); const result = await operation(lease); lease.assertHeld(); return result; }
  catch (error) { failure = error; throw error; }
  finally {
    try { await lease.release(); }
    catch (cause) { throw new PackageError('HANDOVER_CLEANUP_FAILED', 'Native gate cleanup failed', { cause: failure ? new AggregateError([failure, cause]) : cause }); }
  }
}
