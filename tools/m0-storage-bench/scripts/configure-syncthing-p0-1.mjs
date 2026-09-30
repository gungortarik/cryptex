#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";

function fail(message) {
  console.error(`error: ${message}`);
  process.exit(2);
}

function parseArgs(argv) {
  let home = null;

  for (let i = 0; i < argv.length; i += 1) {
    const arg = argv[i];
    if (arg === "--home") {
      if (i + 1 >= argv.length) fail("--home requires a value");
      home = argv[i + 1];
      i += 1;
      continue;
    }
    fail(`unknown argument: ${arg}`);
  }

  if (!home) fail("usage: configure-syncthing-p0-1.mjs --home <isolated-syncthing-home>");
  return { home: path.resolve(home) };
}

const { home } = parseArgs(process.argv.slice(2));
const normalized = home.replaceAll("\\", "/");

if (!normalized.includes("/Cryptex-Dev/") || !normalized.endsWith("/syncthing-p0-1")) {
  fail(`refusing to modify non-Cryptex P0-1 Syncthing home: ${home}`);
}

const configPath = path.join(home, "config.xml");
if (!fs.existsSync(configPath)) fail(`config.xml not found: ${configPath}`);

const original = fs.readFileSync(configPath, "utf8");
let updated = original;

const guiBlockPattern = /(<gui\b[^>]*>[\s\S]*?<address>)([^<]*)(<\/address>[\s\S]*?<\/gui>)/;
const guiMatch = updated.match(guiBlockPattern);
if (!guiMatch) fail("unable to locate GUI address in config.xml");
const previousGui = guiMatch[2];
updated = updated.replace(guiBlockPattern, `$1${"127.0.0.1:18384"}$3`);

const optionsPattern = /(<options\b[^>]*>)([\s\S]*?)(<\/options>)/;
const optionsMatch = updated.match(optionsPattern);
if (!optionsMatch) fail("unable to locate <options> block in config.xml");

let optionsBody = optionsMatch[2];
const listenerPattern = /^([ \t]*)<listenAddress>[^<]*<\/listenAddress>[ \t]*\r?\n?/gm;
const listenerMatches = [...optionsBody.matchAll(listenerPattern)];
const previousListeners = listenerMatches.map((match) => {
  const value = match[0].match(/<listenAddress>([^<]*)<\/listenAddress>/);
  return value ? value[1] : "";
});

if (listenerMatches.length > 0) {
  const indent = listenerMatches[0][1];
  let inserted = false;
  optionsBody = optionsBody.replace(listenerPattern, () => {
    if (inserted) return "";
    inserted = true;
    return `${indent}<listenAddress>tcp://0.0.0.0:22001</listenAddress>\n`;
  });
} else {
  const lineEnding = original.includes("\r\n") ? "\r\n" : "\n";
  optionsBody = `${lineEnding}        <listenAddress>tcp://0.0.0.0:22001</listenAddress>${optionsBody}`;
}

updated = updated.replace(optionsPattern, `$1${optionsBody}$3`);

const activeFolderPattern = /<folder\b(?=[^>]*\bid="[^"]+")(?=[^>]*\bpath="[^"]+")[^>]*>/g;
const activeFolders = updated.match(activeFolderPattern) ?? [];
if (activeFolders.length > 0) {
  fail("refusing to continue because active folder entries already exist in the isolated config");
}

const backupPath = `${configPath}.pre-p0-1.bak`;
if (!fs.existsSync(backupPath)) {
  fs.copyFileSync(configPath, backupPath);
}

if (updated !== original) {
  fs.writeFileSync(configPath, updated, "utf8");
}

console.log(`home=${home}`);
console.log(`config=${configPath}`);
console.log(`backup=${backupPath}`);
console.log(`previous_gui=${previousGui}`);
console.log(`gui=127.0.0.1:18384`);
console.log(`previous_listeners=${previousListeners.join(",") || "none"}`);
console.log(`listen_address=tcp://0.0.0.0:22001`);
console.log(`active_folder_count=${activeFolders.length}`);
console.log("scope=isolated_p0_1_syncthing_config_only");
