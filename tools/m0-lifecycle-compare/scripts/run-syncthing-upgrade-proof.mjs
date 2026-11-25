// Real 2.1.2 -> pinned 2.1.5 probe with an explicit injected health rejection.
import * as fs from 'node:fs';
import path from 'node:path';
import { setTimeout as pause } from 'node:timers/promises';
import { assertExperimentLocation, digest, PackageError } from './package-transaction.mjs';
import { startSyncthingUpgradePilot, readSyncthingFixture, syncthingFolderIdle } from './syncthing-pilot.mjs';
import { stageSyncthingBundle, verifySyncthingBundle } from './syncthing-bundle.mjs';
import { snapshotSyncthingProfile, restoreSyncthingProfile, assertSyncthingUpgradeHealth } from './syncthing-upgrade.mjs';

async function until(check, code) {
  const deadline = Date.now() + 45000;
  while (Date.now() < deadline) { if (await check()) return; await pause(200); }
  throw new PackageError(code, 'Bounded synthetic upgrade observation failed');
}
async function pair(node, peer) {
  const device = await node.request('/rest/config/defaults/device');
  Object.assign(device, { deviceID: peer.id, name: 'Synthetic upgrade peer', addresses: [`tcp://127.0.0.1:${peer.syncPort}`], autoAcceptFolders: false });
  await node.request('/rest/config/devices', 'POST', device);
  const payload = path.join(node.home, 'payload'); fs.mkdirSync(payload);
  const folder = await node.request('/rest/config/defaults/folder');
  Object.assign(folder, { id: 'm0-managed-fixture', path: payload, type: 'sendreceive', paused: true,
    devices: [{ deviceID: node.id }, { deviceID: peer.id }], fsWatcherEnabled: false, rescanIntervalS: 0,
    versioning: { type: 'simple', params: { keep: '3' }, cleanupIntervalS: 3600, fsPath: '', fsType: 'basic' } });
  await node.request('/rest/config/folders', 'POST', folder);
}
async function folderPause(node, paused) {
  await node.request('/rest/config/folders/m0-managed-fixture', 'PATCH', { paused });
  const config = await node.request('/rest/config/folders/m0-managed-fixture');
  if (config.paused !== paused) throw new PackageError('SYNCTHING_UPGRADE_PAUSE_UNPROVEN', 'Fixture pause state differs');
}
async function transfer(first, second, name, body) {
  fs.writeFileSync(path.join(first.home, 'payload', name), body);
  await first.request('/rest/db/scan?folder=m0-managed-fixture', 'POST');
  await until(async () => readSyncthingFixture(path.join(second.home, 'payload', name)) === body, 'SYNCTHING_UPGRADE_TRANSFER_TIMEOUT');
  for (const node of [first, second]) await until(async () => syncthingFolderIdle(await node.request('/rest/db/status?folder=m0-managed-fixture')), 'SYNCTHING_UPGRADE_IDLE_TIMEOUT');
}
function requireRetained(node, id, certificate, privateKey, originalFile) {
  if (node.id !== id || digest(path.join(node.home, 'cert.pem')) !== certificate || digest(path.join(node.home, 'key.pem')) !== privateKey
    || readSyncthingFixture(path.join(node.home, 'payload', 'before.txt')) !== originalFile) throw new PackageError('SYNCTHING_UPGRADE_RETENTION_FAILED', 'Synthetic identity/payload changed');
}
async function scenario(root, baseline, candidate, reject, baselineVersion, candidateVersion, nativeStageTest) {
  fs.mkdirSync(root); const stops = []; let first; let second; let failure; let result;
  async function stopSecond() { const receipt = await second.stop(); stops.push(receipt); second = null; return receipt; }
  try {
    first = await startSyncthingUpgradePilot(baseline, path.join(root, 'first'), baselineVersion, false, baselineVersion);
    second = await startSyncthingUpgradePilot(baseline, path.join(root, 'second'), baselineVersion, false, baselineVersion);
    if (first.id === second.id) throw new PackageError('SYNCTHING_IDENTITY_COLLISION', 'Distinct synthetic peers required');
    await pair(first, second); await pair(second, first);
    await folderPause(first, false); await folderPause(second, false);
    await transfer(first, second, 'before.txt', 'synthetic-before-upgrade\n');
    // Quiesce both peers before snapshot. No writes are accepted during the
    // rejection window; rollback cannot discard a newly accepted canonical write.
    await folderPause(first, true); await folderPause(second, true);
    const identity = second.id; const home = second.home;
    const cert = digest(path.join(home, 'cert.pem')); const key = digest(path.join(home, 'key.pem'));
    const snapshot = snapshotSyncthingProfile(home, path.join(root, 'cold-baseline'), await stopSecond(), baselineVersion);
    const ownerStage = nativeStageTest ? await import('../../../scripts/m17/native_stage_trial.mjs') : null;
    if (ownerStage) ownerStage.stageOwnerEngine(nativeStageTest, snapshot);
    second = await startSyncthingUpgradePilot(candidate, home, candidateVersion, true, baselineVersion);
    const candidatePid = second.pid;
    requireRetained(second, identity, cert, key, 'synthetic-before-upgrade\n');
    const folder = await second.request('/rest/config/folders/m0-managed-fixture');
    if (!folder.paused || folder.versioning.type !== 'simple' || folder.devices.length !== 2
      || !folder.devices.some(device => device.deviceID === first.id)) throw new PackageError('SYNCTHING_UPGRADE_CONFIG_CHANGED', 'Fixture pairing/versioning/maintenance state changed');
    if (reject) {
      // Native candidate is genuinely running and authenticated. Only the
      // compatibility expectation is injected; no claim of native failure.
      const observed = await second.request('/rest/system/version');
      assertSyncthingUpgradeHealth(observed, 'v' + candidateVersion);
      let rejected = false;
      try { assertSyncthingUpgradeHealth(observed, 'v2.1.999'); }
      catch (cause) { if (cause.code !== 'SYNCTHING_UPGRADE_HEALTH_REJECTED') throw cause; rejected = true; }
      if (!rejected) throw new PackageError('SYNCTHING_HEALTH_REJECTION_UNPROVEN', 'Injected version-health rejection not established');
      const restore = ownerStage ? ownerStage.restoreOwnerEngine : restoreSyncthingProfile;
      const restored = restore(snapshot, await stopSecond(), path.join(root, 'rejected-candidate'));
      second = await startSyncthingUpgradePilot(baseline, home, baselineVersion, true, baselineVersion);
      requireRetained(second, identity, cert, key, 'synthetic-before-upgrade\n');
      await folderPause(first, false); await folderPause(second, false);
      await transfer(first, second, 'after-rollback.txt', 'synthetic-after-cold-rollback\n');
      result = { selection: 'baseline', candidate_pid: candidatePid, restored_pid: second.pid,
        injected_health_rejection: true, ...restored, restored_baseline_pairing_transfer: true };
    } else {
      await folderPause(first, false); await folderPause(second, false);
      await transfer(first, second, 'after-upgrade.txt', 'synthetic-after-real-upgrade\n');
      await stopSecond();
      second = await startSyncthingUpgradePilot(candidate, home, candidateVersion, true, baselineVersion);
      requireRetained(second, identity, cert, key, 'synthetic-before-upgrade\n');
      if (readSyncthingFixture(path.join(home, 'payload', 'after-upgrade.txt')) !== 'synthetic-after-real-upgrade\n') throw new PackageError('SYNCTHING_UPGRADE_RETENTION_FAILED', 'Upgraded payload absent after restart');
      await transfer(first, second, 'after-upgrade-restart.txt', 'synthetic-after-upgraded-restart\n');
      result = { selection: 'candidate', candidate_pid: candidatePid, restarted_pid: second.pid,
        actual_changed_version: true, upgraded_pairing_transfer: true, upgraded_restart_transfer: true };
    }
    result.identity_key_payload_retained = true;
  } catch (cause) { failure = cause; }
  finally {
    const errors = [];
    for (const node of [second, first].filter(Boolean)) {
      try { stops.push(await node.stop()); } catch (cause) { errors.push(cause); }
    }
    if (errors.length) failure = new PackageError('SYNCTHING_UPGRADE_CLEANUP_FAILED', 'Owned clean shutdown failed', { cause: new AggregateError([...(failure ? [failure] : []), ...errors]) });
  }
  if (failure) throw failure;
  return { ...result, stops };
}
async function main() {
  const args = process.argv.slice(2);
  const ownerTrial = args[0] === '--m17-owner-transport';
  if (ownerTrial) args.shift();
  const baselineVersion = ownerTrial ? '2.1.5' : '2.1.2';
  const candidateVersion = ownerTrial ? '2.1.7' : '2.1.5';
  const values = {};
  for (let index = 0; index < args.length; index += 2) {
    const allowed = ['--root', '--baseline', '--candidate', ...(ownerTrial ? ['--native-stage-test'] : [])];
    if (!allowed.includes(args[index]) || !args[index + 1] || Object.hasOwn(values, args[index])) throw new PackageError('INVALID_SYNCTHING_UPGRADE_ARGS', 'Unique explicit trial flags required');
    values[args[index]] = args[index + 1];
  }
  const root = values['--root']; const baselineInput = values['--baseline']; const candidateInput = values['--candidate'];
  if (![root, baselineInput, candidateInput].every(value => typeof value === 'string' && path.isAbsolute(value))) throw new PackageError('INVALID_SYNCTHING_UPGRADE_ARGS', 'Absolute explicit inputs required');
  const base = process.platform === 'win32' ? path.join(process.env.LOCALAPPDATA ?? '', 'Cryptex-Dev', 'p0-4') : path.join(process.env.HOME ?? '', 'Library', 'Application Support', 'Cryptex-Dev', 'p0-4');
  if (path.dirname(root) !== base) throw new PackageError('INVALID_SYNCTHING_UPGRADE_ROOT', 'Fresh direct app-data child required');
  assertExperimentLocation(root); fs.mkdirSync(root, { mode: 0o700 });
  const baselineSource = fs.realpathSync(baselineInput);
  const baseline = path.join(root, process.platform === 'win32' ? 'baseline-syncthing.exe' : 'baseline-syncthing');
  const sourceInfo = fs.lstatSync(baselineSource);
  if (!sourceInfo.isFile() || (process.platform !== 'win32' && !(sourceInfo.mode & 0o111))) throw new PackageError('UNSAFE_SYNCTHING_UPGRADE_BINARY', 'Ordinary executable baseline required');
  const baselineHash = digest(baselineSource);
  fs.copyFileSync(baselineSource, baseline, fs.constants.COPYFILE_EXCL);
  if (process.platform !== 'win32') fs.chmodSync(baseline, sourceInfo.mode & 0o777);
  if (digest(baseline) !== baselineHash || digest(baselineSource) !== baselineHash) throw new PackageError('SYNCTHING_UPGRADE_BINARY_CHANGED', 'Baseline copy differs');
  const bundle = ownerTrial ? null : stageSyncthingBundle(fs.realpathSync(candidateInput), path.join(root, 'candidate-package'));
  const candidate = bundle?.binary ?? fs.realpathSync(candidateInput);
  const candidateHash = digest(candidate);
  if (ownerTrial) {
    const pins = {
      'win32-x64': ['36a0f7bc372f64fa7cc4f5654fa324c0dd9f7fef2e07565e00c6e1cf73f50344', 'cd9aea20afda2b9bdba5aff2e6ac964ed5c2e741ed4ceb48ad9e1be096ca7891'],
      'darwin-arm64': ['6de41132c8a3fc78fc57486df32f9ac69d6f53db1e1910bb46f548ee7f29725e', '38aab77073e47a717ec59eab082670b4fccf28c256fa0f7935aebb3f4665bb01'],
    };
    const approved = pins[process.platform + '-' + process.arch];
    if (!approved || approved[0] !== baselineHash || approved[1] !== candidateHash) throw new PackageError('SYNCTHING_PIN_MISMATCH', 'Owner trial requires the approved native executable pair');
  }
  if (candidateHash === baselineHash) throw new PackageError('SYNCTHING_UPGRADE_SAME_BYTES', 'Real distinct versions/bytes required');
  const globalConfig = process.platform === 'win32' ? path.join(process.env.LOCALAPPDATA, 'Syncthing', 'config.xml') : path.join(process.env.HOME, 'Library', 'Application Support', 'Syncthing', 'config.xml');
  const globalBefore = fs.existsSync(globalConfig) ? digest(globalConfig) : null;
  const nativeStageTest = values['--native-stage-test'];
  if (nativeStageTest && !path.isAbsolute(nativeStageTest)) throw new PackageError('INVALID_SYNCTHING_UPGRADE_ARGS', 'Absolute native test binary required');
  const compatible = await scenario(path.join(root, 'compatible'), baseline, candidate, false, baselineVersion, candidateVersion, nativeStageTest);
  const rejected = await scenario(path.join(root, 'health-reject'), baseline, candidate, true, baselineVersion, candidateVersion, nativeStageTest);
  if (bundle) verifySyncthingBundle(bundle.root);
  if (digest(candidate) !== candidateHash) throw new PackageError('SYNCTHING_UPGRADE_BINARY_CHANGED', 'Candidate changed during proof');
  if (digest(baseline) !== baselineHash || digest(baselineSource) !== baselineHash) throw new PackageError('SYNCTHING_UPGRADE_BINARY_CHANGED', 'Baseline changed during proof');
  if ((fs.existsSync(globalConfig) ? digest(globalConfig) : null) !== globalBefore) throw new PackageError('GLOBAL_SYNCTHING_CONFIG_CHANGED', 'Observed global config changed');
  const report = { schema: 1, scope: ownerTrial ? 'm17-owner-transport-trial' : 'm0-syncthing-upgrade-pilot', platform: process.platform, arch: process.arch, node: process.version,
    baseline_version: baselineVersion, candidate_version: candidateVersion, baseline_sha256: baselineHash, candidate_sha256: candidateHash,
    compatible, rejected, native_database_staged_by_cryptex: Boolean(nativeStageTest), global_config_unchanged: true, policy: 'quiesce-cold-snapshot-before-candidate; rejected-profile-quarantine-and-cold-restore',
    production_updater_verified: false, crash_atomicity_verified: false, power_loss_verified: false, publisher_authenticated: false, production_secret_storage_verified: false };
  fs.writeFileSync(path.join(root, 'syncthing-upgrade-report.json'), `${JSON.stringify(report, null, 2)}\n`, { flag: 'wx', mode: 0o600 });
  console.log(JSON.stringify(report));
}
try { await main(); }
catch (error) {
  function diagnostic(value) { return { code: value.code ?? value.name ?? 'UNKNOWN', diagnostics: value.diagnostics ?? null,
    causes: value.cause instanceof AggregateError ? value.cause.errors.map(diagnostic) : value.cause ? [diagnostic(value.cause)] : [] }; }
  console.error(JSON.stringify(diagnostic(error))); process.exitCode = 2;
}
