import path from 'node:path';
import { PackageError, assertExperimentLocation } from './package-transaction.mjs';
import { describeNativeTree } from './package-native-tree.mjs';

export function selectionProofInputs(args) {
  const required = ['--source', '--root', '--helper', '--desktop', '--daemon'];
  const flags = [...required, '--candidate-source']; const values = {};
  for (let index = 0; index < args.length; index += 2) {
    if (!flags.includes(args[index]) || !args[index + 1] || Object.hasOwn(values, args[index])) throw new PackageError('INVALID_SELECTION_PROOF_ARGS', 'Unique required flags and optional candidate source required');
    values[args[index]] = args[index + 1];
  }
  if (required.some(flag => !values[flag]) || ['--source', '--root', '--helper', ...(values['--candidate-source'] ? ['--candidate-source'] : [])].some(flag => !path.isAbsolute(values[flag]))) throw new PackageError('INVALID_SELECTION_PROOF_ARGS', 'Absolute input, candidate, fresh root and helper required');
  const root = values['--root']; const source = values['--source']; const candidateSource = values['--candidate-source'] ?? source;
  assertExperimentLocation(root);
  for (const input of [source, candidateSource]) for (const [parent, child] of [[root, input], [input, root]]) {
    const relative = path.relative(parent, child);
    if (!relative || (!relative.startsWith(`..${path.sep}`) && relative !== '..' && !path.isAbsolute(relative))) throw new PackageError('OVERLAPPING_NATIVE_ROOTS', 'Disjoint input and proof root required');
  }
  const roles = { desktop: values['--desktop'], daemon: values['--daemon'] };
  const baselineManifest = describeNativeTree(source, roles); const candidateManifest = describeNativeTree(candidateSource, roles);
  const roleDigests = manifest => Object.fromEntries(Object.entries(roles).map(([role, name]) => [role, manifest.entries.find(entry => entry.path === name).sha256]));
  const baselineDigests = roleDigests(baselineManifest); const candidateDigests = roleDigests(candidateManifest);
  const changedRoles = Object.fromEntries(Object.keys(roles).map(role => [role, baselineDigests[role] !== candidateDigests[role]]));
  if (values['--candidate-source'] && !Object.values(changedRoles).some(Boolean)) throw new PackageError('UNCHANGED_NATIVE_CANDIDATE', 'Changed-package proof requires different actual Desktop or daemon bytes');
  return { root, source, candidateSource, helper: values['--helper'], baselineManifest, candidateManifest, baselineDigests, candidateDigests, changedRoles };
}
