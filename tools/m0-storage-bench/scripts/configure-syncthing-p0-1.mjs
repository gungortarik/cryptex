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

function setSingleOption(body, name, value, lineEnding) {
  const pattern = new RegExp(
    `^([ \\t]*)<${name}>[^<]*<\\/${name}>[ \\t]*\\r?\\n?`,
    "gm",
  );
  const matches = [...body.matchAll(pattern)];
  const previous = matches.map((match) => {
    const valueMatch = match[0].match(new RegExp(`<${name}>([^<]*)<\\/${name}>`));
    return valueMatch ? valueMatch[1] : "";
  });

  if (matches.length > 0) {
    const indent = matches[0][1];
    let inserted = false;
    const updated = body.replace(pattern, () => {
      if (inserted) return "";
      inserted = true;
      return `${indent}<${name}>${value}</${name}>${lineEnding}`;
    });
    return { body: updated, previous };
  }

  const indentMatch = body.match(/^([ \t]*)<listenAddress>/m);
  const indent = indentMatch ? indentMatch[1] : "        ";
  return {
    body: `${lineEnding}${indent}<${name}>${value}</${name}>${body}`,
    previous,
  };
}

const { home } = parseArgs(process.argv.slice(2));
const normalized = home.replaceAll("\\", "/");

if (!normalized.includes("/Cryptex-Dev/") || !normalized.endsWith("/syncthing-p0-1")) {
  fail(`refusing to modify non-Cryptex P0-1 Syncthing home: ${home}`);
}

const configPath = path.join(home, "config.xml");
if (!fs.existsSync(configPath)) fail(`config.xml not found: ${configPath}`);

const original = fs.readFileSync(configPath, "utf8");
const lineEnding = original.includes("\r\n") ? "\r\n" : "\n";
let updated = original;

const guiBlockPattern = /(<gui\b[^>]*>[\s\S]*?<address>)([^<]*)(<\/address>[\s\S]*?<\/gui>)/;
const guiMatch = updated.match(guiBlockPattern);
if (!guiMatch) fail("unable to locate GUI address in config.xml");
const previousGui = guiMatch[2];
updated = updated.replace(
  guiBlockPattern,
  (_match, prefix, _value, suffix) => `${prefix}127.0.0.1:18384${suffix}`,
);

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
    return `${indent}<listenAddress>tcp://0.0.0.0:22001</listenAddress>${lineEnding}`;
  });
} else {
  optionsBody = `${lineEnding}        <listenAddress>tcp://0.0.0.0:22001</listenAddress>${optionsBody}`;
}

const transportOptions = [
  ["globalAnnounceEnabled", "false"],
  ["localAnnounceEnabled", "false"],
  ["relaysEnabled", "false"],
  ["natEnabled", "false"],
];
const previousTransportOptions = new Map();
for (const [name, value] of transportOptions) {
  const result = setSingleOption(optionsBody, name, value, lineEnding);
  optionsBody = result.body;
  previousTransportOptions.set(name, result.previous);
}

updated = updated.replace(
  optionsPattern,
  (_match, prefix, _body, suffix) => `${prefix}${optionsBody}${suffix}`,
);

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
console.log("gui=127.0.0.1:18384");
console.log(`previous_listeners=${previousListeners.join(",") || "none"}`);
console.log("listen_address=tcp://0.0.0.0:22001");
for (const [name, value] of transportOptions) {
  const previous = previousTransportOptions.get(name) ?? [];
  console.log(`previous_${name}=${previous.join(",") || "missing"}`);
  console.log(`${name}=${value}`);
}
console.log(`active_folder_count=${activeFolders.length}`);
console.log("transport_scope=direct_tcp_only");
console.log("scope=isolated_p0_1_syncthing_config_only");
