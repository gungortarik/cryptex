import assert from "node:assert/strict";
import test from "node:test";
import path from "node:path";
import { mkdir } from "node:fs/promises";
import { openBuiltDesktop } from "./desktop-view-fixture.mjs";
import { chromium } from "playwright-core";

for (const mac of [false, true]) {
  test(`connection screen with ${mac ? "Mac" : "Windows"} IPC responses`, async () => {
    const browser = await chromium.launch({ channel: "chrome" });
    try {
      const page = await browser.newPage({
        viewport: { width: mac ? 960 : 720, height: 700 }, colorScheme: mac ? "light" : "dark",
      });
      await openBuiltDesktop(page, mac);
      await page.getByText("Local service ready", { exact: true }).waitFor();
      const health = page.getByRole("region", { name: "Device health" });
      const nav = page.getByRole("navigation", { name: "Primary" });
      await nav.getByRole("link", { name: "AI access" }).click();
      assert.equal(await page.getByRole("region", { name: "AI access" }).getByRole("button", { name: "Review AI access" }).isVisible(), true);
      await nav.getByRole("link", { name: "Doctor" }).click();
      const doctor = page.getByRole("region", { name: "Doctor" });
      await doctor.getByText("The local service answered the health check.").waitFor();
      await health.getByText("Ready", { exact: true }).waitFor();
      await health.getByText("Ready on this device", { exact: true }).waitFor();
      await health.getByText("Configured peers", { exact: true }).waitFor();
      await health.getByText("Unverified", { exact: true }).waitFor();
      if (process.env.CRYPTEX_M13_RENDER_DIR) {
        const directory = path.resolve(process.env.CRYPTEX_M13_RENDER_DIR);
        await mkdir(directory, { recursive: true });
        await nav.getByRole("link", { name: "Cryptex home" }).click();
        await page.screenshot({ path: path.join(directory, mac ? "home-light.png" : "home-dark.png") });
      }
      await health.getByRole("link", { name: "Write a note" }).click();
      assert.equal(await page.evaluate(() => document.activeElement.id), "capture-text");
      await health.getByRole("button", { name: "Search notes" }).click();
      assert.equal(await page.evaluate(() => document.activeElement.id), "memory-search");
      const query = page.getByRole("textbox", { name: "Search your memory" });
      await query.fill("empty fixture");
      await query.press("Enter");
      await page.getByText("No matching notes for “empty fixture”.").waitFor();
      await query.fill("shell fixture");
      await query.press("Enter");
      const results = page.getByRole("region", { name: "Search results" });
      const open = results.getByRole("button", { name: "Open full note" });
      await open.click();
      const detail = results.getByRole("article", { name: "Note detail" });
      await detail.getByText("Full fixture note").waitFor();
      assert.equal(await detail.evaluate(el => el === document.activeElement), true);
      await detail.getByRole("button", { name: "Close", exact: true }).click();
      assert.equal(await open.evaluate(el => el === document.activeElement), true);
      await page.evaluate(() => { window.fixture.hideNote = true; });
      await open.click();
      await page.getByText("This note is unavailable. Refresh the search or timeline.").waitFor();
      assert.equal(await page.getByText("Fixture excerpt").count(), 0);
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
      await health.getByText("Sync paused", { exact: true }).waitFor();
      await panel.getByRole("button", { name: "Enter Maintenance Mode" }).click();
      await panel.getByText(/Maintenance Mode is on/).waitFor();
      assert.equal(await pause.isEnabled(), false);
      await health.getByText("Maintenance Mode", { exact: true }).waitFor();
      assert.equal(await page.getByLabel("Note", { exact: true }).isEnabled(), true);
      await panel.getByRole("button", { name: "Exit Maintenance Mode" }).click();
      await health.getByText("Sync paused", { exact: true }).waitFor();
      assert.equal(await pause.isChecked(), true);
      await pause.uncheck();
      await page.evaluate(() => { window.fixture.loseReply = true; });
      await save.click();
      await panel.getByText(/result is uncertain/).waitFor();
      await health.getByText("Unavailable", { exact: true }).waitFor();
      assert.equal(await health.getByText("Configured peers", { exact: true }).count(), 0);
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
      const draft = page.getByLabel("Note", { exact: true });
      await draft.fill("Keep this draft across a service restart");
      await page.evaluate(() => {
        window.fixture.serviceDown = true;
      });
      await doctor.getByRole("button", { name: "Check local service" }).click();
      await page.getByText("Local service needs attention", { exact: true }).waitFor();
      await doctor.getByText("The local service did not complete the health check.").waitFor();
      await health.getByText("Needs attention", { exact: true }).waitFor();
      assert.equal(await health.getByText("Ready", { exact: true }).count(), 0);
      assert.equal(await panel.getByText("Connection settings are unavailable until the local service responds.").isVisible(), true);
      await page.evaluate(() => {
        window.fixture.serviceDown = false;
        window.dispatchEvent(new Event("focus"));
      });
      await page.getByText("Local service ready", { exact: true }).waitFor();
      assert.equal(await draft.inputValue(), "Keep this draft across a service restart");
      await page.setViewportSize({ width: 390, height: 700 });
      assert.ok(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth), "narrow desktop layout");
      for (const field of [draft, page.getByLabel("Classification", { exact: true }), page.getByRole("button", { name: "Search", exact: true })]) {
        const contrast = await field.evaluate(element => {
          const style = getComputedStyle(element);
          function luminance(color) {
            const [r, g, b] = color.match(/[\d.]+/g).slice(0, 3).map(Number).map(channel => {
              const value = channel / 255;
              return value <= 0.04045 ? value / 12.92 : ((value + 0.055) / 1.055) ** 2.4;
            });
            return 0.2126 * r + 0.7152 * g + 0.0722 * b;
          }
          const text = luminance(style.color), background = luminance(style.backgroundColor);
          return (Math.max(text, background) + 0.05) / (Math.min(text, background) + 0.05);
        });
        assert.ok(contrast >= 4.5, `text contrast ${contrast}`);
      }
    } finally {
      await browser.close();
    }
  });
}
