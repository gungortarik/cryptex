#!/usr/bin/env node

/**
 * Capture raw timed-runner stdout into a Cryptex-Dev runtime evidence file.
 * Refuses repository destinations and non-run roots.
 *
 * `--raw-file` decoding:
 * - BOM `FF FE` => UTF-16LE (PowerShell redirected stdout)
 * - otherwise => UTF-8 (existing behavior)
 */

import fs from "node:fs";
import path from "node:path";
import { pathToFileURL } from "node:url";

function fail(message) {
  throw new Error(message);
}

function parseArgs(argv) {
  const values = new Map();
  for (let i = 0; i < argv.length; i += 1) {
    const arg = argv[i];
    if (!arg.startsWith("--")) fail(`unexpected argument: ${arg}`);
    if (i + 1 >= argv.length) fail(`${arg} requires a value`);
    values.set(arg, argv[i + 1]);
    i += 1;
  }
  const folderId = values.get("--folder-id");
  const rawText = values.get("--raw-text");
  const rawFile = values.get("--raw-file");
  if (!folderId || (!rawText && !rawFile) || (rawText && rawFile)) {
    fail(
      "usage: capture-syncthing-p0-1-raw-result.mjs --folder-id <id> (--raw-text <text> | --raw-file <path>)",
    );
  }
  if (!/^[a-z0-9-]+$/.test(folderId) || !folderId.startsWith("p0-1-")) {
    fail("folder-id must be a p0-1-* id");
  }
  return { folderId, rawText, rawFile };
}

function normalizeForGuard(value) {
  return path.resolve(value).replaceAll("\\", "/");
}

function assertRunRoot(destinationDir) {
  const normalized = normalizeForGuard(destinationDir);
  if (!normalized.includes("/Cryptex-Dev/syncthing-p0-1-runs/")) {
    fail(`destination must be under syncthing-p0-1-runs: ${destinationDir}`);
  }
  if (normalized.includes("/github_repos/") || normalized.includes("/cryptex/evidence/")) {
    fail(`refusing repository destination: ${destinationDir}`);
  }
}

function platformRunsRoot() {
  if (process.platform === "darwin") {
    return path.join(
      process.env.HOME ?? fail("HOME is required"),
      "Library",
      "Application Support",
      "Cryptex-Dev",
      "syncthing-p0-1-runs",
    );
  }
  if (process.platform === "win32") {
    return path.join(
      process.env.LOCALAPPDATA ?? fail("LOCALAPPDATA is required"),
      "Cryptex-Dev",
      "syncthing-p0-1-runs",
    );
  }
  fail(`unsupported platform: ${process.platform}`);
}

/** Decode a --raw-file buffer using explicit BOM detection only. */
export function decodeRawFileBuffer(buffer) {
  if (
    Buffer.isBuffer(buffer) &&
    buffer.length >= 2 &&
    buffer[0] === 0xff &&
    buffer[1] === 0xfe
  ) {
    return buffer.subarray(2).toString("utf16le");
  }
  return Buffer.from(buffer).toString("utf8");
}

/** Shared marker / folder-id guards for timed initial-sync captures. */
export function validateTimedRawBody(body, folderId) {
  if (!body.includes("scope=isolated_p0_1_initial_syncthing_scan_sync_measurement")) {
    fail("raw result does not look like a timed initial-sync measurement output");
  }
  if (!body.includes(`folder_id=${folderId}`)) {
    fail("raw result folder_id does not match requested folder-id");
  }
}

function readRawBody(args) {
  if (args.rawFile) {
    const buffer = fs.readFileSync(path.resolve(args.rawFile));
    return decodeRawFileBuffer(buffer);
  }
  return args.rawText;
}

function main() {
  const args = parseArgs(process.argv.slice(2));
  const dir = path.join(platformRunsRoot(), args.folderId);
  assertRunRoot(dir);
  fs.mkdirSync(dir, { recursive: true });
  const stamp = new Date().toISOString().replaceAll(":", "").replaceAll(".", "");
  const outPath = path.join(dir, `timed-result-${stamp}.txt`);
  if (fs.existsSync(outPath)) fail(`refusing to overwrite existing capture: ${outPath}`);
  const body = readRawBody(args);
  validateTimedRawBody(body, args.folderId);
  fs.writeFileSync(outPath, body.endsWith("\n") ? body : `${body}\n`, { flag: "wx" });
  console.log(`folder_id=${args.folderId}`);
  console.log(`raw_capture=${outPath}`);
  console.log("scope=isolated_p0_1_raw_result_capture_only");
}

function isDirectRun() {
  const entry = process.argv[1];
  if (!entry) return false;
  return pathToFileURL(path.resolve(entry)).href === import.meta.url;
}

if (isDirectRun()) {
  try {
    main();
  } catch (error) {
    console.error(`error: ${error instanceof Error ? error.message : String(error)}`);
    process.exitCode = 2;
  }
}
