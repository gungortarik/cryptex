import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import os from "node:os";
import path from "node:path";
import test from "node:test";
import { sourceRevision } from "./build-provenance.mjs";

test("only a clean checkout of the actual package source identifies a reviewed revision", t => {
  const root = mkdtempSync(path.join(os.tmpdir(), "cryptex-provenance-"));
  t.after(() => rmSync(root, { recursive: true, force: true }));
  const git = (...args) => execFileSync("git", args, { cwd: root, encoding: "utf8" }).trim();
  assert.deepEqual(sourceRevision(root), { commit: null, clean: false });
  git("init", "-q");
  assert.deepEqual(sourceRevision(root), { commit: null, clean: false });
  writeFileSync(path.join(root, "source"), "original\n");
  git("add", "source");
  git("-c", "user.name=Test", "-c", "user.email=test@example.invalid",
    "-c", "commit.gpgsign=false", "commit", "-qm", "fixture");
  const commit = git("rev-parse", "HEAD");
  assert.deepEqual(sourceRevision(root), { commit, clean: true });
  const nested = path.join(root, "unpacked");
  mkdirSync(nested);
  assert.deepEqual(sourceRevision(nested), { commit: null, clean: false });
  writeFileSync(path.join(root, "extra"), "untracked\n");
  assert.deepEqual(sourceRevision(root), { commit, clean: false });
  rmSync(path.join(root, "extra"));
  writeFileSync(path.join(root, "source"), "modified\n");
  assert.deepEqual(sourceRevision(root), { commit, clean: false });
});
