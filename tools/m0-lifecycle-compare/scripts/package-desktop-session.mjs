// Actual owned Desktop/sidecar session; no slot activation or installed updater.
import * as fs from 'node:fs';
import path from 'node:path';
import { spawn, execFile } from 'node:child_process';
import { promisify } from 'node:util';
import { setTimeout as pause } from 'node:timers/promises';
import { assertExperimentLocation, PackageError } from './package-transaction.mjs';
import { verifyNativeTree } from './package-native-tree.mjs';
import { verifyOwnedProcess } from './package-process-identity.mjs';

const execute = promisify(execFile);
const options = { windowsHide: true, timeout: 4500, maxBuffer: 16384 };
async function absent(helper) {
  const response = JSON.parse((await execute(helper, ['probe-package-health', '--expect-absent'], options)).stdout);
  if (response.endpoint_absent !== true) throw new PackageError('ENDPOINT_BUSY', 'IPC endpoint not proven absent');
}
async function waitClosed(closed, milliseconds) {
  let timer;
  try { return await Promise.race([closed, new Promise((_, reject) => { timer = setTimeout(() => reject(new PackageError('DESKTOP_STOP_TIMEOUT', 'Owned Desktop did not exit')), milliseconds); })]); }
  finally { clearTimeout(timer); }
}
export function isDesktopLaunchSnapshot(text) {
  return /^note=desktop_launched\r?$/m.test(text) && /^updated_unix_ms=\d+\r?$/m.test(text) && text.endsWith('\n');
}
export async function startOwnedDesktop({ lease, helper, payload, manifest, runtimeName }) {
  if (!lease || typeof lease.assertHeld !== 'function' || typeof lease.root !== 'string' || !path.isAbsolute(lease.root)
    || typeof helper !== 'string' || !path.isAbsolute(helper) || typeof runtimeName !== 'string' || !/^[a-z][a-z0-9-]{0,63}$/.test(runtimeName)) throw new PackageError('INVALID_DESKTOP_SESSION', 'Held pilot gate, helper and bounded runtime name required');
  lease.assertHeld(); assertExperimentLocation(lease.root); verifyNativeTree(payload, manifest);
  await absent(helper); lease.assertHeld();
  const root = path.join(lease.root, runtimeName); fs.mkdirSync(root);
  fs.writeFileSync(path.join(root, 'pilot-runtime.marker'), 'm0-native-desktop-runtime-v1\n', { flag: 'wx' });
  const executable = path.join(payload, ...manifest.roles.desktop.split('/'));
  const daemon = path.join(payload, ...manifest.roles.daemon.split('/'));
  const environment = { ...process.env }; delete environment.CRYPTEXD_BIN;
  const child = spawn(executable, ['--cryptex-background', '--cryptex-pilot-runtime', root], { windowsHide: true, env: environment, stdio: 'ignore' });
  let outcome = null; let daemonPid = null; let stopped;
  const closed = new Promise(resolve => {
    child.once('error', error => { outcome = { spawn_error: error.code }; resolve(outcome); });
    child.once('close', (code, signal) => { outcome ??= { code, signal }; resolve(outcome); });
  });
  function stop() {
    if (stopped) return stopped;
    stopped = (async () => {
      if (outcome === null) {
        fs.writeFileSync(path.join(root, 'pilot-quit.request'), 'quit\n', { flag: 'wx' });
        try { await waitClosed(closed, 6000); }
        catch (cause) {
          fs.writeFileSync(path.join(root, 'stop'), 'stop\n');
          child.kill('SIGKILL'); await waitClosed(closed, 3000);
          throw new PackageError('FORCED_DESKTOP_CLEANUP', 'Owned Desktop required forced cleanup; no clean stop claimed', { cause });
        }
      }
      const result = await closed;
      if (result.spawn_error) {
        await absent(helper);
        return { spawn_failed: true, no_child_started: true, endpoint_absent: true };
      }
      if (result.code !== 0 || result.signal !== null) throw new PackageError('DESKTOP_UNCLEAN_EXIT', 'Owned Desktop did not exit cleanly');
      const snapshot = fs.readFileSync(path.join(root, 'gui-proof.snapshot'), 'utf8');
      if (!/^note=clean_quit\r?$/m.test(snapshot) || !/^daemon_pid=none\r?$/m.test(snapshot)) throw new PackageError('DESKTOP_CLEAN_QUIT_UNPROVEN', 'Clean shutdown snapshot missing');
      const status = (await execute(daemon, ['lifecycle', 'status', '--runtime-root', root], options)).stdout;
      if (!/^instance=(stale|absent)\r?$/m.test(status)) throw new PackageError('LOCK_NOT_RELEASED', 'Owned daemon lock still held');
      await absent(helper);
      if (daemonPid !== null) {
        try { process.kill(daemonPid, 0); }
        catch (error) { if (error.code === 'ESRCH') return { desktop_pid: child.pid, daemon_pid: daemonPid, exit_code: 0, lock_released: true, endpoint_absent: true }; throw error; }
        throw new PackageError('DAEMON_PROCESS_REMAINS', 'Recorded daemon PID is still present or reused');
      }
      return { desktop_pid: child.pid, exit_code: 0, lock_released: true, endpoint_absent: true };
    })();
    return stopped;
  }
  try {
    const deadline = Date.now() + 8000;
    const snapshotFile = path.join(root, 'gui-proof.snapshot'); const ready = path.join(root, 'ready');
    while (!fs.existsSync(snapshotFile) || !fs.existsSync(ready) || !isDesktopLaunchSnapshot(fs.readFileSync(snapshotFile, 'utf8'))) {
      lease.assertHeld();
      if (outcome !== null) {
        const failure = new PackageError(outcome.spawn_error ? 'DESKTOP_SPAWN_FAILED' : 'DESKTOP_START_FAILED', 'Owned Desktop exited before readiness');
        failure.diagnostics = outcome; throw failure;
      }
      if (Date.now() >= deadline) throw new PackageError('DESKTOP_READY_TIMEOUT', 'Owned Desktop readiness timed out');
      await pause(50);
    }
    lease.assertHeld();
    const snapshot = fs.readFileSync(snapshotFile, 'utf8');
    daemonPid = Number(fs.readFileSync(ready, 'utf8').trim());
    if (!Number.isSafeInteger(daemonPid) || daemonPid < 1 || !snapshot.includes(`desktop_pid=${child.pid}\n`) || !snapshot.includes(`daemon_pid=${daemonPid}\n`)
      || !/^window_visible=false\r?$/m.test(snapshot)) {
      const failure = new PackageError('DESKTOP_IDENTITY_MISMATCH', 'Desktop/daemon snapshot mismatch or visible initial window');
      failure.diagnostics = { expected_desktop_pid: child.pid, ready_daemon_pid: daemonPid,
        snapshot_desktop_pid: /^desktop_pid=(.*)$/m.exec(snapshot)?.[1]?.trim() ?? null,
        snapshot_daemon_pid: /^daemon_pid=(.*)$/m.exec(snapshot)?.[1]?.trim() ?? null,
        window_visible: /^window_visible=(.*)$/m.exec(snapshot)?.[1]?.trim() ?? null };
      fs.writeFileSync(path.join(root, 'pilot-startup-diagnostic.json'), JSON.stringify(failure.diagnostics), { flag: 'wx' });
      throw failure;
    }
    const desktopIdentity = await verifyOwnedProcess(child.pid, process.pid, executable); lease.assertHeld();
    const daemonIdentity = await verifyOwnedProcess(daemonPid, child.pid, daemon); lease.assertHeld();
    const response = JSON.parse((await execute(helper, ['probe-package-health', '--expected-pid', String(daemonPid), '--expected-protocol', String(manifest.protocol), '--runtime-root', root], options)).stdout); lease.assertHeld();
    if (response.daemon_pid !== daemonPid || response.protocol_version !== manifest.protocol || response.lock_held !== true || response.operation !== 'health.pong') throw new PackageError('DESKTOP_HEALTH_MISMATCH', 'Owned Desktop daemon IPC health mismatch');
    verifyNativeTree(payload, manifest);
    return { root, desktop_pid: child.pid, daemon_pid: daemonPid, health: response, desktop_identity: desktopIdentity, daemon_identity: daemonIdentity, stop };
  } catch (cause) {
    try { await stop(); }
    catch (cleanup) { throw new PackageError('DESKTOP_SESSION_CLEANUP_FAILED', 'Owned session start and cleanup failed', { cause: new AggregateError([cause, cleanup]) }); }
    throw cause;
  }
}
