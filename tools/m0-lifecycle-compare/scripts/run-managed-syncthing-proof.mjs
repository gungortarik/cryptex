// Two synthetic local devices. No production trust or canonical transport.
import * as fs from 'node:fs';
import path from 'node:path';
import { setTimeout as pause } from 'node:timers/promises';
import { assertExperimentLocation, digest, PackageError } from './package-transaction.mjs';
import { startSyncthingPilot } from './syncthing-pilot.mjs';
import { stageSyncthingBundle, verifySyncthingBundle } from './syncthing-bundle.mjs';

async function until(check, code) {
  const deadline = Date.now() + 45000;
  while (Date.now() < deadline) { if (await check()) return; await pause(200); }
  throw new PackageError(code, 'Bounded managed Syncthing observation did not complete');
}
async function main() {
  const args = process.argv.slice(2); const values = {};
  for (let index = 0; index < args.length; index += 2) {
    if (!['--root', '--binary'].includes(args[index]) || !args[index + 1] || Object.hasOwn(values, args[index])) throw new PackageError('INVALID_MANAGED_PROOF_ARGS', 'Unique root/binary flags required');
    values[args[index]] = args[index + 1];
  }
  const root = values['--root']; const inputBinary = values['--binary'];
  if (!path.isAbsolute(root ?? '') || !path.isAbsolute(inputBinary ?? '')) throw new PackageError('INVALID_MANAGED_PROOF_ARGS', 'Absolute fresh app-data root/binary required');
  const base = process.platform === 'win32' ? path.join(process.env.LOCALAPPDATA ?? '', 'Cryptex-Dev', 'p0-4')
    : path.join(process.env.HOME ?? '', 'Library', 'Application Support', 'Cryptex-Dev', 'p0-4');
  if (path.dirname(root) !== base) throw new PackageError('INVALID_MANAGED_PROOF_ROOT', 'Fresh direct child of Cryptex-Dev p0-4 required');
  assertExperimentLocation(root); fs.mkdirSync(root);
  // Resolve an explicit operator-supplied OS package link once, then run only
  // the exact copied package role. No PATH/developer environment resolution.
  const bundle = stageSyncthingBundle(fs.realpathSync(inputBinary), path.join(root, 'native-package'));
  const binary = bundle.binary;
  const globalConfig = process.platform === 'win32' ? path.join(process.env.LOCALAPPDATA, 'Syncthing', 'config.xml')
    : path.join(process.env.HOME, 'Library', 'Application Support', 'Syncthing', 'config.xml');
  const globalBefore = fs.existsSync(globalConfig) ? digest(globalConfig) : null;
  let first; let second; const stopped = [];
  let measurements; let workFailure;
  try {
    first = await startSyncthingPilot(binary, path.join(root, 'first'));
    second = await startSyncthingPilot(binary, path.join(root, 'second'));
    if (first.id === second.id) throw new PackageError('SYNCTHING_IDENTITY_COLLISION', 'Fresh isolated identities must differ');
    for (const [node, peer] of [[first, second], [second, first]]) {
      const device = await node.request('/rest/config/defaults/device');
      Object.assign(device, { deviceID: peer.id, name: 'Synthetic peer', addresses: [`tcp://127.0.0.1:${peer.syncPort}`], autoAcceptFolders: false });
      await node.request('/rest/config/devices', 'POST', device);
      const payload = path.join(node.home, 'payload'); fs.mkdirSync(payload);
      const folder = await node.request('/rest/config/defaults/folder');
      Object.assign(folder, { id: 'm0-managed-fixture', label: 'Synthetic managed fixture', path: payload, type: 'sendreceive', paused: true,
        devices: [{ deviceID: node.id }, { deviceID: peer.id }], fsWatcherEnabled: false, rescanIntervalS: 0,
        versioning: { type: 'simple', params: { keep: '3' }, cleanupIntervalS: 3600, fsPath: '', fsType: 'basic' } });
      await node.request('/rest/config/folders', 'POST', folder);
      const observed = await node.request('/rest/config/folders/m0-managed-fixture');
      if (observed.paused !== true) throw new PackageError('SYNCTHING_PAUSE_UNPROVEN', 'Configured folder did not pause');
    }
    const left = path.join(first.home, 'payload', 'synthetic.txt'); const right = path.join(second.home, 'payload', 'synthetic.txt');
    fs.writeFileSync(left, 'synthetic-one\n');
    for (const node of [first, second]) await node.request('/rest/config/folders/m0-managed-fixture', 'PATCH', { paused: false });
    await first.request('/rest/db/scan?folder=m0-managed-fixture', 'POST');
    await until(async () => fs.existsSync(right) && fs.readFileSync(right, 'utf8') === 'synthetic-one\n', 'SYNCTHING_PAIR_TRANSFER_TIMEOUT');
    fs.writeFileSync(left, 'synthetic-two-with-distinct-length\n');
    await first.request('/rest/db/scan?folder=m0-managed-fixture', 'POST');
    await until(async () => fs.readFileSync(right, 'utf8') === 'synthetic-two-with-distinct-length\n', 'SYNCTHING_UPDATE_TRANSFER_TIMEOUT');
    const versionDir = path.join(second.home, 'payload', '.stversions');
    await until(async () => fs.existsSync(versionDir) && fs.readdirSync(versionDir).some(name => name.startsWith('synthetic~')), 'SYNCTHING_VERSIONING_UNPROVEN');
    const status = await second.request('/rest/db/status?folder=m0-managed-fixture');
    if (status.state !== 'idle' || status.needTotalItems !== 0) throw new PackageError('SYNCTHING_STATUS_UNPROVEN', 'Folder not fully synchronized');
    const firstId = first.id; const secondId = second.id; const oldSecondPid = second.pid;
    stopped.push(await second.stop()); second = null;
    console.log(JSON.stringify({ phase: 'paired-peer-stopped', result: stopped.at(-1) }));
    second = await startSyncthingPilot(binary, path.join(root, 'second'), true);
    console.log(JSON.stringify({ phase: 'peer-restarted', pid: second.pid }));
    if (second.id !== secondId || first.id !== firstId) throw new PackageError('SYNCTHING_RESTART_IDENTITY_CHANGED', 'Retained isolated config identity changed');
    if (fs.readFileSync(right, 'utf8') !== 'synthetic-two-with-distinct-length\n') throw new PackageError('SYNCTHING_RESTART_PAYLOAD_CHANGED', 'Synthetic payload changed across restart');
    await until(async () => {
      const folderStatus = await second.request('/rest/db/status?folder=m0-managed-fixture');
      return folderStatus.state === 'idle' && folderStatus.needTotalItems === 0;
    }, 'SYNCTHING_RESTART_READY_TIMEOUT');
    const earlyRestartPid = second.pid;
    stopped.push(await second.stop()); second = null;
    second = await startSyncthingPilot(binary, path.join(root, 'second'), true);
    if (second.id !== secondId) throw new PackageError('SYNCTHING_RESTART_IDENTITY_CHANGED', 'Second retained identity changed');
    fs.writeFileSync(path.join(first.home, 'payload', 'after-restart.txt'), 'synthetic-after-restart\n');
    await first.request('/rest/db/scan?folder=m0-managed-fixture', 'POST');
    await until(async () => {
      const restored = path.join(second.home, 'payload', 'after-restart.txt');
      return fs.existsSync(restored) && fs.readFileSync(restored, 'utf8') === 'synthetic-after-restart\n';
    }, 'SYNCTHING_RESTART_TRANSFER_TIMEOUT');
    // Disconnect only the synthetic peer, so both local scanners can record
    // divergent versions before either peer receives the other's update.
    for (const [node, peer] of [[first, second], [second, first]]) {
      await node.request(`/rest/config/devices/${peer.id}`, 'PATCH', { paused: true });
    }
    await until(async () => {
      const states = await Promise.all([first.request('/rest/system/connections'), second.request('/rest/system/connections')]);
      return !states[0].connections[second.id]?.connected && !states[1].connections[first.id]?.connected;
    }, 'SYNCTHING_PEER_PAUSE_TIMEOUT');
    const conflictName = 'after-restart.txt';
    const divergent = ['synthetic-left-divergence\n', 'synthetic-right-divergence-longer\n'];
    for (const [index, node] of [first, second].entries()) {
      fs.writeFileSync(path.join(node.home, 'payload', conflictName), divergent[index]);
      await node.request('/rest/db/scan?folder=m0-managed-fixture', 'POST');
      await until(async () => {
        const file = await node.request(`/rest/db/file?folder=m0-managed-fixture&file=${conflictName}`);
        return file.local.size === Buffer.byteLength(divergent[index]);
      }, 'SYNCTHING_DIVERGENT_SCAN_TIMEOUT');
    }
    for (const [node, peer] of [[first, second], [second, first]]) {
      await node.request(`/rest/config/devices/${peer.id}`, 'PATCH', { paused: false });
    }
    await until(async () => [first, second].some(node => {
      const payload = path.join(node.home, 'payload');
      const conflict = fs.readdirSync(payload).find(name => name.startsWith('after-restart.sync-conflict-') && name.endsWith('.txt'));
      if (!conflict) return false;
      const preserved = [fs.readFileSync(path.join(payload, conflictName), 'utf8'), fs.readFileSync(path.join(payload, conflict), 'utf8')].sort();
      return JSON.stringify(preserved) === JSON.stringify([...divergent].sort());
    }), 'SYNCTHING_CONFLICT_VISIBILITY_TIMEOUT');
    measurements = { first_pid: first.pid, second_before_restart_pid: oldSecondPid, second_after_restart_pid: second.pid,
      api_auth_rejections: [first.auth_rejection, second.auth_rejection], isolated_identities: true, pairing_transfer: true,
      folder_pause_resume: true, status_idle: true, versioning_visible: true, retained_config_same_version_restart: true,
      early_restart_shutdown_pid: earlyRestartPid, early_restart_clean_shutdown: true,
      restart_pairing_transfer: true, actual_version_upgrade_verified: false, conflict_visibility_verified: true, cross_host_verified: false, bundled_distribution_verified: false,
      staged_native_package_verified: true, staged_license_attribution_verified: true,
      binary_sha256: digest(binary), version: first.version };
  } catch (cause) { workFailure = cause; throw cause; }
  finally {
    const results = [];
    for (const node of [second, first].filter(Boolean)) {
      try { results.push({ status: 'fulfilled', value: await node.stop() }); }
      catch (reason) { results.push({ status: 'rejected', reason }); }
    }
    for (const result of results) { if (result.status === 'fulfilled') stopped.push(result.value); }
    if (results.some(result => result.status === 'rejected')) throw new PackageError('MANAGED_SYNCTHING_CLEANUP_FAILED', 'Owned process clean shutdown failed', { cause: new AggregateError([...(workFailure ? [workFailure] : []), ...results.filter(result => result.status === 'rejected').map(result => result.reason)]) });
  }
  const globalAfter = fs.existsSync(globalConfig) ? digest(globalConfig) : null;
  verifySyncthingBundle(bundle.root);
  if (globalBefore !== globalAfter) throw new PackageError('GLOBAL_SYNCTHING_CONFIG_CHANGED', 'Observed user-global config changed; cause requires inspection');
  const report = { schema: 1, scope: 'm0-managed-syncthing-pilot', platform: process.platform, arch: process.arch, node: process.version,
    measurements, stopped, staged_package: bundle.manifest, shutdown_transport: 'pinned-native-cli', global_config_unchanged: true, production_api_secret_storage_verified: false, canonical_data_acceptance: false };
  fs.writeFileSync(path.join(root, 'managed-syncthing-report.json'), `${JSON.stringify(report, null, 2)}\n`, { flag: 'wx' });
  console.log(JSON.stringify(report));
}
try { await main(); }
catch (error) {
  function diagnostic(value) { return { code: value.code ?? value.name ?? 'UNKNOWN', diagnostics: value.diagnostics ?? null,
    causes: value.cause instanceof AggregateError ? value.cause.errors.map(diagnostic) : value.cause ? [diagnostic(value.cause)] : [] }; }
  console.error(JSON.stringify(diagnostic(error))); process.exitCode = 2;
}
