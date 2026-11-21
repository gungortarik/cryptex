import { spawn } from "node:child_process";
import { writeFile } from "node:fs/promises";
import { createInterface } from "node:readline";

// The witness forwards discovery only. No tool invocation can reach a daemon.
const [entry, receipt] = process.argv.slice(2);
const bridge = spawn(process.execPath, [entry, "--public-notes"], {
  stdio: ["pipe", "pipe", "ignore"], windowsHide: true,
});
let initialized;
let initializedId;
const listings = new Set();
const tools = ["cryptex_health", "cryptex_request_public_read", "cryptex_search_public_notes"];
createInterface({ input: process.stdin }).on("line", line => {
  const m = JSON.parse(line);
  if (m.method === "initialize") initializedId = m.id;
  else if (m.method === "tools/list") listings.add(m.id);
  else if (!["ping", "notifications/initialized", "notifications/cancelled"].includes(m.method)) {
    if (m.id !== undefined) process.stdout.write(JSON.stringify({
      jsonrpc: "2.0", id: m.id, error: { code: -32601, message: "Discovery only" },
    }) + "\n");
    return;
  }
  bridge.stdin.write(line + "\n");
}).on("close", () => bridge.stdin.end());
for await (const line of createInterface({ input: bridge.stdout })) {
  const m = JSON.parse(line);
  if (m.id === initializedId && m.result?.serverInfo?.name === "cryptex-local") initialized = true;
  if (initialized && listings.delete(m.id) &&
      JSON.stringify(m.result?.tools?.map(t => t.name).sort()) === JSON.stringify(tools)) {
    await writeFile(receipt, JSON.stringify({ bridge: "cryptex-local", tools }), { mode: 0o600 });
  }
  process.stdout.write(line + "\n");
}
