#!/usr/bin/env node

/**
 * Timed P0-1 incremental Syncthing sync runner (source machine).
 *
 * Follows evidence/m0/p0-1/syncthing-incremental-workload-APPROVED-FOR-MEASUREMENT.md:
 * append while paused (outside timing), then unpause and measure sync.
 */

import { spawnSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { performance } from "node:perf_hooks";

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

  const required = [
    "--home",
    "--peer-id",
    "--folder-id",
    "--layout",
    "--baseline-events",
    "--incremental-events",
    "--payload-bytes",
    "--source-platform",
    "--destination-platform",
    "--bench-bin",
  ];
  for (const key of required) {
    if (!values.get(key)) fail(`missing required argument: ${key}`);
  }

  const baselineEvents = Number(values.get("--baseline-events"));
  const incrementalEvents = Number(values.get("--incremental-events"));
  const payloadBytes = Number(values.get("--payload-bytes"));
  const timeoutSeconds = Number(values.get("--timeout-seconds") ?? "7200");
  const segmentEventsRaw = values.get("--segment-events");

  if (![10000, 100000, 500000].includes(baselineEvents)) {
    fail("baseline-events must be 10000, 100000, or 500000");
  }
  if (incrementalEvents !== 100) fail("incremental-events must be 100 for the approved workload");
  if (payloadBytes !== 1024) fail("payload-bytes must be 1024 for the approved workload");
  if (!Number.isFinite(timeoutSeconds) || timeoutSeconds <= 0) fail("timeout-seconds must be positive");

  const layout = values.get("--layout");
  if (layout !== "files" && layout !== "segments") fail("layout must be files or segments");

  let segmentEvents = null;
  if (layout === "segments") {
    segmentEvents = Number(segmentEventsRaw);
    if (segmentEvents !== 1000) fail("segment-events must be 1000 for the approved workload");
  } else if (segmentEventsRaw) {
    fail("files layout must not receive --segment-events");
  }

  const sourcePlatform = values.get("--source-platform");
  const destinationPlatform = values.get("--destination-platform");
  for (const platform of [sourcePlatform, destinationPlatform]) {
    if (platform !== "macos" && platform !== "windows") fail("platform values must be macos or windows");
  }
  if (sourcePlatform === destinationPlatform) {
    fail("source and destination platforms must differ for this cross-platform run");
  }

  return {
    home: path.resolve(values.get("--home")),
    peerId: values.get("--peer-id").trim(),
    folderId: values.get("--folder-id"),
    layout,
    baselineEvents,
    incrementalEvents,
    payloadBytes,
    segmentEvents,
    sourcePlatform,
    destinationPlatform,
    benchBin: path.resolve(values.get("--bench-bin")),
    timeoutMs: timeoutSeconds * 1000,
  };
}

function normalizeForGuard(value) {
  return path.resolve(value).replaceAll("\\", "/");
}

function assertHomeGuard(home) {
  const normalized = normalizeForGuard(home);
  if (!normalized.includes("/Cryptex-Dev/") || !normalized.endsWith("/syncthing-p0-1")) {
    fail(`refusing non-Cryptex P0-1 Syncthing home: ${home}`);
  }
}

async function requestJson(url, apiKey, options = {}) {
  const response = await fetch(url, {
    ...options,
    headers: {
      "X-API-Key": apiKey,
      ...(options.body ? { "Content-Type": "application/json" } : {}),
      ...(options.headers ?? {}),
    },
  });
  const text = await response.text();
  if (!response.ok) {
    fail(`${options.method ?? "GET"} ${url} failed: HTTP ${response.status} ${text}`);
  }
  if (!text.trim()) return null;
  try {
    return JSON.parse(text);
  } catch {
    fail(`non-JSON response from ${url}: ${text}`);
  }
}

async function tryRequestJson(url, apiKey) {
  try {
    return await requestJson(url, apiKey);
  } catch {
    return null;
  }
}

