// Real daemon lifecycle + shared Rust Desktop IPC, isolated from installed Desktop.
import * as fs from 'node:fs';
import path from 'node:path';
import { execFile, spawn } from 'node:child_process';
import { promisify } from 'node:util';
import { setTimeout as pause } from 'node:timers/promises';
import { assertExperimentLocation, PackageError } from './package-transaction.mjs';

const execute = promisify(execFile);
const hidden = { windowsHide: true, timeout: 4500, maxBuffer: 16384 };
async function closedWithin(closed, milliseconds) {
  let timer;
  try {
    return await Promise.race([closed, new Promise((_, reject) => {
      timer = setTimeout(() => reject(new PackageError('STOP_TIMEOUT', 'Owned daemon did not stop within deadline')), milliseconds);
    })]);
  } finally { clearTimeout(timer); }
}
export async function probeDaemon({ helper, daemon, root, protocol }) {
  if (!path.isAbsolute(root) || !path.isAbsolute(helper) || !path.isAbsolute(daemon) || !Number.isSafeInteger(protocol) || protocol < 1 || protocol > 0xffffffff) throw new PackageError('INVALID_HEALTH_INPUT', 'Absolute paths and positive u32 protocol required');
  assertExperimentLocation(root);
  // Do not start a test daemon against an active or ambiguous shared endpoint.
  const absent = JSON.parse((await execute(helper, ['probe-package-health', '--expect-absent'], hidden)).stdout);
  if (absent.endpoint_absent !== true) throw new PackageError('ENDPOINT_BUSY', 'Shared endpoint not proved absent');
  fs.mkdirSync(root); // Never adopt another runtime or stale ready/stop metadata.
  const ready = path.join(root, 'ready'); const stop = path.join(root, 'stop');
  const child = spawn(daemon, ['lifecycle', 'hold', '--runtime-root', root, '--ready-file', ready, '--stop-file', stop], { windowsHide: true, stdio: 'ignore' });
  let outcome = null;
  const closed = new Promise(resolve => {
    child.once('error', error => { outcome = { spawn_error: error.code }; resolve(outcome); });
    child.once('close', (code, signal) => { outcome = { code, signal }; resolve(outcome); });
  });
  let failure = null;
  try {
    const deadline = Date.now() + 5000;
    while (!fs.existsSync(ready)) {
      if (outcome !== null) throw new PackageError('DAEMON_START_FAILED', `Candidate daemon exited before ready (${JSON.stringify(outcome)})`);
      if (Date.now() >= deadline) throw new PackageError('READY_TIMEOUT', 'Candidate daemon did not become ready');
      await pause(25);
    }
    if (Number(fs.readFileSync(ready, 'utf8').trim()) !== child.pid) throw new PackageError('READY_PID_MISMATCH', 'Ready PID does not identify the owned daemon');
    const response = JSON.parse((await execute(helper, ['probe-package-health', '--expected-pid', String(child.pid), '--expected-protocol', String(protocol), '--runtime-root', root], hidden)).stdout);
    if (response.daemon_pid !== child.pid || response.protocol_version !== protocol || response.operation !== 'health.pong' || response.client_role !== 'desktop' || response.lock_held !== true) throw new PackageError('INVALID_HEALTH_RESPONSE', 'Unexpected native helper response');
    let busyRefused = false;
    try { await execute(helper, ['probe-package-health', '--expect-absent'], hidden); }
    catch (error) {
      busyRefused = error.code === 2 && error.stderr?.includes(`daemon instance already running (pid ${child.pid})`);
      if (!busyRefused) throw error;
    }
    if (!busyRefused) throw new PackageError('ENDPOINT_GUARD_FAILED', 'Active owned endpoint was not refused by preflight');
    fs.writeFileSync(stop, 'stop\n');
    const exit = await closedWithin(closed, 3000);
    if (exit.code !== 0 || exit.signal !== null) throw new PackageError('UNCLEAN_EXIT', 'Owned daemon did not exit cleanly');
    const status = (await execute(daemon, ['lifecycle', 'status', '--runtime-root', root], hidden)).stdout;
    const instance = /^instance=(.*)$/m.exec(status)?.[1]?.trim();
    if (!['stale', 'absent'].includes(instance)) throw new PackageError('LOCK_NOT_RELEASED', 'Runtime lock release not proved');
    return { ...response, exit_code: exit.code, lock_released: true, busy_endpoint_refused: true, runtime_root: root };
  } catch (error) {
    failure = error;
    throw error;
  } finally {
    if (outcome === null && child.pid) {
      fs.writeFileSync(stop, 'stop\n');
      try { await closedWithin(closed, 3000); }
      catch (error) {
        child.kill('SIGKILL');
        await closedWithin(closed, 3000);
        throw new PackageError('FORCED_CLEANUP', `Owned candidate needed forced cleanup (${error.code})`);
      }
    }
    if (failure && child.pid && outcome && !outcome.spawn_error) {
      if (outcome.code !== 0 || outcome.signal !== null) throw new PackageError('UNCLEAN_FAILURE_EXIT', 'Rejected candidate did not stop cleanly');
      const status = (await execute(daemon, ['lifecycle', 'status', '--runtime-root', root], hidden)).stdout;
      if (!['stale', 'absent'].includes(/^instance=(.*)$/m.exec(status)?.[1]?.trim())) throw new PackageError('LOCK_NOT_RELEASED', 'Rejected candidate retained its lock');
      failure.cleanup = { daemon_pid: child.pid, exit_code: 0, lock_released: true };
    }
  }
}
