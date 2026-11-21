import assert from "node:assert/strict";
import { createRequire } from "node:module";
import path from "node:path";
import { createInterface } from "node:readline";
import { fileURLToPath } from "node:url";
import { connectCodex } from "./codex-mcp-client.mjs";
import { chromium } from "playwright-core";

const entry = process.env.M12_INSTALLED_STDIO;
assert.ok(path.isAbsolute(entry));
async function connectSdk() {
  const installed = createRequire(entry);
  const { Client } = installed("@modelcontextprotocol/sdk/client/index.js");
  const { StdioClientTransport } = installed("@modelcontextprotocol/sdk/client/stdio.js");
  const client = new Client({ name: "installed-review-client", version: "1" });
  await client.connect(new StdioClientTransport({ command: process.execPath, args: [entry, "--public-notes"] }));
  return client;
}
const client = process.env.M12_CODEX_BIN
  ? await connectCodex(process.env.M12_CODEX_BIN, entry)
  : await connectSdk();
const lines = createInterface({ input: process.stdin })[Symbol.asyncIterator]();
async function native(action) {
  process.stdout.write(JSON.stringify(action) + "\n");
  const { value, done } = await lines.next();
  assert.equal(done, false);
  return JSON.parse(value);
}
const request = () => client.callTool({ name: "cryptex_request_public_read", arguments: {} });
const search = () => client.callTool({ name: "cryptex_search_public_notes", arguments: { phrase: "synthetic" } });
function denied(result) {
  assert.equal(result.isError, true);
  assert.deepEqual(result.content, [{ type: "text", text: "public_read_denied" }]);
  assert.equal(result.structuredContent, undefined);
}
const dist = fileURLToPath(new URL("../dist/", import.meta.url));
const origin = "http://127.0.0.1:1420";
const browser = await chromium.launch({ channel: "chrome" });
try {
  const page = await browser.newPage({ viewport: { width: 860, height: 680 } });
  await page.exposeFunction("nativeInvoke", async (command, args) => {
    if (command === "p0_3_ipc_health") return {};
    if (command !== "m12_public_read_review") throw Error("Unsupported fixture command");
    const reply = await native(args.action);
    if (!reply.Ok) throw Error(reply.Err);
    return reply.Ok;
  });
  await page.addInitScript(() => {
    window.__TAURI_INTERNALS__ = { invoke: (...args) => window.nativeInvoke(...args) };
  });
  await page.route("**/*", async route => {
    const url = new URL(route.request().url());
    assert.equal(url.origin, origin);
    const asset = path.resolve(dist, "." + (url.pathname === "/" ? "/index.html" : url.pathname));
    assert.ok(asset.startsWith(path.resolve(dist) + path.sep));
    await route.fulfill({ path: asset });
  });
  await page.goto(origin);
  await page.getByRole("navigation", { name: "Primary" }).getByRole("link", { name: "AI access" }).click();
  const inspect = page.getByRole("button", { name: "Review AI access" });
  const dialog = page.getByRole("dialog", { name: "Allow one public-note search?" });
  denied(await search());
  const pause = page.getByRole("button", { name: "Pause AI access for this session" });
  const resume = page.getByRole("button", { name: "Resume AI access" });
  for (const grantBeforePause of [false, true]) {
    const pending = await request();
    assert.ok(!pending.isError);
    if (grantBeforePause) {
      await inspect.click();
      await dialog.getByRole("button", { name: "Allow one search" }).click();
      await dialog.waitFor({ state: "hidden" });
    }
    await pause.click();
    await resume.waitFor();
    assert.equal(await inspect.isEnabled(), false);
    // A pause invalidates this socket even if the owner resumes before use.
    await resume.click();
    await pause.waitFor();
    denied(await search());
    await pause.click();
    await resume.waitFor();
    assert.equal((await request()).isError, true);
    await resume.click();
    await pause.waitFor();
  }
  for (const outcome of ["Deny", "Escape", "Allow one search"]) {
    const result = await request();
    assert.ok(!result.isError);
    const ticket = result.content[0].text.split(" ").at(-1);
    await inspect.click();
    await dialog.getByText(ticket, { exact: true }).waitFor();
    assert.equal(await page.evaluate(() => document.activeElement.textContent), "Deny");
    assert.ok(await dialog.evaluate(el => el.scrollWidth <= el.clientWidth));
    const denyButton = await dialog.getByRole("button", { name: "Deny", exact: true }).boundingBox();
    const allowButton = await dialog.getByRole("button", { name: "Allow one search", exact: true }).boundingBox();
    assert.ok(denyButton && allowButton && denyButton.x + denyButton.width < allowButton.x);
    await page.keyboard.press("Tab");
    assert.equal(await page.evaluate(() => document.activeElement.textContent), "Allow one search");
    await page.keyboard.press("Shift+Tab");
    assert.equal(await page.evaluate(() => document.activeElement.textContent), "Deny");
    const waiting = search();
    if (outcome === "Escape") await page.keyboard.press("Escape");
    else await dialog.getByRole("button", { name: outcome, exact: true }).click();
    await dialog.waitFor({ state: "hidden" });
    const reply = await waiting;
    if (outcome === "Allow one search") {
      assert.ok(!reply.isError);
      assert.deepEqual(reply.structuredContent.notes.map(n => n.text), ["synthetic installed public"]);
    } else denied(reply);
    denied(await search());
    assert.deepEqual(await native({ kind: "allow", ticket }), { Err: "expired" });
  }
} finally {
  await client.close();
  await browser.close();
  process.stdin.destroy();
}
