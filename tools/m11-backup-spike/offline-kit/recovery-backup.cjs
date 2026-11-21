// Standalone owner ceremony. Published Kit editions are immutable and share their original keys.
"use strict";
const fs = require("node:fs/promises");
const {constants, createReadStream} = require("node:fs");
const {createHash} = require("node:crypto");
const {execFile} = require("node:child_process");
const {createInterface} = require("node:readline/promises");
const path = require("node:path");
const os = require("node:os");
const {withKit, readSmall} = require("./open-kit.cjs");
const {zipSync} = require("./fflate.cjs");
const pgp = require("./openpgp.min.cjs");

const options = {flag: "wx", mode: 0o600, flush: true};
async function digest(file) {
  const hash = createHash("sha256");
  for await (const block of createReadStream(file)) hash.update(block);
  return hash.digest("hex");
}

function reconstruct(current, candidate) {
  const exe = path.join(__dirname, process.platform === "win32" ? "cryptex-kit-check.exe" : "cryptex-kit-check");
  return new Promise((resolve, reject) => {
    const child = execFile(exe, [], {windowsHide: true, maxBuffer: 16384}, (error, stdout) => {
      if (error) return reject(Error("Recovery backup verification failed"));
      try { resolve(JSON.parse(stdout)); } catch (cause) { reject(cause); }
    });
    child.stdin.on("error", reject);
    child.stdin.end(JSON.stringify({current, candidate}));
  });
}

function contains(a, b) {
  const relative = path.relative(a, b);
  return !relative || (relative !== ".." && !relative.startsWith(".." + path.sep) && !path.isAbsolute(relative));
}

async function newFolder(name) {
  if (!path.isAbsolute(name) || !path.basename(name)) throw Error("Choose a new folder");
  const normalized = path.resolve(name);
  const resolved = path.join(await fs.realpath(path.dirname(normalized)), path.basename(normalized));
  try { await fs.lstat(resolved); } catch (err) {
    if (err.code === "ENOENT") return resolved;
    throw err;
  }
  throw Error("Choose a new folder");
}

async function copyChecked(source, destination, expected) {
  await fs.copyFile(source, destination, constants.COPYFILE_EXCL);
  const file = await fs.open(destination, "r+");
  try { await file.sync(); } finally { await file.close(); }
  if (await digest(destination) !== expected) throw Error("Copy verification failed");
}

