#!/usr/bin/env node

import fs from "node:fs";
import fsp from "node:fs/promises";
import path from "node:path";

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

  const home = values.get("--home");
  const peerId = values.get("--peer-id");
  const folderId = values.get("--folder-id");
  const folderPath = values.get("--folder-path");
  const role = values.get("--role");
  const sourceCorpus = values.get("--source-corpus");
  const expectedFilesRaw = values.get("--expected-files");
  const expectedBytesRaw = values.get("--expected-bytes");

  if (!home || !peerId || !folderId || !folderPath || !role) {
    fail(
      "usage: prepare-syncthing-p0-1-initial-sync.mjs --home <isolated-home> --peer-id <device-id> --folder-id <id> --folder-path <runtime-path> --role <source|destination> [--source-corpus <retained-corpus> --expected-files <n> --expected-bytes <n>]",
    );
  }
  if (!/^[a-z0-9-]+$/.test(folderId)) fail("folder-id must contain only lowercase letters, digits, and hyphens");
  if (role !== "source" && role !== "destination") fail("role must be source or destination");
  if (!/^[A-Z0-9-]+$/.test(peerId)) fail("peer device ID format is invalid");

  let expectedFiles = null;
  let expectedBytes = null;
  if (role === "source") {
    if (!sourceCorpus || !expectedFilesRaw || !expectedBytesRaw) {
      fail("source role requires --source-corpus, --expected-files, and --expected-bytes");
    }
    expectedFiles = Number(expectedFilesRaw);
    expectedBytes = Number(expectedBytesRaw);
    if (!Number.isSafeInteger(expectedFiles) || expectedFiles <= 0) fail("expected-files must be a positive integer");
    if (!Number.isSafeInteger(expectedBytes) || expectedBytes <= 0) fail("expected-bytes must be a positive integer");
  } else if (sourceCorpus || expectedFilesRaw || expectedBytesRaw) {
    fail("destination role must not receive source corpus or expected source counts");
  }

  return {
    home: path.resolve(home),
    peerId: peerId.trim(),
    folderId,
    folderPath: path.resolve(folderPath),
    role,
    sourceCorpus: sourceCorpus ? path.resolve(sourceCorpus) : null,
    expectedFiles,
    expectedBytes,
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

function assertRunPathGuard(folderPath) {
  const normalized = normalizeForGuard(folderPath);
  if (!normalized.includes("/Cryptex-Dev/syncthing-p0-1-runs/")) {
    fail(`folder path must be under the isolated Cryptex-Dev syncthing-p0-1-runs root: ${folderPath}`);
  }
}

function assertCorpusGuard(sourceCorpus) {
  const normalized = normalizeForGuard(sourceCorpus);
  if (!normalized.includes("/Cryptex-Dev/benchmarks/p0-1/")) {
    fail(`source corpus must be a retained P0-1 benchmark dataset: ${sourceCorpus}`);
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

async function readTreeStats(root) {
  let files = 0;
  let bytes = 0;
  const stack = [root];
  while (stack.length > 0) {
    const current = stack.pop();
    const entries = await fsp.readdir(current, { withFileTypes: true });
    for (const entry of entries) {
      if (entry.name === ".stfolder") continue;
      const full = path.join(current, entry.name);
      if (entry.isDirectory()) {
        stack.push(full);
      } else if (entry.isFile()) {
        const stat = await fsp.stat(full);
        files += 1;
        bytes += stat.size;
      } else {
        fail(`unsupported non-file/non-directory entry in benchmark tree: ${full}`);
      }
    }
  }
  return { files, bytes };
}

async function waitForDestinationIdle(baseUrl, apiKey, folderId) {
  const deadline = Date.now() + 30_000;
  while (Date.now() < deadline) {
    try {
      const status = await requestJson(`${baseUrl}/rest/db/status?folder=${encodeURIComponent(folderId)}`, apiKey);
      if (status?.state === "idle" && Number(status.localFiles ?? 0) === 0 && Number(status.localBytes ?? 0) === 0) {
        return;
      }
    } catch {
      // Folder database can take a moment to appear after config mutation.
    }
    await new Promise((resolve) => setTimeout(resolve, 250));
  }
  fail("destination folder did not become idle and empty within 30 seconds");
}

async function main() {
  const args = parseArgs(process.argv.slice(2));
  assertHomeGuard(args.home);
  assertRunPathGuard(args.folderPath);
  if (args.sourceCorpus) assertCorpusGuard(args.sourceCorpus);

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
  if (!apiKey) fail("isolated Syncthing API key is empty");

  const baseUrl = `http://${guiAddress}`;
  const systemStatus = await requestJson(`${baseUrl}/rest/system/status`, apiKey);
  if (!systemStatus?.myID) fail("system status did not return local device ID");

  const folders = await requestJson(`${baseUrl}/rest/config/folders`, apiKey);
  if (!Array.isArray(folders)) fail("unexpected folders response");
  if (folders.length !== 0) fail(`benchmark folders already configured (${folders.length}); refusing setup`);

  const devices = await requestJson(`${baseUrl}/rest/config/devices`, apiKey);
  if (!Array.isArray(devices)) fail("unexpected devices response");
  const remotes = devices.filter((device) => device.deviceID !== systemStatus.myID);
  if (remotes.length !== 1 || remotes[0].deviceID !== args.peerId) {
    fail(`unexpected remote device set: ${remotes.map((device) => device.deviceID).join(",") || "none"}`);
  }
  const peer = remotes[0];
  if (peer.paused === true || peer.autoAcceptFolders !== false || peer.introducer !== false) {
    fail("peer configuration is not in the guarded P0-1 state");
  }

  const connections = await requestJson(`${baseUrl}/rest/system/connections`, apiKey);
  const runtime = connections?.connections?.[args.peerId];
  if (!runtime?.connected || typeof runtime.type !== "string" || !/^tcp-/i.test(runtime.type)) {
    fail("intended benchmark peer is not connected over TCP");
  }

  if (fs.existsSync(args.folderPath)) {
    fail(`folder path already exists; refusing automatic reuse or deletion: ${args.folderPath}`);
  }

  await fsp.mkdir(path.dirname(args.folderPath), { recursive: true });

  let preConfigStats;
  if (args.role === "source") {
    if (!fs.existsSync(args.sourceCorpus)) fail(`source corpus does not exist: ${args.sourceCorpus}`);
    const corpusStats = await readTreeStats(args.sourceCorpus);
    if (corpusStats.files !== args.expectedFiles || corpusStats.bytes !== args.expectedBytes) {
      fail(
        `source corpus stats mismatch: expected files=${args.expectedFiles} bytes=${args.expectedBytes}, observed files=${corpusStats.files} bytes=${corpusStats.bytes}`,
      );
    }
    await fsp.cp(args.sourceCorpus, args.folderPath, {
      recursive: true,
      force: false,
      errorOnExist: true,
      preserveTimestamps: true,
    });
    preConfigStats = await readTreeStats(args.folderPath);
    if (preConfigStats.files !== args.expectedFiles || preConfigStats.bytes !== args.expectedBytes) {
      fail(
        `working copy stats mismatch: expected files=${args.expectedFiles} bytes=${args.expectedBytes}, observed files=${preConfigStats.files} bytes=${preConfigStats.bytes}`,
      );
    }
  } else {
    await fsp.mkdir(args.folderPath, { recursive: false });
    preConfigStats = await readTreeStats(args.folderPath);
    if (preConfigStats.files !== 0 || preConfigStats.bytes !== 0) {
      fail("destination working folder was not empty after creation");
    }
  }

  const template = await requestJson(`${baseUrl}/rest/config/defaults/folder`, apiKey);
  if (!template || typeof template !== "object") fail("unexpected default folder template response");

  template.id = args.folderId;
  template.label = args.folderId;
  template.path = args.folderPath;
  template.type = args.role === "source" ? "sendonly" : "receiveonly";
  template.paused = args.role === "source";
  template.fsWatcherEnabled = false;
  template.rescanIntervalS = 3600;
  template.devices = [
    { deviceID: systemStatus.myID, encryptionPassword: "" },
    { deviceID: args.peerId, encryptionPassword: "" },
  ];

  await requestJson(`${baseUrl}/rest/config/folders`, apiKey, {
    method: "POST",
    body: JSON.stringify(template),
  });

  const verified = await requestJson(
    `${baseUrl}/rest/config/folders/${encodeURIComponent(args.folderId)}`,
    apiKey,
  );
  if (!verified || verified.id !== args.folderId) fail("folder verification returned the wrong folder ID");
  if (path.resolve(verified.path) !== args.folderPath) fail(`folder verification returned unexpected path: ${verified.path}`);
  const expectedType = args.role === "source" ? "sendonly" : "receiveonly";
  if (verified.type !== expectedType) fail(`folder verification returned unexpected type: ${verified.type}`);
  if (Boolean(verified.fsWatcherEnabled) !== false) fail("folder watcher must remain disabled for the controlled benchmark");
  if (args.role === "source" && verified.paused !== true) fail("source folder must remain paused until timed start");
  if (args.role === "destination" && verified.paused === true) fail("destination folder must be unpaused before timed source start");

  if (args.role === "destination") {
    await waitForDestinationIdle(baseUrl, apiKey, args.folderId);
  }

  console.log(`home=${args.home}`);
  console.log(`local_device_id=${systemStatus.myID}`);
  console.log(`peer_id=${args.peerId}`);
  console.log(`folder_id=${args.folderId}`);
  console.log(`folder_path=${args.folderPath}`);
  console.log(`role=${args.role}`);
  console.log(`folder_type=${expectedType}`);
  console.log(`fs_watcher_enabled=false`);
  console.log(`pre_config_files=${preConfigStats.files}`);
  console.log(`pre_config_bytes=${preConfigStats.bytes}`);
  if (args.sourceCorpus) console.log(`source_corpus=${args.sourceCorpus}`);
  console.log(`paused=${Boolean(verified.paused)}`);
  console.log(`peer_runtime_connected=true`);
  console.log("scope=isolated_p0_1_initial_sync_preparation_only");
}

main().catch((error) => {
  console.error(`error: ${error instanceof Error ? error.message : String(error)}`);
  process.exitCode = 2;
});
