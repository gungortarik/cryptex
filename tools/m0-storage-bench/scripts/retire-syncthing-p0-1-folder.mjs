#!/usr/bin/env node

/**
 * Guarded retirement of one isolated P0-1 Syncthing benchmark folder.
 *
 * Removes the folder from the isolated Cryptex-Dev Syncthing config only.
 * Does NOT delete retained benchmark corpora.
 * Does NOT delete working run directories automatically (optional --delete-work-path).
 * Refuses repository paths, arbitrary user paths, and non-P0-1 Syncthing homes.
 */

import fs from "node:fs";
import fsp from "node:fs/promises";
import path from "node:path";

function fail(message) {
  throw new Error(message);
}

function parseArgs(argv) {
  const values = new Map();
  let deleteWorkPath = false;
  for (let i = 0; i < argv.length; i += 1) {
    const arg = argv[i];
    if (arg === "--delete-work-path") {
      deleteWorkPath = true;
      continue;
    }
    if (!arg.startsWith("--")) fail(`unexpected argument: ${arg}`);
    if (i + 1 >= argv.length) fail(`${arg} requires a value`);
    values.set(arg, argv[i + 1]);
    i += 1;
  }

  const home = values.get("--home");
  const folderId = values.get("--folder-id");
  const expectedLocalDeviceId = values.get("--expected-local-device-id");
  const expectedPeerId = values.get("--expected-peer-id");
  if (!home || !folderId || !expectedLocalDeviceId || !expectedPeerId) {
    fail(
      "usage: retire-syncthing-p0-1-folder.mjs --home <isolated-home> --folder-id <id> --expected-local-device-id <id> --expected-peer-id <id> [--delete-work-path]",
    );
  }
  if (!/^[a-z0-9-]+$/.test(folderId)) fail("folder-id format is invalid");
  if (!folderId.startsWith("p0-1-")) fail("folder-id must start with p0-1-");

  return {
    home: path.resolve(home),
    folderId,
    expectedLocalDeviceId: expectedLocalDeviceId.trim(),
    expectedPeerId: expectedPeerId.trim(),
    deleteWorkPath,
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
  if (normalized.includes("/.config/syncthing") || normalized.endsWith("/syncthing")) {
    fail(`refusing normal/global Syncthing home: ${home}`);
  }
}

function assertRunPathGuard(folderPath) {
  const normalized = normalizeForGuard(folderPath);
  if (!normalized.includes("/Cryptex-Dev/syncthing-p0-1-runs/")) {
    fail(`refusing to touch path outside syncthing-p0-1-runs: ${folderPath}`);
  }
  if (normalized.includes("/Cryptex-Dev/benchmarks/")) {
    fail(`refusing retained benchmark corpus path: ${folderPath}`);
  }
  const repoMarkers = ["/github_repos/", "/cryptex/tools/", "/cryptex/evidence/", "/cryptex/.git"];
  for (const marker of repoMarkers) {
    if (normalized.includes(marker)) fail(`refusing repository path: ${folderPath}`);
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

async function main() {
  const args = parseArgs(process.argv.slice(2));
  assertHomeGuard(args.home);

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
  if (systemStatus?.myID !== args.expectedLocalDeviceId) {
    fail(
      `local device ID mismatch: expected ${args.expectedLocalDeviceId}, observed ${systemStatus?.myID ?? "missing"}`,
    );
  }

  const devices = await requestJson(`${baseUrl}/rest/config/devices`, apiKey);
  const remotes = (devices ?? []).filter((device) => device.deviceID !== systemStatus.myID);
  if (remotes.length !== 1 || remotes[0].deviceID !== args.expectedPeerId) {
    fail(`unexpected remote device set; refusing retirement`);
  }

  const folders = await requestJson(`${baseUrl}/rest/config/folders`, apiKey);
  if (!Array.isArray(folders)) fail("unexpected folders response");
  const folder = folders.find((entry) => entry.id === args.folderId);
  if (!folder) fail(`folder not configured: ${args.folderId}`);
  assertRunPathGuard(folder.path);

  if (folder.paused !== true) {
    await requestJson(`${baseUrl}/rest/config/folders/${encodeURIComponent(args.folderId)}`, apiKey, {
      method: "PATCH",
      body: JSON.stringify({ paused: true }),
    });
  }

  await requestJson(`${baseUrl}/rest/config/folders/${encodeURIComponent(args.folderId)}`, apiKey, {
    method: "DELETE",
  });

  const remaining = await requestJson(`${baseUrl}/rest/config/folders`, apiKey);
  if ((remaining ?? []).some((entry) => entry.id === args.folderId)) {
    fail("folder remained configured after DELETE");
  }

  let workPathDeleted = false;
  if (args.deleteWorkPath) {
    const workRoot = path.resolve(folder.path, "..");
    assertRunPathGuard(workRoot);
    if (!workRoot.includes(args.folderId)) {
      fail(`refusing delete: parent path does not contain folder-id: ${workRoot}`);
    }
    await fsp.rm(workRoot, { recursive: true, force: false });
    workPathDeleted = true;
  }

  console.log(`home=${args.home}`);
  console.log(`local_device_id=${systemStatus.myID}`);
  console.log(`peer_id=${args.expectedPeerId}`);
  console.log(`folder_id=${args.folderId}`);
  console.log(`folder_path=${folder.path}`);
  console.log(`config_removed=true`);
  console.log(`work_path_deleted=${workPathDeleted}`);
  console.log("retained_corpus_untouched=true");
  console.log("scope=isolated_p0_1_folder_retirement_only");
}

main().catch((error) => {
  console.error(`error: ${error instanceof Error ? error.message : String(error)}`);
  process.exitCode = 2;
});
