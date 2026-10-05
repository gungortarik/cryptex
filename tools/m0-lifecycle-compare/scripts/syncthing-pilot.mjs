// M0 P0-4 test-only external pinned binary; no global profile or canonical path.
import * as fs from 'node:fs';
import path from 'node:path';
import net from 'node:net';
import { spawn, execFile } from 'node:child_process';
import { promisify } from 'node:util';
import { setTimeout as pause } from 'node:timers/promises';
import { PackageError, assertExperimentLocation } from './package-transaction.mjs';
const execute = promisify(execFile);
// Pinned v2.1.5 protocol CloseTimeout is 10s; its database close wait is
// another 10s. Include both stages and 5s monitor/OS scheduling margin.
export const SYNCTHING_STOP_BUDGET_MS = 25000;
export function assertSyncthingVersion(version) {
  if (!/^syncthing v2\.1\.5 /.test(version)) throw new PackageError('SYNCTHING_PIN_MISMATCH', 'Stable Syncthing 2.1.5 baseline required');
}

export function isolateSyncthingXml(xml, apiPort, syncPort) {
  if (![apiPort, syncPort].every(port => Number.isInteger(port) && port > 1024 && port < 65536) || apiPort === syncPort) throw new PackageError('INVALID_SYNCTHING_PORT', 'Distinct loopback ports required');
  const gui = xml.match(/<gui\b[^>]*>[\s\S]*?<\/gui>/g);
  const opts = xml.match(/<options>[\s\S]*?<\/options>/g);
  if (gui?.length !== 1 || opts?.length !== 1) throw new PackageError('INVALID_SYNCTHING_CONFIG', 'Unique generated GUI/options required');
  if ((gui[0].match(/<address>[^<]*<\/address>/g) ?? []).length !== 1) throw new PackageError('INVALID_SYNCTHING_CONFIG', 'Unique GUI listener required');
  let options = opts[0];
  const values = { listenAddress: `tcp://127.0.0.1:${syncPort}`, globalAnnounceEnabled: 'false', localAnnounceEnabled: 'false',
    relaysEnabled: 'false', natEnabled: 'false', autoUpgradeIntervalH: '0', urAccepted: '-1' };
  for (const [name, value] of Object.entries(values)) {
    const pattern = new RegExp(`<${name}>[^<]*<\\/${name}>`, 'g');
    const matches = options.match(pattern);
    if (!matches?.length) throw new PackageError('INVALID_SYNCTHING_CONFIG', 'Missing generated isolation option');
    let first = true;
    options = options.replace(pattern, () => { if (!first) return ''; first = false; return `<${name}>${value}</${name}>`; });
  }
  const privateGui = gui[0].replace(/<address>[^<]*<\/address>/, `<address>127.0.0.1:${apiPort}</address>`);
  // Generated defaults stay templates; no active default ~/Sync folder is used.
  const head = xml.slice(0, xml.indexOf('<defaults>'));
  const tail = xml.slice(xml.indexOf('<defaults>'));
  if (!xml.includes('<defaults>')) throw new PackageError('INVALID_SYNCTHING_CONFIG', 'Generated defaults required');
  return (head.replace(/<folder\b[^>]*>[\s\S]*?<\/folder>/g, '') + tail).replace(gui[0], privateGui).replace(opts[0], options);
}
async function port() {
  const server = net.createServer();
  await new Promise((resolve, reject) => { server.once('error', reject); server.listen(0, '127.0.0.1', resolve); });
  const number = server.address().port; await new Promise(resolve => server.close(resolve)); return number;
}
export async function startSyncthingPilot(binary, home, reuse = false) {
  if (!path.isAbsolute(binary ?? '') || !path.isAbsolute(home ?? '')) throw new PackageError('INVALID_SYNCTHING_ARGS', 'Absolute binary/fresh home required');
  assertExperimentLocation(home);
  const version = (await execute(binary, ['--version'], { windowsHide: true, timeout: 5000 })).stdout.trim();
  assertSyncthingVersion(version);
  if (!reuse) fs.mkdirSync(home, { mode: 0o700 });
  const env = { ...process.env, STHOMEDIR: home, STNODEFAULTFOLDER: '1' };
  // Clear user overrides that could redirect config, data, API or logs.
  for (const key of Object.keys(env)) if (key.startsWith('ST') && !['STHOMEDIR', 'STNODEFAULTFOLDER'].includes(key)) delete env[key];
  if (!reuse) await execute(binary, ['generate', '--home', home, '--no-port-probing'], { windowsHide: true, timeout: 10000, env });
  const configFile = path.join(home, 'config.xml');
  const configInfo = fs.lstatSync(configFile);
  if (!configInfo.isFile() || configInfo.isSymbolicLink()) throw new PackageError('UNSAFE_SYNCTHING_CONFIG', 'Ordinary generated config required');
  const original = fs.readFileSync(configFile, 'utf8');
  const apiKey = /<apikey>([^<]+)<\/apikey>/.exec(original)?.[1];
  if (!apiKey) throw new PackageError('SYNCTHING_API_KEY_MISSING', 'Generated authentication key missing');
  let apiPort; let syncPort;
  if (reuse) {
    const markerFile = path.join(home, 'pilot-owned.json');
    const info = fs.lstatSync(markerFile);
    if (info.isSymbolicLink() || !info.isFile()) throw new PackageError('UNOWNED_SYNCTHING_HOME', 'Ordinary pilot ownership marker required');
    const marker = JSON.parse(fs.readFileSync(markerFile, 'utf8'));
    if (marker.schema !== 1 || marker.scope !== 'm0-managed-syncthing-pilot' || marker.pin !== '2.1.5') throw new PackageError('UNOWNED_SYNCTHING_HOME', 'Pinned pilot ownership marker required');
    ({ apiPort, syncPort } = marker);
    if (![apiPort, syncPort].every(value => Number.isInteger(value) && value > 1024 && value < 65536) || apiPort === syncPort) throw new PackageError('INVALID_SYNCTHING_PORT', 'Invalid retained ports');
  } else {
    apiPort = await port(); syncPort = await port(); while (syncPort === apiPort) syncPort = await port();
    fs.writeFileSync(configFile, isolateSyncthingXml(original, apiPort, syncPort));
    fs.writeFileSync(path.join(home, 'pilot-owned.json'), JSON.stringify({ schema: 1, scope: 'm0-managed-syncthing-pilot', pin: '2.1.5', apiPort, syncPort }), { flag: 'wx' });
  }
  const base = `http://127.0.0.1:${apiPort}`;
  const child = spawn(binary, ['serve', '--home', home, '--no-browser', '--no-restart', '--no-upgrade', '--no-port-probing', '--log-file', path.join(home, 'native.log')], { windowsHide: true, detached: process.platform !== 'win32', env, stdio: ['ignore', 'pipe', 'pipe'] });
  child.stdout.on('data', () => {}); child.stderr.on('data', () => {});
  let outcome = null;
  const closed = new Promise(resolve => {
    child.once('error', error => { outcome = { spawn_error: error.code }; resolve(outcome); });
    child.once('close', (code, signal) => { outcome ??= { code, signal }; resolve(outcome); });
  });
  async function request(endpoint, method = 'GET', body) {
    if (!endpoint.startsWith('/rest/') || endpoint.includes('://')) throw new PackageError('INVALID_SYNCTHING_ENDPOINT', 'Only fixed local REST paths permitted');
    const response = await fetch(`${base}${endpoint}`, { method, signal: AbortSignal.timeout(3000), headers: { 'X-API-Key': apiKey, Connection: 'close', ...(body ? { 'Content-Type': 'application/json' } : {}) }, body: body ? JSON.stringify(body) : undefined });
    if (!response.ok) { const failure = new PackageError('SYNCTHING_REST_FAILED', 'Owned local REST request rejected'); failure.diagnostics = { status: response.status }; throw failure; }
    const text = await response.text(); if (text.length > 1_000_000) throw new PackageError('SYNCTHING_RESPONSE_LIMIT', 'REST response too large');
    return text.trim() ? JSON.parse(text) : null;
  }
  let stopped;
  async function stop() {
    if (stopped) return stopped;
    stopped = (async () => {
      let timer;
      const started = Date.now();
      const logFile = path.join(home, 'native.log');
      const logOffset = fs.existsSync(logFile) ? fs.statSync(logFile).size : 0;
      try {
        if (outcome === null) {
          const folders = await request('/rest/config/folders');
          for (const folder of folders) {
            if (folder.id !== 'm0-managed-fixture' || folder.path !== path.join(home, 'payload')) throw new PackageError('SYNCTHING_STOP_SCOPE_MISMATCH', 'Cannot quiesce an unexpected folder');
          }
          await execute(binary, ['cli', '--home', home, 'operations', 'shutdown'], { windowsHide: true, timeout: 5000, env });
        }
        const result = await Promise.race([closed, new Promise((_, reject) => { timer = setTimeout(() => {
          const failure = new PackageError('SYNCTHING_STOP_TIMEOUT', 'Owned process did not exit');
          let processPresent = true; try { process.kill(child.pid, 0); } catch (cause) { if (cause.code === 'ESRCH') processPresent = false; }
          failure.diagnostics = { pid: child.pid, process_present: processPresent, observed_exit_code: child.exitCode }; reject(failure);
        }, SYNCTHING_STOP_BUDGET_MS); })]);
        if (result.code !== 0 || result.signal !== null) {
          const failure = new PackageError('SYNCTHING_UNCLEAN_EXIT', 'Owned monitor did not exit cleanly'); failure.diagnostics = result; throw failure;
        }
        const shutdownLog = fs.readFileSync(logFile).subarray(logOffset).toString('utf8');
        if (shutdownLog.includes('Database failed to stop within 10s')) throw new PackageError('SYNCTHING_DATABASE_STOP_FAILED', 'Native database stop failed despite exit status');
        try { process.kill(child.pid, 0); } catch (cause) { if (cause.code === 'ESRCH') return { pid: child.pid, exit_code: 0, process_absent: true, clean_shutdown_ms: Date.now() - started }; throw cause; }
        throw new PackageError('SYNCTHING_PROCESS_REMAINS', 'Owned monitor PID remains or reused');
      } catch (cause) {
        if (outcome === null) {
          if (process.platform === 'win32') await execute('taskkill', ['/PID', String(child.pid), '/T', '/F'], { windowsHide: true, timeout: 5000 });
          else process.kill(-child.pid, 'SIGKILL');
          await Promise.race([closed, new Promise((_, reject) => { timer = setTimeout(() => reject(new PackageError('SYNCTHING_FORCED_STOP_TIMEOUT', 'Owned process tree cleanup timed out')), 5000); })]);
        }
        throw new PackageError('SYNCTHING_FORCED_CLEANUP', 'Clean stop failed; owned process cleanup is not a PASS', { cause });
      } finally { clearTimeout(timer); }
    })(); return stopped;
  }
  try {
    const deadline = Date.now() + 12000; let status;
    while (Date.now() < deadline) {
      if (outcome !== null) throw new PackageError('SYNCTHING_START_FAILED', 'Owned process exited before API readiness');
      try { status = await request('/rest/system/status'); break; }
      catch (cause) { if (!(cause instanceof TypeError) && cause.name !== 'TimeoutError') throw cause; await pause(100); }
    }
    if (!status?.myID) throw new PackageError('SYNCTHING_READY_TIMEOUT', 'Authenticated REST not ready');
    const config = await request('/rest/config');
    if ((!reuse && config.folders.length !== 0) || config.folders.some(folder => folder.id !== 'm0-managed-fixture' || folder.path !== path.join(home, 'payload'))
      || config.options.globalAnnounceEnabled || config.options.localAnnounceEnabled || config.options.relaysEnabled || config.options.natEnabled
      || config.gui.address !== `127.0.0.1:${apiPort}` || config.gui.insecureAdminAccess
      || config.options.listenAddresses?.length !== 1 || config.options.listenAddresses[0] !== `tcp://127.0.0.1:${syncPort}`
      || config.options.autoUpgradeIntervalH !== 0 || config.options.urAccepted !== -1) throw new PackageError('SYNCTHING_ISOLATION_UNPROVEN', 'External transport, override or unexpected folder enabled');
    const unauthorized = await fetch(`${base}/rest/system/status`, { signal: AbortSignal.timeout(3000), headers: { 'X-API-Key': 'incorrect-fixture-key', Connection: 'close' } });
    await unauthorized.arrayBuffer();
    if (![401, 403].includes(unauthorized.status)) throw new PackageError('SYNCTHING_AUTH_UNPROVEN', 'Unauthenticated API was not rejected');
    return { home, id: status.myID, pid: child.pid, syncPort, version, auth_rejection: unauthorized.status, request, stop };
  } catch (cause) {
    try { await stop(); }
    catch (cleanup) {
      if (outcome === null) {
        if (process.platform === 'win32') await execute('taskkill', ['/PID', String(child.pid), '/T', '/F'], { windowsHide: true, timeout: 5000 });
        else process.kill(-child.pid, 'SIGKILL');
        await closed;
      }
      throw new PackageError('SYNCTHING_START_CLEANUP_FAILED', 'Owned start and clean shutdown failed', { cause: new AggregateError([cause, cleanup]) });
    }
    throw cause;
  }
}
