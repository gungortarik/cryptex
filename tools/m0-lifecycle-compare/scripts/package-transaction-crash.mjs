import { spawn } from 'node:child_process';
import { fileURLToPath } from 'node:url';

// Only kills the exact subprocess this function created; never searches for app processes.
export async function killTransactionAt(root, value, sources, boundary) {
  const worker = fileURLToPath(new URL('./package-transaction-worker.mjs', import.meta.url));
  const child = spawn(process.execPath, [worker, root, JSON.stringify(value), sources.desktop, sources.daemon, boundary], { windowsHide: true, stdio: ['ignore', 'pipe', 'pipe'] });
  let stderr = '';
  child.stderr.on('data', chunk => { stderr += chunk; });
  const closed = new Promise((resolve, reject) => { child.once('error', reject); child.once('close', (exit, signal) => resolve({ exit, signal })); });
  // An error may arrive while checkpoint observation is still pending.
  closed.catch(() => {});
  try {
    await new Promise((resolve, reject) => {
      const timer = setTimeout(() => reject(new Error(`Checkpoint timeout (${boundary}): ${stderr}`)), 10000);
      let output = '';
      child.stdout.on('data', chunk => { output += chunk; if (output.includes(`${boundary}\n`)) { clearTimeout(timer); resolve(); } });
      child.once('error', error => { clearTimeout(timer); reject(error); });
      child.once('close', () => { clearTimeout(timer); reject(new Error(`Worker exited before checkpoint: ${stderr}`)); });
    });
    if (!child.kill('SIGKILL')) throw new Error('Unable to kill owned transaction subprocess');
    const result = await closed;
    if (result.signal !== 'SIGKILL') throw new Error('Owned subprocess did not report the requested forced termination');
    return { checkpoint: boundary, pid: child.pid, ...result };
  } finally {
    if (child.exitCode === null && child.signalCode === null) { child.kill('SIGKILL'); await closed; }
  }
}
