// Read-only source inventory and isolated copy; never launches package binaries.
import * as fs from 'node:fs';
import path from 'node:path';
import { assertExperimentLocation, PackageError } from './package-transaction.mjs';
import { describeNativeTree, stageNativeTree, verifyNativeTree } from './package-native-tree.mjs';

function main() {
  const options = {}; const args = process.argv.slice(2);
  for (let index = 0; index < args.length; index += 2) {
    const flag = args[index]; const value = args[index + 1];
    if (!['--source', '--root', '--desktop', '--daemon'].includes(flag) || !value || Object.hasOwn(options, flag)) throw new PackageError('INVALID_PROOF_ARGUMENTS', 'Four unique documented flags required');
    options[flag] = value;
  }
  if (Object.keys(options).length !== 4 || !path.isAbsolute(options['--source']) || !path.isAbsolute(options['--root'])) throw new PackageError('INVALID_PROOF_ARGUMENTS', 'Absolute source and fresh experiment root required');
  const source = options['--source']; const root = options['--root']; assertExperimentLocation(root);
  const manifest = describeNativeTree(source, { desktop: options['--desktop'], daemon: options['--daemon'] });
  // Refuse overlap before writing the ownership marker.
  for (const [parent, child] of [[source, root], [root, source]]) {
    const relative = path.relative(parent, child);
    if (!relative || (!relative.startsWith(`..${path.sep}`) && relative !== '..' && !path.isAbsolute(relative))) throw new PackageError('OVERLAPPING_NATIVE_ROOTS', 'Source and experiment roots must be disjoint');
  }
  fs.mkdirSync(root);
  fs.writeFileSync(path.join(root, 'experiment.json'), `${JSON.stringify({ schema: 1, scope: 'm0-native-tree-proof' })}\n`, { flag: 'wx' });
  const result = stageNativeTree(source, path.join(root, 'payload'), manifest);
  const report = { schema: 1, scope: 'm0-native-tree-proof', node: process.version, platform: process.platform, arch: process.arch,
    source, manifest, stage: result, source_after: verifyNativeTree(source, manifest), staged_after: verifyNativeTree(result.root, manifest),
    binaries_executed: false, publisher_authenticated: false, handover_verified: false, canonical_data_acceptance: false };
  fs.writeFileSync(path.join(root, 'native-tree-report.json'), `${JSON.stringify(report, null, 2)}\n`, { flag: 'wx' });
  console.log(JSON.stringify(report));
}
try { main(); }
catch (error) { console.error(JSON.stringify({ scope: 'm0-native-tree-proof', code: error.code ?? 'NATIVE_PROOF_FAILED', diagnostics: error.diagnostics ?? null })); process.exitCode = 2; }
