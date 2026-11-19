import assert from "node:assert/strict";
import test from "node:test";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { chromium } from "playwright-core";

const dist = fileURLToPath(new URL("../dist/", import.meta.url));
const origin = "http://127.0.0.1:1420";

// Only the native IPC boundary is simulated. Chromium loads the built app.
function installFixture(mac) {
  Object.defineProperty(navigator, "platform", { value: mac ? "MacIntel" : "Win32" });
  const state = window.fixture = { native: "unavailable", connection: "ready_locally", error: null, loseReply: false };
  window.__TAURI_INTERNALS__ = { async invoke(command, args) {
    if (command === "p0_3_ipc_health" || command === "p0_5_set_capture_session_guard") return;
    if (command === "m8_native_profile_status") {
      if (!mac) throw "Command m8_native_profile_status not found";
      if (state.native === "unavailable") throw "channel_unavailable";
      return { schemaVersion: 1, incoming: state.native, protection: "attention",
        transportObserved: true, remoteDurabilityVerified: false };
    }
    if (command !== "m8_connection_settings") throw "unsupported";
    if (args.update) {
      if (state.loseReply) throw "reply_lost";
      state.connection = args.update.paused ? "paused" : "ready_locally";
    }
    return { revision: "7", paused: state.connection === "paused", listenAddress: "192.168.1.20",
      localTransportPort: 22000, state: state.connection, error: state.error,
      devices: state.error ? [] : [{ deviceId: "peer-a", transportId: Array(8).fill("AAAAAAA").join("-"),
        endpoint: "192.168.1.21:22001" }] };
  } };
}

for (const mac of [false, true]) {
  test(`connection screen with ${mac ? "Mac" : "Windows"} IPC responses`, async () => {
    const browser = await chromium.launch({ channel: "chrome" });
    try {
      const page = await browser.newPage({
        viewport: { width: mac ? 960 : 720, height: 700 }, colorScheme: mac ? "light" : "dark",
      });
      await page.route("**/*", async route => {
        const url = new URL(route.request().url());
        assert.equal(url.origin, origin);
        const file = path.resolve(dist, "." + (url.pathname === "/" ? "/index.html" : url.pathname));
        assert.ok(file.startsWith(path.resolve(dist) + path.sep));
        await route.fulfill({ path: file });
      });
      await page.addInitScript(installFixture, mac);
      await page.goto(origin);
      await page.getByText("Local service ready", { exact: true }).waitFor();
      await page.getByRole("button", { name: "Connection settings", exact: true }).click();
      const panel = page.locator(".connections-panel");
      const pause = panel.getByLabel("Pause sync");
      const save = panel.getByRole("button", { name: "Save connection settings" });
      await panel.getByText(/Connection ready on this device.*has not been verified/).waitFor();
      if (mac) {
        const banner = page.getByLabel("Device status");
        await banner.getByText(/unavailable/).waitFor();
        await page.evaluate(() => { window.fixture.native = "held"; });
        await banner.getByText(/paused for verification/).waitFor();
      } else {
        assert.equal(await page.getByLabel("Device status").count(), 0);
      }
      await pause.check();
      await panel.getByText("Unsaved connection settings.").waitFor();
      await save.click();
      await panel.getByText(/Settings saved. Sync paused/).waitFor();
      await pause.uncheck();
      await page.evaluate(() => { window.fixture.loseReply = true; });
      await save.click();
      await panel.getByText(/result is uncertain/).waitFor();
      assert.equal(await save.isEnabled(), false);
      assert.doesNotMatch(await panel.innerText(), /Connection ready|Sync paused\./);
      await page.evaluate(() => {
        window.fixture.connection = "attention";
        window.fixture.error = "local_restricted";
      });
      await panel.getByRole("button", { name: "Reload current settings" }).click();
      await panel.getByText(/This device is retired or restricted/).waitFor();
      assert.equal(await pause.isEnabled(), false);
      assert.doesNotMatch(await panel.innerText(), /Complete a signed pairing/);
      assert.ok(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth));
    } finally {
      await browser.close();
    }
  });
}
