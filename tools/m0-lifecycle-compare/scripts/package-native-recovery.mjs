// Test-only native stale-claim cleanup, then the existing candidate recovery.
import path from 'node:path';
import { execFile } from 'node:child_process';
import { promisify } from 'node:util';
import { assertExperimentLocation, PackageError, recover } from './package-transaction.mjs';

const execute = promisify(execFile);
export async function clearInterruptedClaim(root, helper) {
  if (!path.isAbsolute(root) || !path.isAbsolute(helper)) throw new PackageError('INVALID_NATIVE_RECOVERY_INPUT', 'Absolute root and helper required');
  assertExperimentLocation(root);
  let output;
  try {
    output = await execute(helper, ['clear-interrupted-recovery-claim', '--root', root, '--node', process.execPath], {
      windowsHide: true, timeout: 5000, maxBuffer: 8192,
    });
  } catch (cause) {
    let diagnostic;
    try { diagnostic = JSON.parse(cause.stdout?.trim()); } catch { /* Non-JSON process errors remain explicit below. */ }
    throw new PackageError(diagnostic?.schema === 1 && diagnostic.ok === false && typeof diagnostic.code === 'string' ? diagnostic.code : 'NATIVE_RECOVERY_FAILED', 'Native claim cleanup refused or failed', { cause });
  }
  let result;
  try { result = JSON.parse(output.stdout.trim()); }
  catch (cause) { throw new PackageError('INVALID_NATIVE_RECOVERY_RESPONSE', 'Native claim cleanup response invalid', { cause }); }
  if (result.schema !== 1 || result.ok !== true || typeof result.reclaimed !== 'boolean') throw new PackageError('INVALID_NATIVE_RECOVERY_RESPONSE', 'Native claim cleanup response invalid');
  return result;
}
export async function recoverAfterInterruption(root, helper) {
  const cleanup = await clearInterruptedClaim(root, helper);
  // A new recovery can win the filesystem claim between these calls. The core
  // engine will refuse safely rather than deleting the new owner's claim.
  return { cleanup, recovery: recover(root) };
}
