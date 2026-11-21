import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { once } from "node:events";
import { mkdir, mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";
import { createInterface } from "node:readline";

// Drive the installed Codex app-server over stdio. Only Codex talks MCP to the bridge.
export async function connectCodex(executable, bridge) {
  assert.ok(path.isAbsolute(executable) && path.isAbsolute(bridge));
  const root = await mkdtemp(path.join(tmpdir(), "cryptex-codex-"));
  const home = path.join(root, "codex");
  const workspace = path.join(root, "workspace");
  await Promise.all([mkdir(home), mkdir(workspace)]);
  const quote = JSON.stringify;
  await writeFile(path.join(home, "config.toml"), [
    'model_provider = "fixture"',
    '[model_providers.fixture]', 'name = "Offline fixture"', 'base_url = "http://127.0.0.1:1/v1"',
    'wire_api = "responses"', '[analytics]', 'enabled = false',
    '[projects.' + quote(workspace) + ']', 'trust_level = "trusted"',
    '[mcp_servers.cryptex]', 'command = ' + quote(process.execPath),
    'args = [' + quote(bridge) + ', "--public-notes"]', 'startup_timeout_sec = 30',
  ].join("\n") + "\n", { mode: 0o600 });
  const env = Object.fromEntries(Object.entries(process.env).filter(([name]) =>
    /^(path|systemroot|windir|comspec|pathext|temp|tmp|tmpdir|lang)$/i.test(name)));
  const child = spawn(executable, ["app-server", "--stdio"], {
    cwd: workspace, env: { ...env, CODEX_HOME: home, HOME: root, USERPROFILE: root },
    stdio: ["pipe", "pipe", "pipe"], windowsHide: true,
  });
  const exit = once(child, "close");
  let sequence = 0;
  const pending = new Map();
  function stopRequests(error) {
    for (const { reject } of pending.values()) reject(error);
    pending.clear();
  }
  child.on("error", stopRequests);
  child.stdin.on("error", stopRequests);
  child.once("close", code => stopRequests(new Error("Codex exited with status " + code)));
  child.stderr.resume();
  const input = createInterface({ input: child.stdout });
  input.on("line", line => {
    const message = JSON.parse(line);
    if (message.method && message.id !== undefined) {
      // Unexpected interactive requests fail closed in the unattended fixture.
      child.stdin.write(JSON.stringify({ id: message.id, error: { code: -32601, message: "Unsupported request" } }) + "\n");
      return;
    }
    const waiter = pending.get(message.id);
    if (!waiter) return;
    pending.delete(message.id);
    if (message.error) waiter.reject(new Error(waiter.method + ": " + message.error.message));
    else waiter.resolve(message.result);
  });
  async function rpc(method, params) {
    const id = ++sequence;
    let timer;
    try {
      return await new Promise((resolve, reject) => {
        timer = setTimeout(() => reject(new Error("Codex timed out on " + method)), 45_000);
        pending.set(id, { method, resolve, reject });
        child.stdin.write(JSON.stringify({ id, method, params }) + "\n");
      });
    } finally { clearTimeout(timer); pending.delete(id); }
  }
  async function close() {
    child.stdin.end();
    const timer = setTimeout(() => child.kill(), 3_000);
    try { await exit; } finally {
      clearTimeout(timer);
      input.close();
      await rm(root, { recursive: true, force: true, maxRetries: 5 });
    }
  }
  try {
    await rpc("initialize", { clientInfo: { name: "cryptex_acceptance", version: "1" }, capabilities: { experimentalApi: true } });
    child.stdin.write(JSON.stringify({ method: "initialized" }) + "\n");
    const { thread } = await rpc("thread/start", { cwd: workspace, ephemeral: true, sandbox: "read-only", approvalPolicy: "never" });
    const threadId = thread.id;
    const { data } = await rpc("mcpServerStatus/list", { threadId });
    const server = data.find(s => s.name === "cryptex");
    assert.deepEqual(Object.keys(server?.tools ?? {}).sort(), [
      "cryptex_health", "cryptex_request_public_read", "cryptex_search_public_notes",
    ], "Codex must discover the installed Cryptex bridge");
    return { close, callTool: ({ name, arguments: args }) =>
      rpc("mcpServer/tool/call", { threadId, server: "cryptex", tool: name, arguments: args }) };
  } catch (error) {
    await close();
    throw error;
  }
}
