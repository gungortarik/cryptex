"use strict";
const assert = require("node:assert/strict");
const fs = require("node:fs/promises");
const path = require("node:path");
const [bundle, root, kit, unlock, backup, newer, selection, peerBackup, peerCheck, unknownBackup, unknownCheck, peerFingerprint] = process.argv.slice(2);
const {keepBackup} = require(path.join(bundle, "recovery-backup.cjs"));

async function run() {
  const folders = tag => ({archiveDir: path.join(root, tag + "-archives"), kitDir: path.join(root, tag + "-kits")});
  const args = {kit, unlock, backup, newer, selection, ...folders("change")};
  const rejectBeforeReview = async (request, reason) => {
    let reviewed = false;
    await assert.rejects(keepBackup(request, () => {
      reviewed = true;
      return "";
    }), reason);
    assert.equal(reviewed, false, "Invalid selection reached owner review");
  };
  const absent = async file => assert.rejects(fs.stat(file), {code: "ENOENT"});
  await rejectBeforeReview({...args, newer: backup});
  await rejectBeforeReview({...args, backup: newer});
  await rejectBeforeReview({...args, archiveDir: path.join(path.dirname(backup), "managed")});
  const untrusted = path.join(path.dirname(newer), "candidate.json");
  await fs.copyFile(selection, untrusted);
  await rejectBeforeReview({...args, selection: untrusted}, /independent checkpoint/);
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
  await rejectBeforeReview(args);
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
  await rejectBeforeReview({...args, ...folders("repeat"), kit: path.join(args.kitDir, "approved-kit.pgp"), backup: newer});
  const transfer = {...args, ...folders("new-writer"), newer: peerBackup, selection: peerCheck};
  await rejectBeforeReview({...transfer, newer: unknownBackup, selection: unknownCheck});
  assert.equal(await keepBackup(transfer, () => "APPROVE 1"), false);
  assert.equal(await keepBackup(transfer, () => "0".repeat(64)), false);
  await absent(transfer.kitDir);
  await absent(transfer.archiveDir);
  assert.equal(await keepBackup(transfer, (review, expected, changingWriter) => {
    assert.equal(changingWriter, true);
    assert.equal(expected, peerFingerprint);
    assert.ok(review.includes("New signing-key SHA-256 " + peerFingerprint));
    return peerFingerprint;
  }), true);
  const advanced = path.join(transfer.kitDir, "approved-kit.pgp");
  const advancedBackup = path.join(transfer.archiveDir, "approved.pgp");
  for (const label of ["pinned", "approved"]) {
    assert.equal(await keepBackup({
      kit: path.join(transfer.kitDir, label + "-kit.pgp"), unlock,
      backup: path.join(transfer.archiveDir, label + ".pgp"), ...folders("writer-reopen-" + label),
    }, (_, expected) => expected), true);
  }
  await rejectBeforeReview({...args, ...folders("rollback"), kit: advanced, backup: advancedBackup,
    newer: backup, selection: path.join(transfer.kitDir, "pinned.json")});

}
run().catch(err => { console.error(err); process.exitCode = 1; });
