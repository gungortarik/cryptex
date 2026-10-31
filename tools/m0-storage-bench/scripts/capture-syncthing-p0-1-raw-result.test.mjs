#!/usr/bin/env node

/**
 * Temp-file validation for capture-syncthing-p0-1-raw-result encoding/guards.
 * Does not touch Cryptex-Dev/syncthing-p0-1-runs or retained benchmark corpora.
 */

import assert from "node:assert/strict";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import test from "node:test";

import {
  decodeRawFileBuffer,
  validateTimedRawBody,
} from "./capture-syncthing-p0-1-raw-result.mjs";

const folderId = "p0-1-encoding-fixture-cell";

function sampleBody(id = folderId) {
  return [
    `folder_id=${id}`,
    "layout=files",
    "events=100000",
    "scope=isolated_p0_1_initial_syncthing_scan_sync_measurement",
    "",
  ].join("\n");
}

function withTempDir(run) {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), "cryptex-p0-1-raw-capture-"));
  try {
    run(dir);
  } finally {
    fs.rmSync(dir, { recursive: true, force: true });
  }
}

test("valid UTF-8 timed output is accepted", () => {
  withTempDir((dir) => {
    const file = path.join(dir, "utf8.txt");
    const body = sampleBody();
    fs.writeFileSync(file, body, "utf8");
    const decoded = decodeRawFileBuffer(fs.readFileSync(file));
    assert.equal(decoded, body);
    assert.doesNotThrow(() => validateTimedRawBody(decoded, folderId));
  });
});

test("valid BOM UTF-16LE timed output is accepted", () => {
  withTempDir((dir) => {
    const file = path.join(dir, "utf16le.txt");
    const body = sampleBody();
    const payload = Buffer.concat([
      Buffer.from([0xff, 0xfe]),
      Buffer.from(body, "utf16le"),
    ]);
    fs.writeFileSync(file, payload);
    const decoded = decodeRawFileBuffer(fs.readFileSync(file));
    assert.equal(decoded, body);
    assert.doesNotThrow(() => validateTimedRawBody(decoded, folderId));
  });
});

test("missing required scope marker is rejected", () => {
  const body = `folder_id=${folderId}\nlayout=files\n`;
  assert.throws(
    () => validateTimedRawBody(body, folderId),
    /does not look like a timed initial-sync measurement output/,
  );
});

test("mismatched folder_id is rejected", () => {
  const body = sampleBody("p0-1-other-cell");
  assert.throws(
    () => validateTimedRawBody(body, folderId),
    /folder_id does not match requested folder-id/,
  );
});

test("UTF-8 without BOM remains unchanged (no silent re-encoding)", () => {
  const body = sampleBody();
  const decoded = decodeRawFileBuffer(Buffer.from(body, "utf8"));
  assert.equal(decoded, body);
});
