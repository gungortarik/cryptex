// Test harness for the separate owner tool. Never included in either package.
"use strict";
const assert = require("node:assert/strict");
const fs = require("node:fs/promises");
const {spawnSync} = require("node:child_process");
const [tool, request, kit, unlock, backup, output] = process.argv.slice(2);
const {authorize} = require(tool);
async function run() {
  const args = {request, kit, unlock, backup};
  const publicRequest = JSON.parse(await fs.readFile(request));
  const cli = spawnSync(process.execPath, [tool, request, kit, unlock, backup, output], {input: publicRequest.new_device + "\n"});
  assert.equal(cli.status, 1);
  await assert.rejects(fs.stat(output), {code: "ENOENT"});
  await assert.rejects(authorize(args, async () => false), /cancelled/);
  const neverAsk = () => assert.fail("Invalid inputs reached owner consent");
  await assert.rejects(authorize({...args, backup: request}, neverAsk), /Wrong backup/);
  const wrong = output + ".wrong-request";
  publicRequest.archive_history_sha256[0] ^= 1;
  await fs.writeFile(wrong, JSON.stringify(publicRequest), {flag: "wx"});
  await assert.rejects(authorize({...args, request: wrong}, neverAsk), /checkpoint mismatch/);
  let reviewed = false;
  const approval = await authorize(args, async (text, device) => {
    assert.equal(device, publicRequest.new_device);
    assert.ok(text.includes(device) && text.includes(publicRequest.brain_id));
    reviewed = true;
    return true;
  });
  assert.ok(reviewed);
  await fs.writeFile(output, JSON.stringify(approval), {flag: "wx"});
}
run().catch(err => { console.error(err); process.exitCode = 1; });
