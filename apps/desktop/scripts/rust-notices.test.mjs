import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import { fileURLToPath } from "node:url";
import test from "node:test";
import { hostTripleFromRustc } from "./prepare-cryptexd-sidecar.mjs";
import { collectRustNotices } from "./rust-notices.mjs";

test("release inventory covers the native desktop and daemon and retains exact notice bytes", () => {
  const root = fileURLToPath(new URL("../../../", import.meta.url));
  const host = hostTripleFromRustc(execFileSync("rustc", ["-vV"], { encoding: "utf8" }));
  const report = collectRustNotices(root, host);
  assert.equal(report.target, host);
  const names = new Set(report.packages.map(pkg => pkg.name));
  for (const name of ["tauri", "rusqlite", "libsqlite3-sys", "ed25519-dalek"]) assert.ok(names.has(name), name);
  assert.ok(!names.has("cryptexd"));
  const keys = report.packages.map(pkg => pkg.name + " v" + pkg.version);
  assert.deepEqual(keys, [...new Set(keys)].sort());
  assert.ok(report.packages.find(pkg => pkg.name === "tauri").documents.length > 0);
  for (const pkg of report.packages) {
    assert.ok(pkg.documents.length, pkg.name);
    for (const doc of pkg.documents) {
      const value = report.documents[doc.sha256];
      const bytes = value.text === undefined ? Buffer.from(value.base64, "base64") : Buffer.from(value.text);
      assert.equal(createHash("sha256").update(bytes).digest("hex"), doc.sha256);
    }
  }
});
