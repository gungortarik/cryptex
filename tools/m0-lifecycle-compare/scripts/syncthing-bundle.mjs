// M0 staged package proof only; no installed-app or publisher-authentication claim.
import * as fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { PackageError, assertExperimentLocation, digest } from './package-transaction.mjs';

const notices = {
  'LICENSE.txt': '9221c2f936159b8446d329249fb4c0f25be510f447383a0f13336ac7985668a3',
  'AUTHORS.txt': '2c197afd6113ec13ae134a20ce8f101d9812b078c3ef32c42c3bb4cdaf2b96a2',
};
const noticeRoot = fileURLToPath(new URL('../../../third_party/syncthing/2.1.5/', import.meta.url));
function ordinary(file) {
  const info = fs.lstatSync(file);
  if (info.isSymbolicLink() || !info.isFile()) throw new PackageError('UNSAFE_SYNCTHING_BUNDLE_FILE', 'Ordinary native bundle file required');
  return info;
}
export function stageSyncthingBundle(binary, root) {
  if (!path.isAbsolute(binary ?? '') || !path.isAbsolute(root ?? '')) throw new PackageError('INVALID_SYNCTHING_BUNDLE_ARGS', 'Absolute binary/fresh bundle root required');
  assertExperimentLocation(root);
  const info = ordinary(binary);
  if (process.platform !== 'win32' && !(info.mode & 0o111)) throw new PackageError('SYNCTHING_BINARY_NOT_EXECUTABLE', 'Native executable mode required');
  const binaryHash = digest(binary);
  fs.mkdirSync(root, { mode: 0o700 });
  const name = process.platform === 'win32' ? 'syncthing.exe' : 'syncthing';
  const entries = [];
  for (const [target, source, expected] of [[name, binary, binaryHash], ...Object.entries(notices).map(([file, hash]) => [file, path.join(noticeRoot, file), hash])]) {
    const sourceInfo = ordinary(source);
    if (digest(source) !== expected) throw new PackageError('SYNCTHING_BUNDLE_SOURCE_CHANGED', 'Input/notice digest changed');
    const destination = path.join(root, target);
    fs.copyFileSync(source, destination, fs.constants.COPYFILE_EXCL);
    if (process.platform !== 'win32') fs.chmodSync(destination, sourceInfo.mode & 0o777);
    if (digest(destination) !== expected || digest(source) !== expected) throw new PackageError('SYNCTHING_BUNDLE_SOURCE_CHANGED', 'Copy/source correspondence changed');
    entries.push({ name: target, sha256: expected, executable: target === name || Boolean(sourceInfo.mode & 0o111) });
  }
  const manifest = { schema: 1, scope: 'm0-syncthing-native-bundle', pin: '2.1.5', platform: process.platform, arch: process.arch,
    binary: name, entries, publisher_authenticated: false, installed_distribution: false };
  fs.writeFileSync(path.join(root, 'bundle.json'), `${JSON.stringify(manifest, null, 2)}\n`, { flag: 'wx' });
  return verifySyncthingBundle(root);
}
export function verifySyncthingBundle(root) {
  if (!path.isAbsolute(root ?? '')) throw new PackageError('INVALID_SYNCTHING_BUNDLE_ARGS', 'Absolute bundle root required');
  assertExperimentLocation(root); ordinary(path.join(root, 'bundle.json'));
  let manifest;
  try { manifest = JSON.parse(fs.readFileSync(path.join(root, 'bundle.json'), 'utf8')); }
  catch (cause) { throw new PackageError('CORRUPT_SYNCTHING_BUNDLE', 'Invalid bundle metadata', { cause }); }
  const name = process.platform === 'win32' ? 'syncthing.exe' : 'syncthing';
  if (!manifest || manifest.schema !== 1 || manifest.scope !== 'm0-syncthing-native-bundle' || manifest.pin !== '2.1.5'
    || manifest.platform !== process.platform || manifest.arch !== process.arch || manifest.binary !== name
    || manifest.publisher_authenticated !== false || manifest.installed_distribution !== false
    || !Array.isArray(manifest.entries) || manifest.entries.length !== 3
    || manifest.entries.some(entry => !entry || typeof entry.name !== 'string')) throw new PackageError('INVALID_SYNCTHING_BUNDLE', 'Native role/platform/pin mismatch');
  const expectedNames = [name, ...Object.keys(notices)].sort();
  if (JSON.stringify(manifest.entries.map(entry => entry.name).sort()) !== JSON.stringify(expectedNames)
    || JSON.stringify(fs.readdirSync(root).sort()) !== JSON.stringify([...expectedNames, 'bundle.json'].sort())) throw new PackageError('INVALID_SYNCTHING_BUNDLE', 'Exact native/notice membership required');
  for (const entry of manifest.entries) {
    const file = path.join(root, entry.name); const info = ordinary(file);
    if (!/^[a-f0-9]{64}$/.test(entry.sha256) || digest(file) !== entry.sha256 || (notices[entry.name] && entry.sha256 !== notices[entry.name])) throw new PackageError('SYNCTHING_BUNDLE_DIGEST_MISMATCH', 'Native/notice digest mismatch');
    if (typeof entry.executable !== 'boolean' || (process.platform !== 'win32' && Boolean(info.mode & 0o111) !== entry.executable)
      || (entry.name === name && !entry.executable)) throw new PackageError('SYNCTHING_BUNDLE_MODE_MISMATCH', 'Native executable mode mismatch');
  }
  return { root, binary: path.join(root, name), manifest };
}
