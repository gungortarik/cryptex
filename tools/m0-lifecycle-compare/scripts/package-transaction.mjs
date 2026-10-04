// M0 filesystem candidate only. No installed-app, startup or canonical-data access.
import * as fs from 'node:fs';
import path from 'node:path';
import { createHash, randomUUID } from 'node:crypto';
import { fileURLToPath } from 'node:url';

const repository = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../../..');
const names = ['desktop.bin', 'daemon.bin'];
const own = Object.hasOwn;
export class PackageError extends Error {
  constructor(code, message) { super(message); this.name = 'PackageError'; this.code = code; }
}
function fail(code, message) { throw new PackageError(code, message); }
export function digest(file) { return createHash('sha256').update(fs.readFileSync(file)).digest('hex'); }
function id(value) {
  if (typeof value !== 'string' || !/^[a-z0-9][a-z0-9-]{0,63}$/.test(value)) fail('INVALID_ID', 'Invalid slot identifier');
  return value;
}
function plain(file, directory = false) {
  const info = fs.lstatSync(file);
  if (info.isSymbolicLink() || !(directory ? info.isDirectory() : info.isFile())) fail('UNSAFE_PATH', 'Expected ordinary file/directory');
}
function safeAncestors(target) {
  let cursor = path.resolve(target);
  while (true) {
    if (fs.existsSync(cursor)) plain(cursor, true);
    const parent = path.dirname(cursor);
    if (parent === cursor) break;
    cursor = parent;
  }
}
function outsideRepository(root) {
  const relative = path.relative(repository, path.resolve(root));
  if (!relative || (!relative.startsWith(`..${path.sep}`) && relative !== '..' && !path.isAbsolute(relative))) fail('REPOSITORY_ROOT', 'Experiment must be outside repository');
  safeAncestors(root);
}
export function assertExperimentLocation(root) { outsideRepository(root); }
function readJson(file) {
  plain(file);
  try { return JSON.parse(fs.readFileSync(file, 'utf8')); }
  catch (error) { if (error instanceof SyntaxError) fail('CORRUPT_METADATA', 'Malformed experiment metadata'); throw error; }
}
function syncDirectory(root) {
  // Windows directory fsync is not portable through Node. Never claim power-loss durability.
  if (process.platform === 'win32') return;
  const fd = fs.openSync(root, 'r');
  try { fs.fsyncSync(fd); } finally { fs.closeSync(fd); }
}
function writeNew(file, value) {
  const fd = fs.openSync(file, 'wx');
  try { fs.writeFileSync(fd, `${JSON.stringify(value)}\n`, 'utf8'); fs.fsyncSync(fd); }
  finally { fs.closeSync(fd); }
  syncDirectory(path.dirname(file));
}
function replaceState(root, value) {
  const temporary = path.join(root, `selection-${randomUUID()}.tmp`);
  writeNew(temporary, value);
  fs.renameSync(temporary, path.join(root, 'selection.json'));
  syncDirectory(root);
}
function manifest(value) {
  if (!value || value.schema !== 1) fail('UNSUPPORTED_SCHEMA', 'Unsupported package manifest schema');
  id(value.id);
  for (const component of ['desktop', 'daemon']) {
    const item = value[component];
    if (!item || !Number.isSafeInteger(item.protocol) || item.protocol < 1 || typeof item.sha256 !== 'string' || !/^[a-f0-9]{64}$/.test(item.sha256)) fail('INVALID_MANIFEST', 'Invalid declared component metadata');
  }
  if (value.desktop.protocol !== value.daemon.protocol) fail('INCOMPATIBLE_PAIR', 'Declared Desktop/daemon protocols differ');
  return value;
}
export function describePackage(packageId, desktop, daemon, desktopProtocol = 1, daemonProtocol = 1) {
  return manifest({ schema: 1, id: packageId, desktop: { protocol: desktopProtocol, sha256: digest(desktop) }, daemon: { protocol: daemonProtocol, sha256: digest(daemon) } });
}
function verifySources(value, sources) {
  manifest(value);
  for (const component of ['desktop', 'daemon']) {
    const file = path.resolve(sources[component]);
    safeAncestors(path.dirname(file)); plain(file);
    if (digest(file) !== value[component].sha256) fail('PAYLOAD_MISMATCH', 'Payload does not match trusted experiment manifest');
  }
}
function stage(root, value, sources) {
  verifySources(value, sources);
  const slot = path.join(root, 'slots', value.id);
  fs.mkdirSync(slot); // Never overwrite an existing or partially staged slot.
  for (const [index, component] of ['desktop', 'daemon'].entries()) {
    const destination = path.join(slot, names[index]);
    fs.copyFileSync(sources[component], destination, fs.constants.COPYFILE_EXCL);
    const fd = fs.openSync(destination, 'r+');
    try { fs.fsyncSync(fd); } finally { fs.closeSync(fd); }
  }
  writeNew(path.join(slot, 'manifest.json'), value);
  verifySlot(root, value.id);
  syncDirectory(path.join(root, 'slots'));
}
function validateRoot(root) {
  outsideRepository(root);
  const marker = readJson(path.join(root, 'experiment.json'));
  if (marker.schema !== 1 || marker.scope !== 'm0-package-filesystem-candidate') fail('UNOWNED_ROOT', 'Not an owned package experiment');
  plain(path.join(root, 'slots'), true);
}
function verifySlot(root, slotId) {
  id(slotId);
  const slot = path.join(root, 'slots', slotId);
  plain(slot, true);
  const expectedFiles = ['manifest.json', ...names].sort();
  if (JSON.stringify(fs.readdirSync(slot).sort()) !== JSON.stringify(expectedFiles)) fail('INCOMPLETE_SLOT', 'Package slot is incomplete or contains unexpected files');
  const value = manifest(readJson(path.join(slot, 'manifest.json')));
  if (value.id !== slotId) fail('INVALID_MANIFEST', 'Slot and manifest identity differ');
  verifySources(value, { desktop: path.join(slot, names[0]), daemon: path.join(slot, names[1]) });
  return value;
}
function selection(value) {
  if (!value || value.schema !== 1 || !own(value, 'previous')) fail('CORRUPT_METADATA', 'Invalid selection schema');
  id(value.active);
  if (value.previous !== null) id(value.previous);
  if (value.previous === value.active) fail('CORRUPT_METADATA', 'Active and previous slots must differ');
  return { schema: 1, active: value.active, previous: value.previous };
}
function same(a, b) { return a.active === b.active && a.previous === b.previous; }
export function selected(root) {
  validateRoot(root);
  const value = selection(readJson(path.join(root, 'selection.json')));
  verifySlot(root, value.active);
  if (value.previous !== null) verifySlot(root, value.previous);
  return value;
}
export function initialize(root, value, sources) {
  outsideRepository(root); verifySources(value, sources);
  fs.mkdirSync(root); // Explicit fresh root required; do not adopt or clear an existing directory.
  fs.mkdirSync(path.join(root, 'slots'));
  writeNew(path.join(root, 'experiment.json'), { schema: 1, scope: 'm0-package-filesystem-candidate' });
  stage(root, value, sources);
  writeNew(path.join(root, 'selection.json'), { schema: 1, active: value.id, previous: null });
  return selected(root);
}
export function recover(root) {
  validateRoot(root);
  const intent = path.join(root, 'pending.json');
  if (!fs.existsSync(intent)) return { recovered: false, selection: selected(root) };
  const journal = readJson(intent);
  if (!journal || journal.schema !== 1) fail('CORRUPT_JOURNAL', 'Invalid transaction journal schema');
  const before = selection(journal.before);
  const after = selection(journal.after);
  if (after.previous !== before.active || after.active === before.active) fail('CORRUPT_JOURNAL', 'Invalid transaction relationship');
  const current = selection(readJson(path.join(root, 'selection.json')));
  if (!same(current, before) && !same(current, after)) fail('CORRUPT_JOURNAL', 'Selection is not associated with pending transaction');
  // Never discard malformed state or silently launch a corrupt last-known-good.
  verifySlot(root, before.active);
  if (before.previous !== null) verifySlot(root, before.previous);
  replaceState(root, before);
  fs.unlinkSync(intent); syncDirectory(root);
  return { recovered: true, selection: selected(root) };
}
export async function transition(root, value, sources, { checkpoint = async () => {}, health } = {}) {
  if (typeof health !== 'function') fail('HEALTH_REQUIRED', 'An explicit experiment health check is required');
  validateRoot(root);
  if (fs.existsSync(path.join(root, 'pending.json'))) fail('RECOVERY_REQUIRED', 'Recover pending transaction before retry');
  const before = selected(root);
  stage(root, value, sources);
  await checkpoint('staged');
  verifySlot(root, value.id);
  // Another updater may have committed while this one staged its private slot.
  // Revalidate after staging; the exclusive intent create below arbitrates
  // overlapping transitions that still race past this read.
  if (!same(selected(root), before)) fail('CONCURRENT_TRANSITION', 'Package selection changed while candidate was staged');
  await checkpoint('selection-validated');
  const after = { schema: 1, active: value.id, previous: before.active };
  const intent = path.join(root, 'pending.json');
  const journal = { schema: 1, before, after };
  try { writeNew(intent, journal); }
  catch (error) {
    if (error?.code === 'EEXIST') fail('CONCURRENT_TRANSITION', 'Another transaction owns the pending intent');
    throw error;
  }
  // Close the race where another invocation commits between our preflight read
  // and exclusive intent creation. Keep unexpected/corrupt intent data for
  // diagnosis; remove only the journal content we just created.
  if (!same(selected(root), before)) {
    if (JSON.stringify(readJson(intent)) !== JSON.stringify(journal)) fail('CORRUPT_JOURNAL', 'Concurrent transition changed the owned intent');
    fs.unlinkSync(intent);
    syncDirectory(root);
    fail('CONCURRENT_TRANSITION', 'Package selection changed before transaction intent was secured');
  }
  await checkpoint('intent');
  replaceState(root, after);
  await checkpoint('selected');
  let healthy;
  try { healthy = await health(path.join(root, 'slots', value.id), manifest(value)); }
  catch (error) {
    recover(root);
    throw new PackageError('HEALTH_FAILED', `Experiment health check threw (${error?.name ?? 'unknown'}); previous package restored`);
  }
  if (healthy !== true) { recover(root); fail('HEALTH_FAILED', 'Experiment health rejected candidate; previous package restored'); }
  try { verifySlot(root, value.id); }
  catch (error) { recover(root); throw error; }
  await checkpoint('healthy');
  fs.unlinkSync(path.join(root, 'pending.json')); syncDirectory(root);
  await checkpoint('committed');
  return selected(root);
}
