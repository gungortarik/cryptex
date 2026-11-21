import { randomUUID } from "node:crypto";
import { connectLocal } from "./health.mjs";

const operation = "context.public.request";
const uuid = /^[0-9a-f]{8}(?:-[0-9a-f]{4}){3}-[0-9a-f]{12}$/;

// A single bounded frame at a time. One IPC stream carries the review, decision and read.
async function* frames(socket) {
  let pending = Buffer.alloc(0);
  for await (const chunk of socket) {
    let start = 0;
    while (start < chunk.length) {
      const end = chunk.indexOf(10, start);
      const part = chunk.subarray(start, end < 0 ? chunk.length : end);
      if (pending.length + part.length > 65536) throw new Error("Invalid response");
      pending = Buffer.concat([pending, part]);
      if (end < 0) break;
      const reply = JSON.parse(pending.toString("utf8"));
      pending = Buffer.alloc(0);
      yield reply;
      start = end + 1;
    }
  }
}

export async function beginReview(endpoint) {
  const socket = await connectLocal(endpoint);
  const timer = setTimeout(() => socket.destroy(), 125_000);
  const close = () => socket.destroy();
  socket.once("close", () => clearTimeout(timer));
  const replies = frames(socket);
  const send = (op, id, extra = {}) => socket.write(JSON.stringify({
    protocol_version: 1, client_role: "mcp", capabilities: ["context.public.read"],
    request_id: id, operation: op, ...extra,
  }) + "\n");
  const id = randomUUID();
  const next = async (op, expectedId) => {
    const { value } = await replies.next();
    if (value?.protocol_version !== 1 || value.operation !== op || value.request_id !== expectedId) {
      throw new Error("Invalid response");
    }
    return value;
  };
  try {
    const pending = next(operation, id);
    send(operation, id);
    const review = await pending;
    if (review.status !== "pending" || !uuid.test(review.ticket)) throw new Error("Review unavailable");
    const decision = next(operation, id).then(reply => reply.status === "allowed" && reply.ticket === review.ticket)
      .catch(() => false);
    return {
      ticket: review.ticket, close, get closed() { return socket.destroyed; },
      async search(phrase) {
        try {
          if (!await decision) return { operation: "context.public.search", result: { Err: "unauthorized" } };
          const requestId = randomUUID();
          send("context.public.search", requestId, { phrase });
          return await next("context.public.search", requestId);
        } finally { close(); }
      },
    };
  } catch (err) {
    close();
    throw err;
  }
}
