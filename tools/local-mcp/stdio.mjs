#!/usr/bin/env node
import { localEndpoint } from "./health.mjs";
import { serve } from "./server.mjs";

try {
  await serve(localEndpoint());
} catch {
  process.stderr.write("Cryptex local MCP startup failed.\n");
  process.exitCode = 1;
}
