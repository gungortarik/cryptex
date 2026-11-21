import assert from "node:assert/strict";
import { mkdtemp, rm } from "node:fs/promises";
import { createServer } from "node:net";
import { randomUUID } from "node:crypto";
import { once } from "node:events";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import test from "node:test";
import { Client } from "@modelcontextprotocol/sdk/client/index.js";
import { StdioClientTransport } from "@modelcontextprotocol/sdk/client/stdio.js";
import { ErrorCode } from "@modelcontextprotocol/sdk/types.js";

const stdio = fileURLToPath(new URL("../stdio.mjs", import.meta.url));
const fixture = fileURLToPath(new URL("./fixture-stdio.mjs", import.meta.url));
const health = { name: "cryptex_health", arguments: {} };

async function connect(t, args) {
  const transport = new StdioClientTransport({ command: process.execPath, args, stderr: "pipe" });
  let diagnostics = "";
  transport.stderr.on("data", (chunk) => { diagnostics += chunk; });
  const client = new Client({ name: "disposable-client", version: "1" });
  t.after(async () => {
    await client.close();
    assert.equal(diagnostics, "");
  });
  await client.connect(transport);
  return client;
}

function check(result, ok) {
  assert.equal(result.isError, !ok);
  assert.deepEqual(result.content, [{ type: "text", text: ok ? "daemon_responding" : "daemon_unavailable" }]);
}

test("normal stdio entry advertises the read-only tools without contacting a profile", async (t) => {
  const client = await connect(t, [stdio]);
  assert.deepEqual(Object.keys(client.getServerCapabilities()), ["tools"]);
  assert.deepEqual((await client.listTools()).tools.map((tool) => tool.name), [health.name, "cryptex_search_public_notes"]);
  for (const args of [{}, { phrase: "  " }, { phrase: "é".repeat(513) },
    { phrase: "x", classification: "private" }, { phrase: "x", role: "desktop" }]) {
    await assert.rejects(client.callTool({ name: "cryptex_search_public_notes", arguments: args }),
      { code: ErrorCode.InvalidParams });
  }
  for (const name of ["purge", "recovery", "search", "capture", "owner.security"]) {
    await assert.rejects(client.callTool({ name }), { code: ErrorCode.InvalidParams });
  }
  await assert.rejects(client.callTool({ ...health, arguments: { role: "desktop", path: "PRIVATE-CANARY" } }),
    { code: ErrorCode.InvalidParams });
});

test("absent, mismatched, oversized and stalled IPC responses stay unavailable", { timeout: 25000 }, async (t) => {
  const dir = await mkdtemp(join(process.platform === "darwin" ? "/tmp" : tmpdir(), "cpx-m12-"));
  t.after(() => rm(dir, { recursive: true, force: true }));
  const endpoint = process.platform === "win32" ? "\\\\.\\pipe\\cryptex-m12-" + randomUUID() : join(dir, "s");
  const client = await connect(t, [fixture, endpoint]);
  check(await client.callTool(health), false);
  const modes = ["mismatched", "oversized", "stalled"];
  const requests = [];
  const server = createServer((socket) => {
    let data = "";
    socket.on("error", () => {});
    socket.on("data", (chunk) => {
      data += chunk;
      if (!data.includes("\n")) return;
      const request = JSON.parse(data);
      requests.push(request);
      const mode = modes.shift();
      if (mode === "mismatched") socket.end(JSON.stringify({ protocol_version: 1, request_id: "wrong",
        accepted: true, operation: "health.pong", daemon_pid: 1, error: null }) + "\n");
      if (mode === "oversized") socket.end("x".repeat(4097));
    });
  });
  server.listen(endpoint);
  await once(server, "listening");
  try {
    for (let i = 0; i < 3; i++) check(await client.callTool(health), false);
  } finally {
    await new Promise((resolve) => server.close(resolve));
  }
  assert.equal(requests.length, 3);
  assert.equal(new Set(requests.map((r) => r.request_id)).size, 3);
  for (const request of requests) {
    assert.deepEqual(request, { protocol_version: 1, request_id: request.request_id, client_role: "mcp",
      capabilities: ["status.read"], operation: "health.ping" });
  }
});

// Only the native Rust test supplies this unique endpoint; it serves one real daemon exchange.
if (process.env.CRYPTEX_M12_TEST_ENDPOINT) {
  test("live daemon answers once and the same client observes its disappearance", { timeout: 15000 }, async (t) => {
    const client = await connect(t, [fixture, process.env.CRYPTEX_M12_TEST_ENDPOINT]);
    check(await client.callTool(health), true);
    check(await client.callTool(health), false);
  });
}
