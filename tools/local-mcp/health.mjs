import { randomUUID } from "node:crypto";
import { lstat } from "node:fs/promises";
import { createConnection } from "node:net";
import { dirname } from "node:path";

// Match cryptexd's product_socket_name. Endpoint selection is outside the MCP schema.
export function localEndpoint() {
  if (process.platform === "win32") return "\\\\.\\pipe\\cryptex-cryptexd-v1";
  if (process.platform === "darwin") return "/tmp/cpx-cryptex-" + process.getuid() + "/s";
  throw new Error("Unsupported platform");
}

export async function exchange(endpoint, fields, maxBytes, timeout) {
  if (process.platform === "darwin") {
    const parent = await lstat(dirname(endpoint));
    if (!parent.isDirectory() || parent.uid !== process.getuid() || (parent.mode & 0o077) !== 0) {
      return null;
    }
    if (!(await lstat(endpoint)).isSocket()) return null;
  }
  const requestId = randomUUID();
  const request = {
    protocol_version: 1, request_id: requestId, client_role: "mcp",
    ...fields,
  };
  return new Promise((resolve) => {
    const socket = createConnection({ path: endpoint });
    const timer = setTimeout(() => finish(null), timeout);
    let frame = Buffer.alloc(0);
    function finish(ok) {
      clearTimeout(timer);
      socket.destroy();
      resolve(ok);
    }
    socket.once("error", () => finish(null));
    socket.once("close", () => finish(null));
    socket.once("connect", () => socket.write(JSON.stringify(request) + "\n"));
    socket.on("data", (chunk) => {
      if (frame.length + chunk.length > maxBytes) return finish(null);
      frame = Buffer.concat([frame, chunk]);
      const end = frame.indexOf(10);
      if (end < 0) return;
      try {
        const reply = JSON.parse(frame.subarray(0, end).toString("utf8"));
        finish(reply?.protocol_version === 1 && reply.request_id === requestId ? reply : null);
      } catch {
        finish(null);
      }
    });
  });
}

export async function probeHealth(endpoint) {
  const pong = await exchange(endpoint, { capabilities: ["status.read"], operation: "health.ping" }, 4096, 2000);
  return pong?.operation === "health.pong" && pong.accepted === true && pong.error === null &&
    Number.isSafeInteger(pong.daemon_pid) && pong.daemon_pid > 0;
}
