import assert from "node:assert/strict";
import test from "node:test";
import path from "node:path";
import {
  hostTripleFromRustc,
  sidecarBuildPaths,
  sidecarFileName,
} from "./prepare-cryptexd-sidecar.mjs";

test("reads the host target from rustc -vV", () => {
  assert.equal(
    hostTripleFromRustc("rustc 1.90.0 (abc 2025-09-18)\nbinary: rustc\nhost: x86_64-pc-windows-msvc\n"),
    "x86_64-pc-windows-msvc",
  );
});

test("uses Tauri's target-suffixed sidecar filename conventions", () => {
  assert.equal(sidecarFileName("x86_64-pc-windows-msvc"), "cryptexd-x86_64-pc-windows-msvc.exe");
  assert.equal(sidecarFileName("aarch64-apple-darwin"), "cryptexd-aarch64-apple-darwin");
  assert.throws(() => sidecarFileName("../windows"), /invalid Rust target triple/);
});

test("stages the built daemon under the Tauri source tree", () => {
  assert.deepEqual(
    sidecarBuildPaths("D:/work/cryptex", "x86_64-pc-windows-msvc"),
    {
      builtBinary: path.join(
        "D:/work/cryptex",
        "target",
        "x86_64-pc-windows-msvc",
        "release",
        "cryptexd.exe",
      ),
      stagedBinary: path.join(
        "D:/work/cryptex",
        "apps",
        "desktop",
        "src-tauri",
        "binaries",
        "cryptexd-x86_64-pc-windows-msvc.exe",
      ),
    },
  );
});
