// Test-only worker. Injected sessions never launch a Desktop or daemon.
import { withNativeHandoverGate } from './package-handover-gate.mjs';
import { handoverNativeSelected, prepareNativeSlots, startNativeSelected } from './package-native-selection.mjs';
import { describeNativeTree } from './package-native-tree.mjs';
const [root, helper, source, desktop, daemon] = process.argv.slice(2);
await withNativeHandoverGate(root, helper, async lease => {
  let active;
  if (source) {
    const manifest = describeNativeTree(source, { desktop, daemon });
    prepareNativeSlots(lease, { source, manifest }, { source, manifest });
    active = await startNativeSelected({ lease, helper, runtimeName: 'baseline-first' });
  } else {
    active = { selection: { slot: 'baseline' }, session: { stop: async () => ({ exit_code: 0, lock_released: true, endpoint_absent: true }) } };
  }
  await handoverNativeSelected({ lease, helper, runtimePrefix: 'interrupted',
    active,
    start: async () => {
      console.log(JSON.stringify({ phase: 'selected-before-candidate-launch', pid: process.pid,
        baseline_desktop_pid: active.session.desktop_pid ?? null, baseline_daemon_pid: active.session.daemon_pid ?? null }));
      await new Promise(() => { setInterval(() => {}, 1000); });
    } });
});
