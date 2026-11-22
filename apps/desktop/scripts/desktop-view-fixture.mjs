import assert from "node:assert/strict";
import path from "node:path";
import { fileURLToPath } from "node:url";

const dist = fileURLToPath(new URL("../dist/", import.meta.url));
const origin = "http://127.0.0.1:1420";

// Only the native IPC boundary is simulated. Chromium loads the built app.
function installFixture(mac) {
  if (typeof mac === "boolean") {
    Object.defineProperty(navigator, "platform", { value: mac ? "MacIntel" : "Win32" });
  } else {
    mac = navigator.platform.toLowerCase().includes("mac");
  }
  const state = window.fixture = { native: "unavailable", connection: "ready_locally", error: null, loseReply: false };
  window.__TAURI_INTERNALS__ = { async invoke(command, args) {
    if (command === "p0_3_ipc_health") {
      if (state.serviceDown) throw "unavailable";
      return;
    }
    if (command === "p0_5_set_capture_session_guard") return;
    if (command === "p0_5_guarded_timeline") {
      return { hits: [], holdReason: null, hasMore: false, nextCursor: null };
    }
    if (command === "p0_4_guarded_search") {
      const hits = args.query.text === "shell fixture" ? [{ eventId: "fixture-note", text: "Fixture excerpt",
        classification: "public", createdAt: "2026-01-01T12:00:00Z" }] : [];
      return { hits, holdReason: null, hasMore: false, nextCursor: null };
    }
    if (command === "p0_5_guarded_event_detail") {
      return { hits: state.hideNote ? [] : [{ eventId: args.eventId, text: "Full fixture note",
        classification: "public", createdAt: "2026-01-01T12:00:00Z" }], holdReason: null };
    }
    if (command === "m8_native_profile_status") {
      if (!mac) throw "Command m8_native_profile_status not found";
      if (state.native === "unavailable") throw "channel_unavailable";
      return { schemaVersion: 1, incoming: state.native, protection: "attention",
        transportObserved: true, remoteDurabilityVerified: false };
    }
    if (command !== "m8_connection_settings") throw "unsupported";
    if (typeof args.maintenance === "boolean") state.maintenance = args.maintenance;
    if (args.update) {
      if (state.loseReply) throw "reply_lost";
      state.connection = args.update.paused ? "paused" : "ready_locally";
    }
    return { revision: "7", paused: state.connection === "paused", listenAddress: "192.168.1.20",
      localTransportPort: 22000, state: state.maintenance ? "maintenance" : state.connection, error: state.error,
      devices: state.error ? [] : [{ deviceId: "peer-a", transportId: Array(8).fill("AAAAAAA").join("-"),
        endpoint: "192.168.1.21:22001" }] };
  } };
}

export async function openBuiltDesktop(page, mac) {
  await page.route("**/*", async route => {
    const url = new URL(route.request().url());
    assert.equal(url.origin, origin);
    const file = path.resolve(dist, "." + (url.pathname === "/" ? "/index.html" : url.pathname));
    assert.ok(file.startsWith(path.resolve(dist) + path.sep));
    await route.fulfill({ path: file });
  });
  await page.addInitScript(installFixture, mac);
  await page.goto(origin);
}
