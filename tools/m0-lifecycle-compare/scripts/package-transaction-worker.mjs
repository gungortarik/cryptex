// Test subprocess: parent force-kills this process at an observed filesystem boundary.
import { transition } from './package-transaction.mjs';
const [root, encoded, desktop, daemon, boundary] = process.argv.slice(2);
await transition(root, JSON.parse(encoded), { desktop, daemon }, {
  health: async () => true, // Fixture health only; no actual application launch.
  checkpoint: async (point) => {
    if (point === boundary) {
      process.stdout.write(`${point}\n`);
      setInterval(() => {}, 1000); // Keep the observed boundary alive until parent kills us.
      await new Promise(() => {});
    }
  },
});