async function keepBackup(args, confirm) {
  const {kit, unlock, backup, newer, selection} = args;
  if (Boolean(newer) !== Boolean(selection)) throw Error("Incomplete candidate");
  const archiveDir = await newFolder(args.archiveDir);
  const kitDir = await newFolder(args.kitDir);
  if (contains(archiveDir, kitDir) || contains(kitDir, archiveDir)) throw Error("Use separate folders");
  for (const file of [backup, newer].filter(Boolean)) {
    const parent = path.dirname(await fs.realpath(file));
    if ([archiveDir, kitDir].some(dest => contains(parent, dest))) throw Error("Choose folders outside the backup target");
  }
  if (selection) {
    const source = path.dirname(await fs.realpath(newer));
    const checkpoint = path.dirname(await fs.realpath(selection));
    if (contains(source, checkpoint) || contains(checkpoint, source)) throw Error("Choose an independent checkpoint");
  }
  const temporary = await fs.realpath(os.tmpdir());
  const stage = await fs.mkdtemp(path.join(temporary, "cryptex-offline-backup-"));
  try {
    const archives = path.join(stage, "archives");
    const editions = path.join(stage, "editions");
    await fs.mkdir(archives, {mode: 0o700});
    await fs.mkdir(editions, {mode: 0o700});
    const files = label => ({kit: path.join(editions, label + "-kit.pgp"), unlock,
      checkpoint: path.join(editions, label + ".json"), backup: path.join(archives, label + ".pgp")});
    const current = files("pinned");
    const candidate = newer ? files("approved") : null;
    await fs.writeFile(current.kit, await readSmall(kit, 256 * 1024), options);
    const [oldSelection, nextSelection] = await withKit(current.kit, unlock, async members => {
      const pinned = members["selected-backup.json"];
      if (!pinned) throw Error("Kit has no independent archive checkpoint");
      await fs.writeFile(current.checkpoint, pinned, options);
      const old = JSON.parse(Buffer.from(pinned).toString("utf8"));
      if (!candidate) return [old, null];
      const bytes = await readSmall(selection, 32768);
      const next = JSON.parse(bytes.toString("utf8"));
      if (!Number.isSafeInteger(next.generation) || next.generation <= old.generation ||
          next.brain_id !== old.brain_id || next.writer_device !== old.writer_device ||
          JSON.stringify(next.writer_key) !== JSON.stringify(old.writer_key)) throw Error("Unexpected successor");
      await fs.writeFile(candidate.checkpoint, bytes, options);
      return [old, next];
    });
    await fs.copyFile(backup, current.backup, constants.COPYFILE_EXCL);
    if (candidate) await fs.copyFile(newer, candidate.backup, constants.COPYFILE_EXCL);
    const hashes = await reconstruct(current, candidate);
    if (!Array.isArray(hashes) || hashes.length !== (candidate ? 2 : 1) ||
        !hashes.every(h => typeof h === "string" && /^[0-9a-f]{64}$/.test(h))) throw Error("Invalid verifier response");
    const review = ["Brain " + oldSelection.brain_id, "Writer " + oldSelection.writer_device,
      "Current generation " + oldSelection.generation, "Current backup SHA-256 " + hashes[0]];
    if (candidate) review.push("Proposed generation " + nextSelection.generation, "Proposed backup SHA-256 " + hashes[1]);
    const expected = candidate ? "APPROVE " + nextSelection.generation : "KEEP " + oldSelection.generation;
    if (await confirm(review.join("\n"), expected) !== expected) return false;
    if (candidate) {
      await withKit(current.kit, unlock, async (members, credential) => {
        const bytes = await readSmall(candidate.checkpoint, 32768);
        const plain = zipSync({...members, "selected-backup.json": bytes}, {level: 0});
        try {
          const cipher = await pgp.encrypt({message: await pgp.createMessage({binary: plain}),
            passwords: [credential.toString("ascii")], format: "binary", config: {aeadProtect: true, v6Keys: true}});
          await fs.writeFile(candidate.kit, cipher, options);
        } finally { plain.fill(0); }
      });
      const [verified] = await reconstruct(candidate, null);
      if (verified !== hashes[1]) throw Error("Approved archive changed");
    }
    // A new sealed Kit is published last, after checking the ciphertext copies on the chosen volume.
    await fs.mkdir(archiveDir, {mode: 0o700});
    await fs.mkdir(kitDir, {mode: 0o700});
    for (const [i, item] of [current, candidate].filter(Boolean).entries()) {
      await copyChecked(item.backup, path.join(archiveDir, path.basename(item.backup)), hashes[i]);
      for (const file of [item.checkpoint, item.kit]) {
        await copyChecked(file, path.join(kitDir, path.basename(file)), await digest(file));
      }
    }
    return true;
  } finally {
    if (path.dirname(await fs.realpath(stage)) !== temporary) throw Error("Unexpected staging directory");
    await fs.rm(stage, {recursive: true, force: true});
  }
}

async function main() {
  if (!process.stdin.isTTY || !process.stdout.isTTY) throw Error("Interactive terminal required");
  const term = createInterface({input: process.stdin, output: process.stdout});
  try {
    process.stdout.write("Disconnect this computer from the network. Keep the unlock file on separate media.\n");
    const args = {};
    const ask = async (key, prompt) => {
      const value = await term.question(prompt + " (full path, Enter to cancel): ");
      if (!path.isAbsolute(value)) throw Error("Selection cancelled");
      args[key] = value;
    };
    await ask("kit", "Encrypted Recovery Kit");
    await ask("unlock", "Separate unlock file");
    await ask("backup", "Backup currently selected by the Kit");
    const action = await term.question("Type KEEP to preserve this backup or ADVANCE to review a newer one: ");
    if (!["KEEP", "ADVANCE"].includes(action)) return;
    if (action === "ADVANCE") {
      await ask("newer", "Newer encrypted backup");
      await ask("selection", "Its checkpoint from your independent trusted folder");
    }
    await ask("archiveDir", "New folder outside the daily target for protected backup copies");
    await ask("kitDir", "New independent folder for Kit editions and checkpoints");
    const saved = await keepBackup(args, (review, text) => term.question(review + "\nType " + text + " to confirm, or Enter to cancel: "));
    process.stdout.write(saved ? "Verified recovery files saved to your selected folders.\n" : "Cancelled.\n");
  } finally { term.close(); }
}
if (require.main === module) main().catch(() => {
  process.stderr.write("Could not verify or save the selected recovery backup.\n");
  process.exitCode = 1;
});
module.exports = {keepBackup};
