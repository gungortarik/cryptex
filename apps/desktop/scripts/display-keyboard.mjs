import assert from "node:assert/strict";
import { mkdir, writeFile } from "node:fs/promises";
import path from "node:path";
import { chromium } from "playwright-core";
import { openBuiltDesktop } from "./desktop-view-fixture.mjs";

const mac = process.platform === "darwin";
const output = process.env.CRYPTEX_M14_DISPLAY_DIR;
const results = [];
if (output) await mkdir(output, { recursive: true });

async function expectFocus(locator) {
  await locator.page().waitForFunction(el => el === document.activeElement, await locator.elementHandle(), { timeout: 4000 });
}

const browser = await chromium.launch({ channel: "chrome" });
try {
  for (const colorScheme of ["light", "dark"]) {
    for (const density of [1, 1.5, 2]) {
      // Keep the same device-pixel budget while the logical viewport reflows.
      const viewport = { width: 960 / density, height: 720 / density };
      const page = await browser.newPage({ viewport, colorScheme, deviceScaleFactor: density });
      try {
        await openBuiltDesktop(page);
        await page.getByText("Local service ready", { exact: true }).waitFor();
        const actual = await page.evaluate(() => ({
          density: devicePixelRatio, width: innerWidth,
          background: getComputedStyle(document.body).backgroundColor,
          noOverflow: document.documentElement.scrollWidth <= document.documentElement.clientWidth + 1,
        }));
        assert.deepEqual(actual, { density, width: viewport.width, noOverflow: true,
          background: colorScheme === "dark" ? "rgb(29, 29, 28)" : "rgb(246, 245, 241)" });

        const nav = page.getByRole("navigation", { name: "Primary" });
        const forward = [
          nav.getByRole("link", { name: "Cryptex home" }),
          nav.getByRole("link", { name: "Health" }),
          nav.getByRole("link", { name: "Memory" }),
          nav.getByRole("button", { name: "Search memory" }),
          nav.getByRole("button", { name: "Open timeline" }),
        ];
        for (const control of forward) {
          await page.keyboard.press("Tab");
          await expectFocus(control);
          assert.ok(await control.evaluate(el => {
            const style = getComputedStyle(el);
            return el.matches(":focus-visible") && style.outlineStyle !== "none" && parseFloat(style.outlineWidth) >= 2;
          }), "missing visible keyboard focus");
        }
        await page.keyboard.press("Enter");
        await expectFocus(page.getByRole("heading", { name: "Timeline", exact: true }));
        await page.keyboard.press(mac ? "Meta+k" : "Control+k");
        const query = page.getByRole("textbox", { name: "Search your memory" });
        await expectFocus(query);
        await page.keyboard.type("shell fixture");
        await page.keyboard.press("Tab");
        await expectFocus(page.getByRole("button", { name: "Search", exact: true }));
        await page.keyboard.press("Shift+Tab");
        await expectFocus(query);
        await page.keyboard.press("Enter");
        await page.getByText("Fixture excerpt", { exact: true }).waitFor();
        const png = await page.screenshot({ scale: "device",
          path: output && path.join(output, colorScheme + "-" + density + "x.png") });
        assert.deepEqual([png.readUInt32BE(16), png.readUInt32BE(20)], [960, 720]);
        results.push({ colorScheme, density, viewport, keyboard: "passed" });
      } finally {
        await page.close();
      }
    }
  }
} finally {
  await browser.close();
}
if (output) await writeFile(path.join(output, "display-keyboard.json"), JSON.stringify({ platform: process.platform, results }, null, 2));
console.log("M14 display and keyboard checks passed in all six configurations");