function platformMatches(expected) {
  if (expected === "macos") return process.platform === "darwin";
  if (expected === "windows") return process.platform === "win32";
  return false;
}

function numeric(value) {
  const n = Number(value ?? 0);
  return Number.isFinite(n) ? n : 0;
}

function parseKv(text) {
  const out = {};
  for (const line of text.split(/\r?\n/)) {
    const idx = line.indexOf("=");
    if (idx <= 0) continue;
    out[line.slice(0, idx)] = line.slice(idx + 1);
  }
  return out;
}

function runBench(args) {
  const result = spawnSync(args[0], args.slice(1), {
    encoding: "utf8",
    env: process.env,
  });
  if (result.error) fail(`bench spawn failed: ${result.error.message}`);
  if (result.status !== 0) {
    fail(`bench exited ${result.status}: ${(result.stderr || result.stdout || "").trim()}`);
  }
  return parseKv(result.stdout || "");
}

async function main() {
  const args = parseArgs(process.argv.slice(2));
  assertHomeGuard(args.home);
  if (!platformMatches(args.sourcePlatform)) {
    fail(`source-platform ${args.sourcePlatform} does not match current runtime platform ${process.platform}`);
  }
  if (!/^[A-Z0-9-]+$/.test(args.peerId)) fail("peer device ID format is invalid");
  if (!/^[a-z0-9-]+$/.test(args.folderId)) fail("folder-id format is invalid");
  if (!fs.existsSync(args.benchBin)) fail(`bench binary not found: ${args.benchBin}`);

  const retired = new Set([
    "p0-1-10k-files-macos-to-windows",
    "p0-1-10k-files-macos-to-windows-r2",
  ]);
  if (retired.has(args.folderId)) {
    fail(`refusing incremental run on invalid/consumed folder id: ${args.folderId}`);
  }

  const configPath = path.join(args.home, "config.xml");
  if (!fs.existsSync(configPath)) fail(`config.xml not found: ${configPath}`);
  const xml = fs.readFileSync(configPath, "utf8");
  const guiMatch = xml.match(
    /<gui\b[^>]*>[\s\S]*?<address>([^<]+)<\/address>[\s\S]*?<apikey>([^<]+)<\/apikey>[\s\S]*?<\/gui>/,
  );
  if (!guiMatch) fail("unable to read isolated GUI address/API key from config.xml");
  const guiAddress = guiMatch[1].trim();
  const apiKey = guiMatch[2].trim();
  if (guiAddress !== "127.0.0.1:18384") fail(`unexpected isolated GUI address: ${guiAddress}`);

  const baseUrl = `http://${guiAddress}`;
  const systemStatus = await requestJson(`${baseUrl}/rest/system/status`, apiKey);
  if (!systemStatus?.myID) fail("system status did not return local device ID");
  const version = await requestJson(`${baseUrl}/rest/system/version`, apiKey);

  const folders = await requestJson(`${baseUrl}/rest/config/folders`, apiKey);
  if (!Array.isArray(folders) || folders.length !== 1) {
    fail(`expected exactly one benchmark folder, observed ${Array.isArray(folders) ? folders.length : "invalid"}`);
  }
  const folder = folders[0];
  if (folder.id !== args.folderId) fail(`unexpected folder configured: ${folder.id ?? "missing"}`);
  if (folder.type !== "sendonly") fail(`incremental runner requires sendonly source, observed ${folder.type}`);
  if (folder.paused !== true) fail("source folder must be paused before incremental append/timing");
  if (folder.fsWatcherEnabled !== false) fail("fsWatcherEnabled must be false");
  const normalizedFolderPath = normalizeForGuard(folder.path);
  if (!normalizedFolderPath.includes("/Cryptex-Dev/syncthing-p0-1-runs/")) {
    fail(`source folder path outside guarded run root: ${folder.path}`);
  }
  if (normalizedFolderPath.includes("/Cryptex-Dev/benchmarks/")) {
    fail(`refusing to append into retained corpus path: ${folder.path}`);
  }

  const devices = await requestJson(`${baseUrl}/rest/config/devices`, apiKey);
  const remotes = (devices ?? []).filter((device) => device.deviceID !== systemStatus.myID);
  if (remotes.length !== 1 || remotes[0].deviceID !== args.peerId) {
    fail(`unexpected remote device set: ${remotes.map((d) => d.deviceID).join(",") || "none"}`);
  }
  const connections = await requestJson(`${baseUrl}/rest/system/connections`, apiKey);
  const runtime = connections?.connections?.[args.peerId];
  if (!runtime?.connected || typeof runtime.type !== "string" || !/^tcp-/i.test(runtime.type)) {
    fail("intended benchmark peer is not connected over TCP before incremental run");
  }

  const preScan = runBench([args.benchBin, "scan", args.layout, folder.path]);
  if (Number(preScan.events) !== args.baselineEvents) {
    fail(
      `baseline event count mismatch before append: expected ${args.baselineEvents}, observed ${preScan.events}`,
    );
  }

  const appendStarted = performance.now();
  const appendArgs = [
    args.benchBin,
    "append",
    args.layout,
    String(args.incrementalEvents),
    String(args.payloadBytes),
    folder.path,
  ];
  if (args.layout === "segments") appendArgs.push(String(args.segmentEvents));
  const appendOut = runBench(appendArgs);
  const appendMs = performance.now() - appendStarted;

  const postScan = runBench([args.benchBin, "scan", args.layout, folder.path]);
  const expectedEvents = args.baselineEvents + args.incrementalEvents;
  if (Number(postScan.events) !== expectedEvents) {
    fail(`post-append events mismatch: expected ${expectedEvents}, observed ${postScan.events}`);
  }
  const expectedFiles = Number(postScan.physical_objects);
  const expectedBytes = Number(postScan.stored_bytes);
  if (!Number.isSafeInteger(expectedFiles) || expectedFiles <= 0) fail("invalid post-append physical_objects");
  if (!Number.isSafeInteger(expectedBytes) || expectedBytes <= 0) fail("invalid post-append stored_bytes");
  const incrementalLogicalBytes = expectedBytes - Number(preScan.stored_bytes);
  if (incrementalLogicalBytes !== args.incrementalEvents * (args.payloadBytes + 12)) {
    fail(
      `incremental logical bytes mismatch: expected ${args.incrementalEvents * (args.payloadBytes + 12)}, observed ${incrementalLogicalBytes}`,
    );
  }

  const startedAtUtc = new Date().toISOString();
  const started = performance.now();

  await requestJson(`${baseUrl}/rest/config/folders/${encodeURIComponent(args.folderId)}`, apiKey, {
    method: "PATCH",
    body: JSON.stringify({ paused: false }),
  });

  let scanCompletedAt = null;
  let scanStatus = null;
  let completion = null;
  let finalStatus = null;
  const deadline = started + args.timeoutMs;

  while (performance.now() < deadline) {
    const status = await tryRequestJson(
      `${baseUrl}/rest/db/status?folder=${encodeURIComponent(args.folderId)}`,
      apiKey,
    );
    if (status) {
      const localFiles = numeric(status.localFiles);
      const localBytes = numeric(status.localBytes);
      const state = String(status.state ?? "unknown");
      if (
        scanCompletedAt === null &&
        localFiles === expectedFiles &&
        localBytes === expectedBytes &&
        state !== "scanning"
      ) {
        scanCompletedAt = performance.now();
        scanStatus = status;
      }
    }

    if (scanCompletedAt !== null) {
      const peerCompletion = await tryRequestJson(
        `${baseUrl}/rest/db/completion?folder=${encodeURIComponent(args.folderId)}&device=${encodeURIComponent(args.peerId)}`,
        apiKey,
      );
      if (peerCompletion) {
        if (
          numeric(peerCompletion.completion) >= 100 &&
          numeric(peerCompletion.needBytes) === 0 &&
          numeric(peerCompletion.needItems) === 0 &&
          numeric(peerCompletion.needDeletes) === 0 &&
          numeric(status?.globalFiles) === expectedFiles
        ) {
          completion = peerCompletion;
          finalStatus = status;
          break;
        }
      }
    }

    await new Promise((resolve) => setTimeout(resolve, 250));
  }

  if (scanCompletedAt === null) {
    fail(`source incremental scan did not reach expected files/bytes within ${Math.round(args.timeoutMs / 1000)} seconds`);
  }
  if (!completion || !finalStatus) {
    fail(`peer incremental synchronization did not complete within ${Math.round(args.timeoutMs / 1000)} seconds`);
  }

  const completed = performance.now();
  const completedAtUtc = new Date().toISOString();

  await requestJson(`${baseUrl}/rest/config/folders/${encodeURIComponent(args.folderId)}`, apiKey, {
    method: "PATCH",
    body: JSON.stringify({ paused: true }),
  });
  const pausedVerified = await requestJson(
    `${baseUrl}/rest/config/folders/${encodeURIComponent(args.folderId)}`,
    apiKey,
  );
  if (pausedVerified?.paused !== true) fail("source folder did not return to paused state after incremental measurement");

  console.log(`home=${args.home}`);
  console.log(`local_device_id=${systemStatus.myID}`);
  console.log(`peer_id=${args.peerId}`);
  console.log(`folder_id=${args.folderId}`);
  console.log(`folder_path=${folder.path}`);
  console.log(`layout=${args.layout}`);
  console.log(`baseline_events=${args.baselineEvents}`);
  console.log(`incremental_events=${args.incrementalEvents}`);
  console.log(`payload_bytes=${args.payloadBytes}`);
  if (args.segmentEvents != null) console.log(`segment_events=${args.segmentEvents}`);
  console.log(`source_platform=${args.sourcePlatform}`);
  console.log(`destination_platform=${args.destinationPlatform}`);
  console.log(`syncthing_version=${version?.version ?? "unknown"}`);
  console.log(`append_setup_ms=${appendMs.toFixed(3)}`);
  console.log(`append_command_events=${appendOut.events ?? "unknown"}`);
  console.log(`expected_files_after_append=${expectedFiles}`);
  console.log(`expected_bytes_after_append=${expectedBytes}`);
  console.log(`incremental_logical_bytes=${incrementalLogicalBytes}`);
  console.log(`started_at_utc=${startedAtUtc}`);
  console.log(
    `scan_completed_at_utc=${new Date(Date.parse(startedAtUtc) + (scanCompletedAt - started)).toISOString()}`,
  );
  console.log(`completed_at_utc=${completedAtUtc}`);
  console.log(`incremental_scan_ms=${(scanCompletedAt - started).toFixed(3)}`);
  console.log(`incremental_sync_total_ms=${(completed - started).toFixed(3)}`);
  console.log(`source_local_files=${numeric(scanStatus?.localFiles)}`);
  console.log(`source_local_bytes=${numeric(scanStatus?.localBytes)}`);
  console.log(`source_global_files=${numeric(finalStatus?.globalFiles)}`);
  console.log(`source_global_bytes=${numeric(finalStatus?.globalBytes)}`);
  console.log(`peer_completion=${numeric(completion.completion)}`);
  console.log(`peer_global_items=${numeric(completion.globalItems)}`);
  console.log(`peer_need_items=${numeric(completion.needItems)}`);
  console.log(`peer_need_bytes=${numeric(completion.needBytes)}`);
  console.log(`peer_need_deletes=${numeric(completion.needDeletes)}`);
  console.log(`source_global_files_verified=${numeric(finalStatus.globalFiles)}`);
  console.log(`source_paused_after_completion=${Boolean(pausedVerified.paused)}`);
  console.log("transport_scope=explicit_direct_tcp_only");
  console.log("scope=isolated_p0_1_incremental_syncthing_scan_sync_measurement");
}

main().catch((error) => {
  console.error(`error: ${error instanceof Error ? error.message : String(error)}`);
  process.exitCode = 2;
});
