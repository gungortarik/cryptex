// Test-only owned subprocess. IPC controls an observed transaction checkpoint.
import { recover, transition } from './package-transaction.mjs';
import * as fs from 'node:fs';
import path from 'node:path';

const [root, encoded, desktop, daemon, boundary, operation = 'transition'] = process.argv.slice(2);
function recoveryBarrier(point) {
    if (boundary === point) {
      const marker = path.join(root, `recovery-checkpoint-${process.pid}`);
      const release = path.join(root, `recovery-release-${process.pid}`);
      fs.writeFileSync(marker, point, { flag: 'wx' });
      const deadline = Date.now() + 10000;
      while (!fs.existsSync(release)) {
        if (Date.now() >= deadline) throw new Error('Recovery barrier deadline exceeded');
        Atomics.wait(new Int32Array(new SharedArrayBuffer(4)), 0, 0, 10);
      }
      fs.unlinkSync(marker);
      fs.unlinkSync(release);
    }
}
try {
  const recoveryIo = { ...fs, renameSync: (...args) => {
    recoveryBarrier('restore-selection');
    return fs.renameSync(...args);
  }, unlinkSync: (...args) => {
    const result = fs.unlinkSync(...args);
    recoveryBarrier('intent-removed');
    return result;
  } };
  const selection = operation === 'recover' ? recover(root, { io: recoveryIo }).selection : await transition(root, JSON.parse(encoded), { desktop, daemon }, {
    health: async () => true,
    checkpoint: async point => {
      if (point !== boundary) return;
      await new Promise((resolve, reject) => {
        process.once('message', message => {
          if (message?.release !== point) reject(new Error('Unexpected checkpoint release'));
          else resolve();
        });
        process.send({ checkpoint: point, pid: process.pid });
      });
    },
  });
  process.send({ result: { ok: true, selection } });
} catch (error) {
  process.exitCode = 2;
  process.send({ result: { ok: false, code: error.code ?? 'UNEXPECTED_ERROR' } });
} finally {
  process.disconnect();
}
