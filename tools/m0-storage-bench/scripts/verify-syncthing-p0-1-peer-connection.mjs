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
  const peerAddress = values.get("--peer-address");
  if (!home || !peerId || !peerAddress) {
    fail(
      "usage: verify-syncthing-p0-1-peer-connection.mjs --home <isolated-home> --peer-id <device-id> --peer-address <tcp://private-ip:22001>",
    );
  }

  return {
    home: path.resolve(home),
    peerId: peerId.trim(),
    peerAddress: peerAddress.trim(),
  };
}

async function requestJson(url, apiKey) {
  const response = await fetch(url, {
    headers: { "X-API-Key": apiKey },
  });
  const text = await response.text();
  if (!response.ok) {
    fail(`GET ${url} failed: HTTP ${response.status} ${text}`);
  }
  if (!text.trim()) return null;
  try {
    return JSON.parse(text);
  } catch {
    fail(`non-JSON response from ${url}: ${text}`);
  }
}

async function main() {
  const { home, peerId, peerAddress } = parseArgs(process.argv.slice(2));
  const normalized = home.replaceAll("\\", "/");
  if (!normalized.includes("/Cryptex-Dev/") || !normalized.endsWith("/syncthing-p0-1")) {
    fail(`refusing non-Cryptex P0-1 Syncthing home: ${home}`);
  }

  const addressMatch = peerAddress.match(/^tcp:\/\/(\d{1,3}(?:\.\d{1,3}){3}):22001$/);
  if (!addressMatch) fail("peer address must be tcp://<ipv4>:22001");
  const expectedPeerIp = addressMatch[1];

  const configPath = path.join(home, "config.xml");
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
  const status = await requestJson(`${baseUrl}/rest/system/status`, apiKey);
  if (!status?.myID) fail("system status did not return local device ID");

  const folders = await requestJson(`${baseUrl}/rest/config/folders`, apiKey);
  if (!Array.isArray(folders)) fail("unexpected folders response");
  if (folders.length !== 0) fail(`active benchmark folders already exist (${folders.length})`);

  const devices = await requestJson(`${baseUrl}/rest/config/devices`, apiKey);
  if (!Array.isArray(devices)) fail("unexpected devices response");
  const remotes = devices.filter((device) => device.deviceID !== status.myID);
  if (remotes.length !== 1 || remotes[0].deviceID !== peerId) {
    fail(`unexpected remote device set: ${remotes.map((device) => device.deviceID).join(",") || "none"}`);
  }

  const peer = remotes[0];
  if (!Array.isArray(peer.addresses) || peer.addresses.length !== 1 || peer.addresses[0] !== peerAddress) {
    fail(`unexpected configured peer addresses: ${JSON.stringify(peer.addresses)}`);
  }
  if (peer.autoAcceptFolders !== false || peer.introducer !== false || peer.paused !== false) {
    fail("peer configuration contains unsafe flags");
  }

  const connections = await requestJson(`${baseUrl}/rest/system/connections`, apiKey);
  const runtime = connections?.connections?.[peerId];
  if (!runtime) fail("runtime connection entry for peer is missing");
  if (runtime.connected !== true) fail("peer is not connected at runtime");
  if (typeof runtime.type !== "string" || !/^tcp-/i.test(runtime.type)) {
    fail(`unexpected runtime connection type: ${runtime.type ?? "missing"}`);
  }
  if (typeof runtime.address !== "string" || !runtime.address.startsWith(`${expectedPeerIp}:`)) {
    fail(`runtime peer address does not match expected LAN IP: ${runtime.address ?? "missing"}`);
  }
  if (runtime.paused === true) fail("runtime peer connection is paused");

  console.log(`home=${home}`);
  console.log(`local_device_id=${status.myID}`);
  console.log(`peer_id=${peerId}`);
  console.log(`configured_peer_address=${peerAddress}`);
  console.log("peer_address_mode=explicit_direct_tcp_only");
  console.log("runtime_connected=true");
  console.log(`runtime_type=${runtime.type}`);
  console.log(`runtime_address=${runtime.address}`);
  console.log(`runtime_client_version=${runtime.clientVersion ?? "unknown"}`);
  console.log("runtime_peer_ip_matches=true");
  console.log(`runtime_is_local=${Boolean(runtime.isLocal)}`);
  console.log("active_folder_count=0");
  console.log("scope=isolated_p0_1_peer_connection_verification_only");
}

main().catch((error) => {
  console.error(`error: ${error instanceof Error ? error.message : String(error)}`);
  process.exitCode = 2;
});
