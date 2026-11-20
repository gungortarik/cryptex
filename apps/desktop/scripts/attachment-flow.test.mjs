import assert from "node:assert/strict";
import { spawn, spawnSync } from "node:child_process";
import { once } from "node:events";
import { mkdtemp, rm, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import { createInterface } from "node:readline";
import test from "node:test";
import { fileURLToPath } from "node:url";
import { chromium } from "playwright-core";

const repo = fileURLToPath(new URL("../../../", import.meta.url));
const dist = path.join(repo, "apps/desktop/dist");
function buildTestBinary() {
  const args = ["test", "--locked", "-p", "cryptexd", "--lib", "--no-run", "--message-format=json"];
  if (process.platform === "darwin") args.push("--features", "m8-development-product");
  const build = spawnSync("cargo", args, { cwd: repo, encoding: "utf8", stdio: ["ignore", "pipe", "inherit"], maxBuffer: 16 * 1024 * 1024 });
  assert.equal(build.status, 0);
  const artifact = build.stdout.trim().split("\n").map(JSON.parse).find(r => r.target?.name === "cryptexd" && r.executable && r.profile.test);
  assert.ok(artifact);
  return artifact.executable;
}
const binary = process.env.CRYPTEX_ATTACHMENT_TEST_BINARY ?? buildTestBinary();

test("purge cleans independent encrypted backups and preserves their surviving notes", () => {
  const result = spawnSync(binary, ["--exact", "ipc::purge_review::physical::retained::tests::purge_independent_encrypted_backups",
    "--ignored", "--nocapture"], { encoding: "utf8" });
  assert.equal(result.status, 0, result.stderr);
  assert.match(result.stdout, /retained_encrypted_purge=verified cleaned=2 untouched=1 authenticated_restore=true/);
});

test("purge scrubs archived note versions and preserves other notes", () => {
  const result = spawnSync(binary, ["--exact", "ipc::purge_review::version_history_test::confirmed_purge_scrubs_archived_versions",
    "--ignored", "--nocapture"], { encoding: "utf8" });
  assert.equal(result.status, 0, result.stderr);
  assert.match(result.stdout, /archive_purge=verified/);
});

test("purge preserves a shared attachment until its last note is deleted", () => {
  const result = spawnSync(binary, ["--exact", "ipc::purge_review::shared_attachment_test::purge_shared_attachment",
    "--ignored", "--nocapture"], { encoding: "utf8" });
  assert.equal(result.status, 0, result.stderr);
  assert.match(result.stdout, /shared_purge=verified first_preserved=true last_removed=true control_preserved=true/);
});

test("purge removes both an unrelated and a restored note while preserving a forgotten note", () => {
  const result = spawnSync(binary, ["--exact", "ipc::purge_review::shared_attachment_test::purge_preserves_mixed_note_lifecycle_history",
    "--ignored", "--nocapture"], { encoding: "utf8" });
  assert.equal(result.status, 0, result.stdout + result.stderr);
  assert.match(result.stdout, /mixed_lifecycle_purge=verified hidden=preserved unrelated=removed restored=removed reopened=true/);
});

async function startClient(mode) {
  const proc = spawn(binary, ["--exact", "ipc::attachment_flow::browser_client", "--ignored", "--nocapture"], {
    env: { ...process.env, CRYPTEX_FILE_FIXTURE_MODE: mode }, stdio: ["pipe", "pipe", "inherit"],
  });
  const exited = once(proc, "exit");
  const lines = createInterface({ input: proc.stdout })[Symbol.asyncIterator]();
  async function read(marker) {
    for (;;) {
      const { done, value } = await lines.next();
      assert.ok(!done, "native client exited before " + marker);
      const index = value.indexOf(marker);
      if (index >= 0) return value.slice(index + marker.length);
    }
  }
  await read("browser-ready");
  return {
    async invoke(command, args) {
      proc.stdin.write(JSON.stringify({ command, args }) + "\n");
      return JSON.parse(await read("browser-reply "));
    },
    async finish() {
      proc.stdin.end();
      const [code] = await exited;
      assert.equal(code, 0, "daemon reopened and verified the temporary profile");
    },
  };
}

for (const mode of ["blocked", "allowed", "physical", "damaged-archive", "lifecycle"]) {
  const name = mode === "lifecycle" ? "Desktop forget hides a note from search and restores the selected forgotten note"
    : "note detail purge review and file attachment with disposable controls " + mode;
  test(name, { timeout: 120_000 }, async () => {
    const tmp = await mkdtemp(path.join(os.tmpdir(), "cryptex-selected-file-"));
    const file = path.join(tmp, "sample.bin");
    const bytes = [0, 255, 13, 10, 65, 128];
    const browser = await chromium.launch({ channel: "chrome" });
    try {
      const client = await startClient(mode);
      try {
        const page = await browser.newPage();
        let detail;
        let reviewTicket;
        await page.exposeFunction("nativeInvoke", async (command, args) => {
          if (command === "p0_3_ipc_health" || command === "p0_5_set_capture_session_guard") return;
          if (!["m9_attach_file", "m10_purge_review", "m10_note_lifecycle", "p0_4_guarded_search", "p0_5_guarded_timeline", "p0_5_guarded_event_detail", "p0_5_capture_memory"].includes(command)) throw Error("unsupported");
          const result = await client.invoke(command, args);
          if (command === "m10_note_lifecycle") {
            if (result.Err) throw result.Err;
            return result.Ok;
          }
          if (command === "m10_purge_review") {
            if (result.Err) throw Error(result.Err);
            if (result.Ok.kind === "prepared") reviewTicket = result.Ok.ticket;
            return result.Ok;
          }
          if (command === "m9_attach_file" && result.Err) throw Error(result.Err);
          if (command === "p0_5_guarded_event_detail") detail = result.hits?.[0];
          return result;
        });
        await page.addInitScript(() => { window.__TAURI_INTERNALS__ = { invoke: (...args) => window.nativeInvoke(...args) }; });
        const origin = "http://127.0.0.1:1420";
        await page.route("**/*", async route => {
          const url = new URL(route.request().url());
          assert.equal(url.origin, origin);
          const asset = path.resolve(dist, "." + (url.pathname === "/" ? "/index.html" : url.pathname));
          assert.ok(asset.startsWith(dist + path.sep));
          await route.fulfill({ path: asset });
        });
        await page.goto(origin);
        const physical = mode === "physical" || mode === "damaged-archive";
        const title = mode === "lifecycle" ? "synthetic forget and restore test" : physical ? "purgetrialbodyxqkz" : "Note selected for a binary attachment";
        await page.getByLabel("Note", { exact: true }).fill(title);
        await page.getByLabel("Classification", { exact: true }).selectOption("internal");
        await page.getByRole("button", { name: "Save capture" }).click();
        await page.getByText(/Saved to durable memory/).waitFor();
        if (physical) {
          await page.getByLabel("Note", { exact: true }).fill("survivor following the purged record");
          await page.getByLabel("Classification", { exact: true }).selectOption("internal");
          await page.getByRole("button", { name: "Save capture" }).click();
          await page.waitForFunction(() => document.querySelector("#capture-text")?.value === "");
          // Capture commits first. Wait for the normal asynchronous indexing worker.
          const deadline = Date.now() + 10_000;
          for (;;) {
            const timeline = await client.invoke("p0_5_guarded_timeline", { query: { limit: 20, cursor: null } });
            if (timeline.hits?.some(note => note.text === "survivor following the purged record")) break;
            assert.ok(Date.now() < deadline, "third signed note must reach the guarded timeline");
            await new Promise(resolve => setTimeout(resolve, 100));
          }
        }
        const row = page.getByRole("listitem").filter({ hasText: title });
        await page.getByRole("button", { name: "Open timeline" }).click();
        await row.getByRole("button", { name: "Open full note" }).click();
        const picker = page.getByLabel("Attach a file");
        await page.getByRole("heading", { name: "Note detail" }).waitFor();
        assert.equal(detail.text, title);
        assert.equal(detail.attachmentLinkCount, 0);
        const selected = { noteId: detail.eventId, noteHash: detail.contentHash, bytes };
        const prepare = { kind: "prepare", note_id: detail.eventId, note_hash: detail.contentHash };
        const review = page.getByRole("button", { name: "Review permanent deletion" });
        const decide = action => client.invoke("m10_purge_review", { action });
        const lifecycle = action => client.invoke("m10_note_lifecycle", { action });
        if (mode === "lifecycle") {
          const original = (await lifecycle({ kind: "inspect", note_id: selected.noteId })).Ok.target;
          assert.equal(original.state_hash, selected.noteHash);
          await page.getByRole("button", { name: "Forget note", exact: true }).click();
          await page.getByText("Note forgotten. You can restore it from Forgotten notes.", { exact: true }).waitFor();
          await page.getByRole("button", { name: "Forgotten notes" }).waitFor();
          const checkSearch = async visible => {
            await page.getByRole("button", { name: "Search memory" }).click();
            await page.getByPlaceholder("Search notes, people, or ideas").fill("synthetic");
            await page.getByRole("button", { name: "Search", exact: true }).click();
            const results = page.getByRole("region", { name: "Search results" });
            await results.getByText("synthetic IPC search fixture note", { exact: true }).waitFor();
            assert.equal(await results.getByText(title, { exact: true }).count(), visible ? 1 : 0);
          };
          await checkSearch(false);
          const hiddenDetail = await client.invoke("p0_5_guarded_event_detail", { eventId: selected.noteId });
          assert.ok(hiddenDetail.accepted && !hiddenDetail.holdReason);
          assert.deepEqual(hiddenDetail.hits, []);
          await page.reload();
          await page.getByRole("button", { name: "Open timeline" }).click();
          await page.getByRole("button", { name: "Forgotten notes" }).click();
          const forgotten = page.getByRole("region", { name: "Forgotten notes" });
          await forgotten.getByText(title, { exact: true }).waitFor();
          assert.equal(await forgotten.getByRole("article").count(), 1);
          await forgotten.getByRole("button", { name: "Restore note" }).click();
          await page.getByText("Note restored.", { exact: true }).waitFor();
          await row.waitFor();
          assert.deepEqual(await lifecycle({ kind: "forget", target: original }), { Err: "stale_selection" });
          await checkSearch(true);
          await page.getByRole("button", { name: "Open timeline" }).click();
          await page.getByRole("button", { name: "Forgotten notes" }).click();
          await page.getByText("No forgotten notes.", { exact: true }).waitFor();
          return;
        }
        if (physical) {
          const dialog = page.getByRole("dialog", { name: "Permanently delete this note?" });
          await review.click();
          await dialog.getByText(/Confirm to remove the note/).waitFor();
          await dialog.getByRole("button", { name: "Cancel", exact: true }).click();
          await page.getByText("Deletion cancelled.").waitFor();
          assert.deepEqual(await client.invoke("approval_inventory", {}), []);
          await review.click();
          await dialog.getByRole("button", { name: "Confirm permanent deletion" }).click();
          const message = mode === "damaged-archive"
            ? "Deletion approval saved. Cleanup is incomplete because an integrity check failed. Copies may remain."
            : "Local cleanup finished. Copies on offline devices or in external backups may remain.";
          await page.getByText(message, { exact: true }).waitFor();
          await row.waitFor({ state: "detached" });
          assert.equal((await client.invoke("approval_inventory", {})).length, 1);
          assert.deepEqual(await decide({ kind: "confirm", ticket: reviewTicket }), { Err: "review_expired" });
          if (mode === "damaged-archive") return; // The native process repairs the archive and verifies both survivors on reopen.
          const removed = await client.invoke("p0_5_guarded_event_detail", { eventId: selected.noteId });
          assert.equal(removed.accepted, true);
          assert.ok(!removed.holdReason);
          assert.deepEqual(removed.hits, []);
          const remaining = await client.invoke("p0_5_guarded_timeline", { query: { limit: 20 } });
          assert.deepEqual(remaining.hits.map(note => note.text).sort(),
            ["survivor following the purged record", "synthetic IPC search fixture note"]);
        } else if (mode === "blocked") {
          const target = { note_id: selected.noteId, note_hash: selected.noteHash, state_hash: selected.noteHash };
          for (const action of [{ kind: "list" }, { kind: "forget", target }, { kind: "restore", target }]) {
            assert.deepEqual(await lifecycle(action), { Err: "disabled" });
          }
          assert.equal(await page.getByRole("button", { name: "Forget note", exact: true }).count(), 0);
          assert.equal(await review.count(), 0);
          assert.deepEqual(await decide(prepare), { Err: "disabled" });
          assert.deepEqual(await client.invoke("approval_inventory", {}), []);
          assert.equal(await picker.count(), 0);
          assert.deepEqual(await client.invoke("m9_attach_file", { file: selected }), { Err: "disabled" });
        } else {
          assert.deepEqual(await client.invoke("m9_attach_file", { file: { ...selected, noteHash: "0".repeat(64) } }), { Err: "target_unavailable" });
          assert.deepEqual(await decide({ ...prepare, note_hash: "0".repeat(64) }), { Err: "target_unavailable" });
          const dialog = page.getByRole("dialog", { name: "Permanently delete this note?" });
          let firstReceipt;
          for (const choice of ["cancel", "escape", "confirm", "confirm"]) {
            await review.click();
            await dialog.getByText(title, { exact: true }).waitFor();
            assert.ok((await dialog.textContent()).includes(detail.eventId));
            const cancel = dialog.getByRole("button", { name: "Cancel", exact: true });
            assert.ok(await cancel.evaluate(button => button === document.activeElement));
            if (choice === "escape") await page.keyboard.press("Escape");
            else if (choice === "cancel") await cancel.click();
            else await dialog.getByRole("button", { name: "Confirm permanent deletion" }).click();
            await dialog.waitFor({ state: "hidden" });
            await page.getByText(choice === "confirm" ? /Permanent deletion confirmed for this test note/ : "Deletion cancelled.").waitFor();
            assert.deepEqual(await decide({ kind: "confirm", ticket: reviewTicket }), { Err: "review_expired" });
            const inventory = await client.invoke("approval_inventory", {});
            if (choice === "confirm") {
              assert.equal(inventory.length, 1);
              assert.equal(inventory[0][0], detail.eventId + ".json");
              if (firstReceipt) assert.deepEqual(inventory, firstReceipt);
              firstReceipt = inventory;
            } else {
              assert.deepEqual(inventory, []);
            }
          }
          await writeFile(file, Buffer.from(bytes));
          await picker.setInputFiles(file);
          await page.getByText(/File attached\./).waitFor();
          // Simulate a client retry after an acknowledgement was lost.
          assert.deepEqual(await client.invoke("m9_attach_file", { file: selected }), { Ok: null });
          await page.getByRole("button", { name: "Close", exact: true }).click();
          await row.getByRole("button", { name: "Open full note" }).click();
          await page.getByText("1 signed attachment link", { exact: true }).waitFor();
          assert.equal(detail.contentHash, selected.noteHash);
          await page.getByRole("button", { name: "Close", exact: true }).click();
          const other = page.getByRole("listitem").filter({ hasText: "synthetic IPC search fixture note" });
          await other.getByRole("button", { name: "Open full note" }).click();
          await page.getByRole("heading", { name: "Note detail" }).waitFor();
          assert.equal(detail.attachmentLinkCount, 0);
          const secondNoteId = detail.eventId;
          assert.notEqual(secondNoteId, selected.noteId);
          await picker.setInputFiles(file);
          await page.getByText(/File attached\./).waitFor();
          await page.getByRole("button", { name: "Close", exact: true }).click();
          await other.getByRole("button", { name: "Open full note" }).click();
          await page.getByText("1 signed attachment link", { exact: true }).waitFor();
          assert.equal(detail.eventId, secondNoteId);
        }
      } catch (error) {
        console.error(error);
        throw error;
      } finally {
        await client.finish();
      }
    } finally {
      await browser.close();
      await rm(tmp, { recursive: true, force: true });
    }
  });
}
