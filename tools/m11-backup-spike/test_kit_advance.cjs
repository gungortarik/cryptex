"use strict";
const assert = require("node:assert/strict");
const fs = require("node:fs/promises");
const path = require("node:path");
const [bundle, root, kit, unlock, backup, newer, selection] = process.argv.slice(2);
const {keepBackup} = require(path.join(bundle, "recovery-backup.cjs"));

async function run() {
  const folders = tag => ({archiveDir: path.join(root, tag + "-archives"), kitDir: path.join(root, tag + "-kits")});
  const args = {kit, unlock, backup, newer, selection, ...folders("change")};
  const denied = () => assert.fail("Invalid selection reached owner review");
  const absent = async file => assert.rejects(fs.stat(file), {code: "ENOENT"});
  await assert.rejects(keepBackup({...args, newer: backup}, denied));
  await assert.rejects(keepBackup({...args, backup: newer}, denied));
  await assert.rejects(keepBackup({...args, archiveDir: path.join(path.dirname(backup), "managed")}, denied));
  const untrusted = path.join(path.dirname(newer), "candidate.json");
  await fs.copyFile(selection, untrusted);
  await assert.rejects(keepBackup({...args, selection: untrusted}, denied), /independent checkpoint/);
  assert.equal(await keepBackup(args, async () => {
    const scratch = require("node:os").tmpdir();
    const stages = (await fs.readdir(scratch)).filter(entry => entry.startsWith("cryptex-offline-backup-"));
    assert.equal(stages.length, 1);
    await absent(path.join(scratch, stages[0], "editions", "approved-kit.pgp"));
    return "";
  }), false);
  await absent(args.archiveDir);
  await absent(args.kitDir);
  assert.equal(await keepBackup(args, (review, expected) => {
    assert.match(review, /Current generation 1\n/);
    assert.match(review, /Proposed generation 2\n/);
    assert.equal(expected, "APPROVE 2");
    return expected;
  }), true);
  await assert.rejects(keepBackup(args, denied));
  for (const [label, generation, cipher] of [["pinned", 1, backup], ["approved", 2, newer]]) {
    const edition = path.join(args.kitDir, label + "-kit.pgp");
    const copy = path.join(args.archiveDir, label + ".pgp");
    assert.deepEqual(await fs.readFile(copy), await fs.readFile(cipher));
    const stored = JSON.parse(await fs.readFile(path.join(args.kitDir, label + ".json")));
    assert.equal(stored.generation, generation);
    assert.equal(await keepBackup({kit: edition, unlock, backup: copy, ...folders("reopen-" + label)}, (_, confirmation) => {
      assert.equal(confirmation, "KEEP " + generation);
      return confirmation;
    }), true);
  }
  assert.deepEqual(await fs.readFile(kit), await fs.readFile(path.join(args.kitDir, "pinned-kit.pgp")));
  await assert.rejects(keepBackup({...args, ...folders("repeat"), kit: path.join(args.kitDir, "approved-kit.pgp"), backup: newer}, denied));
}
run().catch(err => { console.error(err); process.exitCode = 1; });
