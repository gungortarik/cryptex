import { test } from 'node:test';
import assert from 'node:assert/strict';
import * as fs from 'node:fs';
import path from 'node:path';
import { createFixtureRoot } from './package-transaction-fixtures.mjs';
import { selectionProofInputs } from './native-selection-proof-inputs.mjs';
function fixture(t) {
  const root = createFixtureRoot(); t.after(() => fs.rmSync(root, { recursive: true, force: true }));
  for (const name of ['baseline', 'candidate']) {
    const input = path.join(root, name); fs.mkdirSync(input);
    for (const role of ['desktop', 'daemon']) fs.writeFileSync(path.join(input, role), role, { mode: 0o700 });
  }
  const args = ['--source', path.join(root, 'baseline'), '--root', path.join(root, 'proof'), '--helper', process.execPath, '--desktop', 'desktop', '--daemon', 'daemon'];
  return { root, args, candidate: path.join(root, 'candidate') };
}
test('same-source historical proof remains explicitly unchanged', t => {
  const input = fixture(t); const result = selectionProofInputs(input.args);
  assert.equal(result.source, result.candidateSource); assert.deepEqual(result.changedRoles, { desktop: false, daemon: false });
  assert.equal(fs.existsSync(result.root), false);
});
test('different actual Desktop bytes identify changed role without claiming daemon change', t => {
  const input = fixture(t); fs.appendFileSync(path.join(input.candidate, 'desktop'), '-candidate');
  const result = selectionProofInputs([...input.args, '--candidate-source', input.candidate]);
  assert.deepEqual(result.changedRoles, { desktop: true, daemon: false }); assert.notEqual(result.baselineDigests.desktop, result.candidateDigests.desktop);
});
test('unchanged or resource-only candidate cannot be relabeled native component upgrade', t => {
  const input = fixture(t);
  for (const resource of [false, true]) {
    if (resource) fs.writeFileSync(path.join(input.candidate, 'resource.txt'), 'new resource');
    assert.throws(() => selectionProofInputs([...input.args, '--candidate-source', input.candidate]), { code: 'UNCHANGED_NATIVE_CANDIDATE' });
  }
});
test('invalid, duplicate or incomplete flags refuse before file access', () => {
  for (const args of [[], ['--unknown', 'x'], ['--source', 'x', '--source', 'y'], ['--candidate-source']]) assert.throws(() => selectionProofInputs(args), { code: 'INVALID_SELECTION_PROOF_ARGS' });
});
test('relative candidate and overlapping output refuse without mutation', t => {
  const input = fixture(t);
  assert.throws(() => selectionProofInputs([...input.args, '--candidate-source', 'relative']), { code: 'INVALID_SELECTION_PROOF_ARGS' });
  const args = [...input.args]; args[3] = path.join(input.candidate, 'proof');
  assert.throws(() => selectionProofInputs([...args, '--candidate-source', input.candidate]), { code: 'OVERLAPPING_NATIVE_ROOTS' });
  assert.equal(fs.existsSync(args[3]), false);
});
test('candidate must contain complete actual native roles', t => {
  const input = fixture(t); fs.unlinkSync(path.join(input.candidate, 'daemon'));
  assert.throws(() => selectionProofInputs([...input.args, '--candidate-source', input.candidate]), { code: 'INVALID_NATIVE_MANIFEST' });
});
