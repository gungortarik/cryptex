import { Server } from "@modelcontextprotocol/sdk/server/index.js";
import { StdioServerTransport } from "@modelcontextprotocol/sdk/server/stdio.js";
import {
  CallToolRequestSchema, ErrorCode, ListToolsRequestSchema, McpError,
} from "@modelcontextprotocol/sdk/types.js";
import { probeHealth } from "./health.mjs";
import { beginReview } from "./consent.mjs";

const health = {
  name: "cryptex_health",
  description: "Check whether the local Cryptex daemon answers its health ping.",
  inputSchema: { type: "object", properties: {}, additionalProperties: false },
  annotations: { readOnlyHint: true, openWorldHint: false },
};

const search = {
  name: "cryptex_search_public_notes",
  description: "Find up to ten visible public note excerpts containing a phrase.",
  inputSchema: { type: "object", required: ["phrase"], properties: {
    phrase: { type: "string", minLength: 1, maxLength: 1024 },
  }, additionalProperties: false },
  annotations: { readOnlyHint: true, openWorldHint: false },
};

const requestRead = {
  name: "cryptex_request_public_read",
  description: "Request owner approval in Cryptex Desktop for one public-note search. Show the returned review code to the user.",
  inputSchema: health.inputSchema,
  annotations: { readOnlyHint: true, openWorldHint: false },
};

async function searchPublicNotes(takeReview, args) {
  if (Object.keys(args).length !== 1 || typeof args.phrase !== "string" ||
      !args.phrase.trim() || Buffer.byteLength(args.phrase) > 1024) {
    throw new McpError(ErrorCode.InvalidParams, "Provide only a nonempty phrase of up to 1024 UTF-8 bytes");
  }
  const operation = "context.public.search";
  const review = takeReview();
  if (!review) return { isError: true, content: [{ type: "text", text: "public_read_denied" }] };
  const reply = await review.search(args.phrase).catch(() => null);
  const notes = reply?.result?.Ok;
  if (reply?.operation !== operation || !Array.isArray(notes) || Object.keys(reply.result).length !== 1 || notes.length > 10 ||
      !notes.every((n) => n && typeof n.event_id === "string" &&
        /^[0-9a-f]{8}(?:-[0-9a-f]{4}){3}-[0-9a-f]{12}$/i.test(n.event_id) &&
        typeof n.text === "string" && [...n.text].length <= 240)) {
    const denied = reply?.operation === operation && reply.result?.Err === "unauthorized";
    return { isError: true, content: [{ type: "text", text: denied ? "public_read_denied" : "public_read_unavailable" }] };
  }
  const result = { notes: notes.map(({ event_id, text }) => ({ event_id, text })) };
  return { structuredContent: result, content: [{ type: "text", text: JSON.stringify(result) }] };
}

export async function serve(endpoint, { publicNotes = false } = {}) {
  const server = new Server({ name: "cryptex-local", version: "0.0.0" }, { capabilities: { tools: {} } });
  server.setRequestHandler(ListToolsRequestSchema, () => ({ tools: publicNotes ? [health, requestRead, search] : [health] }));
  let review;
  let requesting = false;
  let active;
  process.stdin.once("end", () => { review?.close(); active?.close(); });
  let pending;
  server.setRequestHandler(CallToolRequestSchema, async ({ params }) => {
    if (publicNotes && params.name === requestRead.name && Object.keys(params.arguments ?? {}).length === 0) {
      if (requesting || active) return { isError: true, content: [{ type: "text", text: "public_review_busy" }] };
      requesting = true;
      try {
        if (!review || review.closed) review = await beginReview(endpoint);
        return { content: [{ type: "text", text: "In Cryptex, open Connection settings and select Review AI access. Compare this code before allowing one search: " + review.ticket }] };
      } catch {
        review = undefined;
        return { isError: true, content: [{ type: "text", text: "public_review_unavailable" }] };
      } finally { requesting = false; }
    }
    if (publicNotes && params.name === search.name) {
      let consumed;
      try {
        return await searchPublicNotes(() => {
          consumed = review;
          review = undefined;
          if (consumed) active = consumed;
          return consumed;
        }, params.arguments ?? {});
      } finally { if (consumed === active) active = undefined; }
    }
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
