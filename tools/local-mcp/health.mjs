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

export async function probeHealth(endpoint) {
  if (process.platform === "darwin") {
    const parent = await lstat(dirname(endpoint));
    if (!parent.isDirectory() || parent.uid !== process.getuid() || (parent.mode & 0o077) !== 0) {
      return false;
    }
    if (!(await lstat(endpoint)).isSocket()) return false;
  }
  const requestId = randomUUID();
  const request = {
    protocol_version: 1, request_id: requestId, client_role: "mcp",
    capabilities: ["status.read"], operation: "health.ping",
  };
  return new Promise((resolve) => {
    const socket = createConnection({ path: endpoint });
    const timer = setTimeout(() => finish(false), 2000);
    let frame = Buffer.alloc(0);
    function finish(ok) {
      clearTimeout(timer);
      socket.destroy();
      resolve(ok);
    }
    socket.once("error", () => finish(false));
    socket.once("close", () => finish(false));
    socket.once("connect", () => socket.write(JSON.stringify(request) + "\n"));
    socket.on("data", (chunk) => {
      if (frame.length + chunk.length > 4096) return finish(false);
      frame = Buffer.concat([frame, chunk]);
      const end = frame.indexOf(10);
      if (end < 0) return;
      try {
        const pong = JSON.parse(frame.subarray(0, end).toString("utf8"));
        finish(pong.protocol_version === 1 && pong.request_id === requestId &&
          pong.operation === "health.pong" && pong.accepted === true && pong.error === null &&
          Number.isSafeInteger(pong.daemon_pid) && pong.daemon_pid > 0);
      } catch {
        finish(false);
      }
    });
  });
}
