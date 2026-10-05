// Test-only owned subprocess. IPC controls an observed transaction checkpoint.
import { transition } from './package-transaction.mjs';

const [root, encoded, desktop, daemon, boundary] = process.argv.slice(2);
try {
  const selection = await transition(root, JSON.parse(encoded), { desktop, daemon }, {
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
