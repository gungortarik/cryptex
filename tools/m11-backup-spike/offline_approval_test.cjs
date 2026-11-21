// Test harness for the separate owner tool. Never included in either package.
"use strict";
const assert = require("node:assert/strict");
const fs = require("node:fs/promises");
const {spawnSync} = require("node:child_process");
const path = require("node:path");
const {createHash} = require("node:crypto");
const [tool, request, kit, unlock, backup, output] = process.argv.slice(2);
const {authorize} = require(tool);
async function run() {
  const args = {request, kit, unlock, backup};
  const publicRequest = JSON.parse(await fs.readFile(request));
  const publicTool = path.join(path.dirname(tool), "export-recipient.cjs");
  const {exportRecipient} = require(publicTool);
  const folder = output + ".public";
  const denied = spawnSync(process.execPath, [publicTool], {input: "\n"});
  assert.equal(denied.status, 1);
  await assert.rejects(exportRecipient(kit, request, folder));
  await assert.rejects(fs.stat(folder), {code: "ENOENT"});
  const digest = await exportRecipient(kit, unlock, folder);
  assert.deepEqual((await fs.readdir(folder)).sort(), ["brain-identity.sha256", "recovery.pub"]);
  const certificate = await fs.readFile(path.join(folder, "recovery.pub"));
  assert.equal(digest, createHash("sha256").update(certificate).digest("hex"));
  const keys = await require(path.join(path.dirname(tool), "openpgp.min.cjs")).readKeys({armoredKeys: certificate.toString("ascii")});
  assert.equal(keys.length, 1);
  assert.equal(keys[0].isPrivate(), false);
  assert.deepEqual(await fs.readFile(path.join(folder, "brain-identity.sha256")), Buffer.from(publicRequest.brain_identity_sha256));
  await assert.rejects(exportRecipient(kit, unlock, folder), {code: "EEXIST"});
  assert.deepEqual(await fs.readFile(path.join(folder, "recovery.pub")), certificate);
  const cli = spawnSync(process.execPath, [tool, request, kit, unlock, backup, output], {input: publicRequest.new_device + "\n"});
  assert.equal(cli.status, 1);
  await assert.rejects(fs.stat(output), {code: "ENOENT"});
  for (const answer of ["", true, publicRequest.archive_device, publicRequest.new_device + " "]) {
    await assert.rejects(authorize(args, async () => answer), /cancelled/);
  }
  const neverAsk = () => assert.fail("Invalid inputs reached owner consent");
  await assert.rejects(authorize({...args, backup: request}, neverAsk), /Wrong backup/);
  const wrong = output + ".wrong-request";
  publicRequest.archive_history_sha256[0] ^= 1;
  await fs.writeFile(wrong, JSON.stringify(publicRequest), {flag: "wx"});
  await assert.rejects(authorize({...args, request: wrong}, neverAsk), /checkpoint mismatch/);
  let reviewed = false;
  const approval = await authorize(args, async text => {
    assert.ok(text.includes("New device " + publicRequest.new_device) && text.includes(publicRequest.brain_id));
    assert.ok(text.includes("Recovery Kit ID " + publicRequest.kit_id));
    assert.ok(text.includes("Backup SHA-256 " + Buffer.from(publicRequest.backup_sha256).toString("hex")));
    const fingerprint = require("node:crypto").createHash("sha256").update(Buffer.from(publicRequest.new_key)).digest("hex");
    assert.ok(text.includes("New key SHA-256 " + fingerprint));
    reviewed = true;
    return publicRequest.new_device;
  });
  assert.ok(reviewed);
  await fs.writeFile(output, JSON.stringify(approval), {flag: "wx"});
}
run().catch(err => { console.error(err); process.exitCode = 1; });
