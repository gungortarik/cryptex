import { spawn } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import * as fs from 'node:fs';
import path from 'node:path';
import { setTimeout as pause } from 'node:timers/promises';

async function within(promise, milliseconds) {
  let timer;
  try {
    return await Promise.race([promise, new Promise((_, reject) => {
      timer = setTimeout(() => reject(new Error('Owned transaction subprocess deadline exceeded')), milliseconds);
    })]);
  } finally { clearTimeout(timer); }
}

// Only the returned owned child can be released or killed. No process discovery.
export function startContender(root, value, sources, boundary = '', operation = 'transition') {
  const worker = fileURLToPath(new URL('./package-transaction-contender.mjs', import.meta.url));
  const child = spawn(process.execPath, [worker, root, JSON.stringify(value), sources.desktop, sources.daemon, boundary, operation], {
    windowsHide: true, stdio: ['ignore', 'ignore', 'pipe', 'ipc'],
  });
  let result;
  let stderr = '';
  let resolveCheckpoint;
  let rejectCheckpoint;
  const checkpoint = new Promise((resolve, reject) => { resolveCheckpoint = resolve; rejectCheckpoint = reject; });
  checkpoint.catch(() => {});
  child.stderr.on('data', chunk => { stderr = `${stderr}${chunk}`.slice(-4096); });
  child.on('message', message => {
    if (message?.checkpoint === boundary && message.pid === child.pid) resolveCheckpoint(message);
    if (message?.result) result = message.result;
  });
  const closed = new Promise((resolve, reject) => {
    child.once('error', error => { rejectCheckpoint(error); reject(error); });
    child.once('close', (exit_code, signal) => {
      rejectCheckpoint(new Error(`Owned contender exited before checkpoint: ${stderr}`));
      resolve({ ...result, pid: child.pid, exit_code, signal });
    });
  });
  closed.catch(() => {});
  return {
    pid: child.pid,
    checkpoint: () => operation === 'recover' && boundary ? within((async () => {
      const marker = path.join(root, `recovery-checkpoint-${child.pid}`);
      while (!fs.existsSync(marker)) {
        if (child.exitCode !== null || child.signalCode !== null) throw new Error('Recovery exited before filesystem checkpoint');
        await pause(10);
      }
      return { checkpoint: boundary, pid: child.pid };
    })(), 10000) : within(checkpoint, 10000),
    result: () => within(closed, 10000),
    release: () => operation === 'recover' && boundary
      ? fs.writeFileSync(path.join(root, `recovery-release-${child.pid}`), 'release\n', { flag: 'wx' })
      : child.send({ release: boundary }),
    stop: async () => {
      if (child.exitCode === null && child.signalCode === null) child.kill('SIGKILL');
      await within(closed, 3000);
    },
  };
}
export function startRecovery(root, boundary = '') { return startContender(root, null, { desktop: '', daemon: '' }, boundary, 'recover'); }
