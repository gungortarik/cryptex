#!/usr/bin/env node

import fs from "node:fs";
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
  const peerName = values.get("--peer-name");
  const peerAddress = values.get("--peer-address");

  if (!home || !peerId || !peerName || !peerAddress) {
    fail(
      "usage: pair-syncthing-p0-1.mjs --home <isolated-home> --peer-id <device-id> --peer-name <name> --peer-address <tcp://private-ip:22001>",
    );
  }

  return {
    home: path.resolve(home),
    peerId: peerId.trim(),
    peerName: peerName.trim(),
    peerAddress: peerAddress.trim(),
  };
}

function isPrivateIpv4(value) {
  const parts = value.split(".").map(Number);
  if (parts.length !== 4 || parts.some((part) => !Number.isInteger(part) || part < 0 || part > 255)) {
    return false;
  }
  if (parts[0] === 10) return true;
  if (parts[0] === 172 && parts[1] >= 16 && parts[1] <= 31) return true;
  if (parts[0] === 192 && parts[1] === 168) return true;
  return false;
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
  const { home, peerId, peerName, peerAddress } = parseArgs(process.argv.slice(2));
  const normalized = home.replaceAll("\\", "/");
  if (!normalized.includes("/Cryptex-Dev/") || !normalized.endsWith("/syncthing-p0-1")) {
    fail(`refusing non-Cryptex P0-1 Syncthing home: ${home}`);
  }

  if (!/^[A-Z0-9-]+$/.test(peerId)) fail("peer device ID format is invalid");
  if (!peerName || peerName.length > 64) fail("peer name is empty or too long");

  const addressMatch = peerAddress.match(/^tcp:\/\/(\d{1,3}(?:\.\d{1,3}){3}):22001$/);
  if (!addressMatch || !isPrivateIpv4(addressMatch[1])) {
    fail("peer address must be an RFC1918 IPv4 address using tcp://<ip>:22001");
  }

  const configPath = path.join(home, "config.xml");
  if (!fs.existsSync(configPath)) fail(`config.xml not found: ${configPath}`);
  const xml = fs.readFileSync(configPath, "utf8");

  const guiMatch = xml.match(/<gui\b[^>]*>[\s\S]*?<address>([^<]+)<\/address>[\s\S]*?<apikey>([^<]+)<\/apikey>[\s\S]*?<\/gui>/);
  if (!guiMatch) fail("unable to read isolated GUI address/API key from config.xml");
  const guiAddress = guiMatch[1].trim();
  const apiKey = guiMatch[2].trim();
  if (guiAddress !== "127.0.0.1:18384") fail(`unexpected isolated GUI address: ${guiAddress}`);
  if (!apiKey) fail("isolated Syncthing API key is empty");

  const baseUrl = `http://${guiAddress}`;
  const folders = await requestJson(`${baseUrl}/rest/config/folders`, apiKey);
  if (!Array.isArray(folders)) fail("unexpected folders response");
  if (folders.length !== 0) fail(`active benchmark folders already exist (${folders.length}); refusing pairing mutation`);

  const status = await requestJson(`${baseUrl}/rest/system/status`, apiKey);
  const localDeviceId = status?.myID;
  if (typeof localDeviceId !== "string" || !localDeviceId) {
    fail("unable to determine local Syncthing device ID");
  }
  if (localDeviceId === peerId) fail("peer device ID must not equal the local device ID");

  const devices = await requestJson(`${baseUrl}/rest/config/devices`, apiKey);
  if (!Array.isArray(devices)) fail("unexpected devices response");

  const remoteDevices = devices.filter((device) => device.deviceID !== localDeviceId);
  const unrelated = remoteDevices.filter((device) => device.deviceID !== peerId);
  if (unrelated.length > 0) {
    fail(`unexpected remote devices already configured: ${unrelated.map((device) => device.deviceID).join(",")}`);
  }

  const existingPeer = remoteDevices.find((device) => device.deviceID === peerId) ?? null;
  let action = "already_configured";
  if (!existingPeer) {
    const template = await requestJson(`${baseUrl}/rest/config/defaults/device`, apiKey);
    if (!template || typeof template !== "object") fail("unexpected default device template response");

    template.deviceID = peerId;
    template.name = peerName;
    template.addresses = [peerAddress];
    template.introducer = false;
    template.skipIntroductionRemovals = false;
    template.paused = false;
    template.autoAcceptFolders = false;
    template.allowedNetworks = [];

    await requestJson(`${baseUrl}/rest/config/devices`, apiKey, {
      method: "POST",
      body: JSON.stringify(template),
    });
    action = "added";
  }

  const verified = await requestJson(
    `${baseUrl}/rest/config/devices/${encodeURIComponent(peerId)}`,
    apiKey,
  );
  if (!verified || verified.deviceID !== peerId) fail("peer verification returned the wrong device ID");
  if (!Array.isArray(verified.addresses) || verified.addresses.length !== 1 || verified.addresses[0] !== peerAddress) {
    fail(`peer verification returned unexpected addresses: ${JSON.stringify(verified.addresses)}`);
  }
  if (verified.autoAcceptFolders !== false || verified.introducer !== false || verified.paused !== false) {
    fail("peer verification returned unsafe device flags");
  }

  const restartState = await requestJson(`${baseUrl}/rest/config/restart-required`, apiKey);

  console.log(`home=${home}`);
  console.log(`local_device_id=${localDeviceId}`);
  console.log(`peer_id=${peerId}`);
  console.log(`peer_name=${peerName}`);
  console.log(`peer_address=${peerAddress}`);
  console.log(`peer_action=${action}`);
  console.log("peer_address_mode=explicit_direct_tcp_only");
  console.log("auto_accept_folders=false");
  console.log("introducer=false");
  console.log("paused=false");
  console.log("active_folder_count=0");
  console.log(`restart_required=${Boolean(restartState?.requiresRestart)}`);
  console.log("scope=isolated_p0_1_peer_pairing_only");
}

main().catch((error) => {
  console.error(`error: ${error instanceof Error ? error.message : String(error)}`);
  process.exitCode = 2;
});
