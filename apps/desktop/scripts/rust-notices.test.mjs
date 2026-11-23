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
  const embedded = name => report.packages.find(pkg => pkg.name === name).documents.map(doc => doc.name);
  assert.ok(embedded("libsqlite3-sys").includes("sqlite/3.54.1/source-notice.txt"));
  if (host.includes("windows")) {
    const sdk = embedded("webview2-com-sys");
    for (const name of ["LICENSE.txt", "NOTICE.txt"]) {
      assert.ok(sdk.includes("microsoft-webview2/1.0.3800.47/" + name));
    }
  }
  for (const pkg of report.packages) {
    assert.ok(pkg.documents.length, pkg.name);
    for (const doc of pkg.documents) {
      const value = report.documents[doc.sha256];
      const bytes = value.text === undefined ? Buffer.from(value.base64, "base64") : Buffer.from(value.text);
      assert.equal(createHash("sha256").update(bytes).digest("hex"), doc.sha256);
    }
  }
});

test("the standalone updater ships its own locked notice set", () => {
  const root = fileURLToPath(new URL("../../../", import.meta.url));
  const host = hostTripleFromRustc(execFileSync("rustc", ["-vV"], { encoding: "utf8" }));
  const report = collectRustNotices(root, host, host, ["cryptex-updater"]);
  const names = new Set(report.packages.map(pkg => pkg.name));
  for (const name of ["process-wrap", "sha2", "cc"]) assert.ok(names.has(name), name);
  assert.ok(!names.has("tauri"));
  for (const pkg of report.packages) {
    assert.ok(pkg.documents.length, pkg.name);
    for (const doc of pkg.documents) {
      const data = report.documents[doc.sha256];
      const bytes = data.text === undefined ? Buffer.from(data.base64, "base64") : Buffer.from(data.text);
      assert.equal(createHash("sha256").update(bytes).digest("hex"), doc.sha256);
    }
  }
});
