import { Server } from "@modelcontextprotocol/sdk/server/index.js";
import { StdioServerTransport } from "@modelcontextprotocol/sdk/server/stdio.js";
import {
  CallToolRequestSchema, ErrorCode, ListToolsRequestSchema, McpError,
} from "@modelcontextprotocol/sdk/types.js";
import { probeHealth } from "./health.mjs";

const health = {
  name: "cryptex_health",
  description: "Check whether the local Cryptex daemon answers its health ping.",
  inputSchema: { type: "object", properties: {}, additionalProperties: false },
  annotations: { readOnlyHint: true, openWorldHint: false },
};

export async function serve(endpoint) {
  const server = new Server({ name: "cryptex-local", version: "0.0.0" }, { capabilities: { tools: {} } });
  server.setRequestHandler(ListToolsRequestSchema, () => ({ tools: [health] }));
  let pending;
  server.setRequestHandler(CallToolRequestSchema, async ({ params }) => {
    if (params.name !== health.name || Object.keys(params.arguments ?? {}).length !== 0) {
      throw new McpError(ErrorCode.InvalidParams, "Unknown tool or unsupported arguments");
    }
    // Share concurrent requests only. The next call after completion probes again.
    pending ??= probeHealth(endpoint).catch(() => false).finally(() => { pending = undefined; });
    const ok = await pending;
    return {
      isError: !ok,
      content: [{ type: "text", text: ok ? "daemon_responding" : "daemon_unavailable" }],
    };
  });
  await server.connect(new StdioServerTransport(process.stdin, process.stdout, { maxBufferSize: 65536 }));
}
