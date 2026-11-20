// The parent rehearsal owns this disposable profile and both signed histories.
import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { once } from "node:events";
import { readFile, writeFile } from "node:fs/promises";
import path from "node:path";
import { createInterface } from "node:readline";
import { fileURLToPath } from "node:url";
import { chromium } from "playwright-core";

const work = process.env.CRYPTEX_M11_REOPEN_FIXTURE;
const archiveId = process.env.CRYPTEX_M11_ARCHIVE_NOTE;
const { replacement_note_id: replacementId } = JSON.parse(await readFile(path.join(work, "public-test-context.json")));
assert.ok(archiveId && replacementId && archiveId !== replacementId);
const receipt = path.join(work, "first-device.json");
const dist = fileURLToPath(new URL("../dist/", import.meta.url));
const origin = "http://127.0.0.1:1420";
const nativeTest = "knowledge_materialization::pairing_recovery_tests::m11_backup_read::restored_note_and_attachment_pass_guarded_reads";

async function session(checkDamagedReceipt) {
  const browser = await chromium.launch({ channel: "chrome" });
  const native = spawn(process.argv[2], ["--exact", nativeTest, "--ignored", "--nocapture"], { stdio: ["pipe", "pipe", "inherit"] });
  const exited = once(native, "exit");
  const lines = createInterface({ input: native.stdout })[Symbol.asyncIterator]();
  async function read(prefix) {
    for (;;) {
      const { done, value } = await lines.next();
      assert.ok(!done, "native host exited before " + prefix);
      if (value.startsWith(prefix)) return value.slice(prefix.length);
    }
  }
  try {
    await read("desktop-search-ready");
    const page = await browser.newPage();
    let response;
    await page.exposeFunction("nativeInvoke", async (command, args) => {
      if (command !== "p0_4_guarded_search") throw Error("unsupported rehearsal command");
      assert.deepEqual(args, { query: { text: "prefix OR recovered", limit: 20 } });
      native.stdin.write(JSON.stringify(args.query) + "\n");
      response = JSON.parse(await read("desktop-search-reply "));
      assert.equal(response.operation, "content.search.result");
      assert.equal(response.accepted, true);
      return response;
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
    const search = page.getByRole("button", { name: "Search", exact: true });
    await page.getByLabel("Search your memory").fill("prefix OR recovered");
    await search.click();
    const archived = page.getByText("synthetic original prefix", { exact: true });
    const replacement = page.getByText("First recovered device note", { exact: true });
    await replacement.waitFor();
    await archived.first().waitFor();
    assert.ok(response.hits.some(hit => hit.eventId === archiveId && hit.text === "synthetic original prefix"));
    assert.ok(response.hits.some(hit => hit.eventId === replacementId && hit.text === "First recovered device note"));
    if (checkDamagedReceipt) {
      const original = await readFile(receipt);
      const damaged = JSON.parse(original);
      damaged.signature[0] ^= 1;
      try {
        await writeFile(receipt, JSON.stringify(damaged));
        await search.click();
        await page.getByText("Retrieval paused", { exact: true }).waitFor();
        assert.equal(response.holdReason, "canonical_verification_unavailable");
        assert.equal(response.hits, null);
        assert.equal(await archived.count(), 0);
        assert.equal(await replacement.count(), 0);
      } finally {
        await writeFile(receipt, original);
      }
    }
  } finally {
    native.stdin.end();
    await browser.close();
    const [code] = await exited;
    assert.equal(code, 0, "native search host exited cleanly");
  }
}

await session(false);
await session(true);
