// Test-only native package inventory. No installed application or canonical paths.
import * as fs from 'node:fs';
import path from 'node:path';
import { assertExperimentLocation, digest, PackageError } from './package-transaction.mjs';

const scope = 'm0-native-package-tree';
function fail(code) { throw new PackageError(code, 'Native package validation failed'); }
function relativeName(value) {
  if (typeof value !== 'string' || !value || value.normalize('NFC') !== value || value.includes('\\')) fail('INVALID_NATIVE_PATH');
  for (const part of value.split('/')) {
    if (!part || part === '.' || part === '..' || /[<>:"|?*\x00-\x1f]/.test(part) || /[. ]$/.test(part)
      || /^(con|prn|aux|nul|com[1-9]|lpt[1-9])(?:\.|$)/i.test(part)) fail('INVALID_NATIVE_PATH');
  }
  return value;
}
function rootPath(root) {
  if (typeof root !== 'string' || !path.isAbsolute(root)) fail('INVALID_NATIVE_ROOT');
  assertExperimentLocation(root);
}
function target(value) {
  if (!value || value.platform !== process.platform || value.arch !== process.arch
    || !['win32', 'darwin'].includes(value.platform)) fail('NATIVE_TARGET_MISMATCH');
}
function inventory(root) {
  rootPath(root);
  const entries = []; const seen = new Set();
  function visit(directory, prefix) {
    for (const name of fs.readdirSync(directory).sort()) {
      const relative = relativeName(prefix ? `${prefix}/${name}` : name);
      const folded = relative.toLowerCase();
      if (seen.has(folded)) fail('NATIVE_PATH_COLLISION');
      seen.add(folded);
      if (entries.length >= 10000) fail('NATIVE_TREE_LIMIT');
      const file = path.join(directory, name); const stat = fs.lstatSync(file);
      if (stat.isSymbolicLink() || (!stat.isFile() && !stat.isDirectory())) fail('UNSAFE_NATIVE_ENTRY');
      if (process.platform !== 'win32' && (stat.mode & 0o7000)) fail('UNSAFE_NATIVE_MODE');
      const entry = { path: relative, kind: stat.isDirectory() ? 'directory' : 'file', mode: process.platform === 'win32' ? null : stat.mode & 0o777 };
      if (stat.isFile()) { entry.size = stat.size; entry.sha256 = digest(file); }
      entries.push(entry);
      if (stat.isDirectory()) visit(file, relative);
    }
  }
  visit(root, '');
  return entries;
}
function validate(manifest) {
  if (!manifest || manifest.schema !== 1 || manifest.scope !== scope) fail('UNSUPPORTED_NATIVE_SCHEMA');
  target(manifest.target);
  if (!Number.isSafeInteger(manifest.protocol) || manifest.protocol < 1 || manifest.protocol > 0xffffffff
    || !Array.isArray(manifest.entries) || manifest.entries.length < 2 || manifest.entries.length > 10000) fail('INVALID_NATIVE_MANIFEST');
  const entries = new Map(); const folded = new Set();
  for (const entry of manifest.entries) {
    const name = relativeName(entry?.path);
    if (folded.has(name.toLowerCase())) fail('NATIVE_PATH_COLLISION');
    folded.add(name.toLowerCase()); entries.set(name, entry);
    if (!['file', 'directory'].includes(entry.kind)) fail('INVALID_NATIVE_MANIFEST');
    if (process.platform === 'win32' ? entry.mode !== null : !Number.isInteger(entry.mode) || entry.mode < 0 || entry.mode > 0o777) fail('INVALID_NATIVE_MANIFEST');
    if (entry.kind === 'file' && (!Number.isSafeInteger(entry.size) || entry.size < 0 || !/^[a-f0-9]{64}$/.test(entry.sha256))) fail('INVALID_NATIVE_MANIFEST');
  }
  for (const name of entries.keys()) {
    const parent = name.includes('/') ? name.slice(0, name.lastIndexOf('/')) : null;
    if (parent && entries.get(parent)?.kind !== 'directory') fail('INVALID_NATIVE_MANIFEST');
  }
  const roles = manifest.roles;
  if (!roles || roles.desktop === roles.daemon) fail('INVALID_NATIVE_ROLES');
  for (const role of ['desktop', 'daemon']) {
    const entry = entries.get(relativeName(roles[role]));
    if (!entry || entry.kind !== 'file') fail('INVALID_NATIVE_ROLES');
    if (process.platform !== 'win32' && !(entry.mode & 0o111)) fail('NATIVE_NOT_EXECUTABLE');
  }
  return manifest;
}
export function describeNativeTree(root, roles, protocol = 1) {
  return validate({ schema: 1, scope, target: { platform: process.platform, arch: process.arch }, protocol,
    roles: { ...roles }, entries: inventory(root) });
}
export function verifyNativeTree(root, manifest) {
  validate(manifest);
  const actual = inventory(root);
  const sorted = list => [...list].sort((a, b) => a.path < b.path ? -1 : a.path > b.path ? 1 : 0);
  const comparable = entry => [entry.path, entry.kind, entry.mode, entry.kind === 'file' ? entry.size : null, entry.kind === 'file' ? entry.sha256 : null];
  if (JSON.stringify(sorted(actual).map(comparable)) !== JSON.stringify(sorted(manifest.entries).map(comparable))) fail('NATIVE_TREE_MISMATCH');
  return { verified: true, entry_count: actual.length, publisher_authenticated: false };
}
export function stageNativeTree(source, destination, manifest, io = fs) {
  rootPath(source); rootPath(destination);
  const contains = (parent, child) => { const relative = path.relative(parent, child); return !relative || (!relative.startsWith(`..${path.sep}`) && relative !== '..' && !path.isAbsolute(relative)); };
  if (contains(source, destination) || contains(destination, source)) fail('OVERLAPPING_NATIVE_ROOTS');
  verifyNativeTree(source, manifest); // Validate before creating any destination.
  if (fs.existsSync(destination)) fail('NATIVE_DESTINATION_EXISTS');
  fs.mkdirSync(destination);
  try {
    const directories = manifest.entries.filter(e => e.kind === 'directory').sort((a, b) => a.path.split('/').length - b.path.split('/').length);
    for (const entry of directories) fs.mkdirSync(path.join(destination, ...entry.path.split('/')));
    for (const entry of manifest.entries.filter(e => e.kind === 'file')) {
      const components = entry.path.split('/'); const output = path.join(destination, ...components);
      io.copyFileSync(path.join(source, ...components), output, fs.constants.COPYFILE_EXCL);
      if (entry.mode !== null) fs.chmodSync(output, entry.mode);
    }
    // Apply restrictive directory permissions after populating their descendants.
    if (process.platform !== 'win32') for (const entry of [...directories].reverse()) fs.chmodSync(path.join(destination, ...entry.path.split('/')), entry.mode);
    verifyNativeTree(source, manifest); verifyNativeTree(destination, manifest);
    return { staged: true, root: destination, entry_count: manifest.entries.length, publisher_authenticated: false };
  } catch (cause) {
    const error = new PackageError('NATIVE_STAGE_FAILED', 'Partial native staging retained for inspection', { cause });
    error.diagnostics = { phase: 'stage', destination_retained: true, cause_code: cause.code ?? 'UNKNOWN' };
    throw error;
  }
}
