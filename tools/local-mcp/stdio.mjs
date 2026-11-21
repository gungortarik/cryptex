#!/usr/bin/env node
import { localEndpoint } from "./health.mjs";
import { serve } from "./server.mjs";

try {
  await serve(localEndpoint(), { publicNotes: process.argv.length === 3 && process.argv[2] === "--public-notes" });
} catch {
  process.stderr.write("Cryptex local MCP startup failed.\n");
  process.exitCode = 1;
}
