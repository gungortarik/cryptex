#!/usr/bin/env node
import { fileURLToPath } from "node:url";

export function clientConfiguration(client, entry = fileURLToPath(new URL("./stdio.mjs", import.meta.url))) {
  const server = { command: process.execPath, args: [entry, "--public-notes"] };
  if ([server.command, entry].some(value => /[\u0000-\u001f$]/.test(value))) {
    throw new Error("Choose an installation path without control characters or dollar signs");
  }
  switch (client) {
    case "cursor":
    case "claude":
    case "antigravity":
      return JSON.stringify({ mcpServers: { cryptex: server } }, null, 2) + "\n";
    case "codex":
    case "grok":
      return "[mcp_servers.cryptex]\ncommand = " + JSON.stringify(server.command) +
        "\nargs = " + JSON.stringify(server.args) + "\n";
    default:
      throw new Error("Choose codex, cursor, claude, antigravity or grok");
  }
}

if (import.meta.main) {
  try {
    if (process.argv.length !== 3) throw new Error("Usage: cryptex-mcp-config <client>");
    process.stdout.write(clientConfiguration(process.argv[2]));
  } catch (error) {
    process.stderr.write(error.message + "\n");
    process.exitCode = 1;
  }
}
