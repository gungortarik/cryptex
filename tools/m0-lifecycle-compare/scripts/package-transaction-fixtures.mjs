// Test fixture allocation only. Do not canonicalize operator package paths here.
import * as fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';

export function createFixtureRoot(temporaryDirectory = os.tmpdir()) {
  // macOS /var and /tmp are OS aliases. Allocate on the real temporary directory,
  // while the package engine continues rejecting links in all supplied paths.
  const realTemporaryDirectory = fs.realpathSync(temporaryDirectory);
  return fs.mkdtempSync(path.join(realTemporaryDirectory, 'cryptex-m0-pair-'));
}
