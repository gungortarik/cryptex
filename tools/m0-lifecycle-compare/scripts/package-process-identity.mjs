// Read-only process identity behind platform adapters; no command-line bodies.
import * as fs from 'node:fs';
import path from 'node:path';
import { execFile } from 'node:child_process';
import { promisify } from 'node:util';
import { PackageError } from './package-transaction.mjs';

const execute = promisify(execFile);
const adapters = {
  async win32(pid) {
    const powershell = path.join(process.env.SystemRoot ?? 'C:\\Windows', 'System32', 'WindowsPowerShell', 'v1.0', 'powershell.exe');
    const script = `$p = Get-CimInstance Win32_Process -Filter 'ProcessId = ${pid}' -ErrorAction Stop; if ($null -eq $p) { exit 3 }; @{ pid = [int]$p.ProcessId; parent = [int]$p.ParentProcessId; executable = $p.ExecutablePath } | ConvertTo-Json -Compress`;
    return JSON.parse((await execute(powershell, ['-NoProfile', '-NonInteractive', '-WindowStyle', 'Hidden', '-Command', script], { windowsHide: true, timeout: 4500, maxBuffer: 8192 })).stdout);
  },
  async darwin(pid) {
    const output = (await execute('/bin/ps', ['-ww', '-p', String(pid), '-o', 'ppid=', '-o', 'comm='], { timeout: 4500, maxBuffer: 8192 })).stdout.trim();
    const match = /^(\d+)\s+(.+)$/.exec(output);
    if (!match) throw new PackageError('PROCESS_IDENTITY_UNAVAILABLE', 'Process identity unavailable');
    return { pid, parent: Number(match[1]), executable: match[2] };
  },
};
export async function verifyOwnedProcess(pid, parent, executable) {
  if (!Number.isSafeInteger(pid) || pid < 1 || !Number.isSafeInteger(parent) || parent < 1 || typeof executable !== 'string' || !path.isAbsolute(executable)) throw new PackageError('INVALID_PROCESS_IDENTITY', 'Positive process IDs and absolute binary required');
  const adapter = adapters[process.platform];
  if (!adapter) throw new PackageError('UNSUPPORTED_PROCESS_PLATFORM', 'Process identity adapter unavailable');
  const observed = await adapter(pid);
  if (observed.pid !== pid || observed.parent !== parent || typeof observed.executable !== 'string'
    || !path.isAbsolute(observed.executable) || fs.realpathSync(observed.executable) !== fs.realpathSync(executable)) throw new PackageError('PROCESS_IDENTITY_MISMATCH', 'Owned process identity mismatch');
  return observed;
}
