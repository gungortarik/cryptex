// The native client is the parent of the daemon and owns its authenticated IPC channel.
import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { createHash } from "node:crypto";
import { once } from "node:events";
import { readFile } from "node:fs/promises";
import path from "node:path";
import { createInterface } from "node:readline";
import { fileURLToPath } from "node:url";
import { chromium } from "playwright-core";

const dist = fileURLToPath(new URL("../dist/", import.meta.url));
const origin = "http://127.0.0.1:1420";
const browser = await chromium.launch({ channel: "chrome" });
const client = spawn(process.argv[2], ["--exact", "ipc::attachment_flow::browser_client", "--ignored", "--nocapture"], {
  env: { ...process.env, CRYPTEX_FILE_FIXTURE_MODE: "backup-removal" }, stdio: ["pipe", "pipe", "inherit"],
});
const exited = once(client, "exit");
const lines = createInterface({ input: client.stdout })[Symbol.asyncIterator]();
async function read(prefix) {
  for (;;) {
    const { value, done } = await lines.next();
    assert.ok(!done, "native host exited before " + prefix);
    if (value.startsWith(prefix)) return value.slice(prefix.length);
  }
}
async function native(action) {
  client.stdin.write(JSON.stringify({ command: "m11_backup_removal", args: { action } }) + "\n");
  return JSON.parse(await read("browser-reply "));
}
try {
  await read("browser-ready");
  const page = await browser.newPage();
  let current;
  await page.exposeFunction("nativeInvoke", async (command, args) => {
    if (command === "p0_3_ipc_health") return {};
    if (command !== "m11_backup_removal") throw Error("unsupported rehearsal command");
    const result = await native(args.action);
    if (!result.Ok) throw Error(result.Err);
    if (result.Ok.kind === "review") current = result.Ok;
    return result.Ok;
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
  await page.getByRole("button", { name: "Connection settings", exact: true }).click();
  const inspect = page.getByRole("button", { name: "Review backup removal" });
  const dialog = page.getByRole("dialog", { name: "Remove this entire encrypted backup?" });
  await inspect.click();
  await dialog.waitFor();
  const { ticket, selection } = current;
  for (const [file, digest] of [[selection.old_file, selection.old_digest], [selection.replacement_file, selection.replacement_digest]]) {
    await dialog.getByText(file, { exact: true }).waitFor();
    assert.equal(createHash("sha256").update(await readFile(file)).digest("hex"), digest);
    await dialog.getByText(digest, { exact: true }).waitFor();
  }
  assert.equal(await page.evaluate(() => document.activeElement.textContent), "Keep backup");
  await page.keyboard.press("Escape");
  await page.getByText("Backup kept.", { exact: true }).waitFor();
  assert.deepEqual(await native({ kind: "remove", ticket }), { Err: "expired" });
  await inspect.click();
  await dialog.waitFor();
  assert.notEqual(current.ticket, ticket);
  assert.deepEqual(current.selection, selection);
  await dialog.getByRole("button", { name: "Remove this backup" }).click();
  await page.getByText("The selected backup was removed.", { exact: true }).waitFor();
  assert.deepEqual(await native({ kind: "remove", ticket: current.ticket }), { Err: "expired" });
  await inspect.click();
  await page.getByText("No backup is awaiting removal.", { exact: true }).waitFor();
} finally {
  client.stdin.end();
  await browser.close();
  const [code] = await exited;
  assert.equal(code, 0, "native daemon verified the remaining backup and restored the surviving attachment");
}
