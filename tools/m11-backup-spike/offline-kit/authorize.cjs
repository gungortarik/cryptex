// Owner-invoked, offline Recovery Kit tool. Packaged separately from Cryptex.
"use strict";
const fs = require("node:fs");
const path = require("node:path");
const {createHash, createPrivateKey, createPublicKey, sign} = require("node:crypto");
const {createInterface} = require("node:readline/promises");
const {readSmall, withKit} = require("./open-kit.cjs");

const purpose = "cryptex.m11.trial.authorize-first-device.v2";
const uuid = /^[0-9a-f]{8}(?:-[0-9a-f]{4}){3}-[0-9a-f]{12}$/;
function deviceId(value) {
  return typeof value === "string" && /^[A-Za-z0-9_-]{1,64}$/.test(value) &&
    !/^(?:con|prn|aux|nul|com[1-9]|lpt[1-9])$/i.test(value);
}
const sha256 = bytes => createHash("sha256").update(bytes).digest("hex");
const fields = ["archive_device", "archive_history_sha256", "archive_key", "backup_sha256",
  "brain_id", "brain_identity_sha256", "challenge", "kit_id", "new_device", "new_key", "operation"];
function hex32(bytes) {
  if (!Array.isArray(bytes) || bytes.length !== 32 ||
      !bytes.every(n => Number.isInteger(n) && n >= 0 && n < 256)) throw Error("Invalid digest or key");
  return Buffer.from(bytes).toString("hex");
}

function parseRequest(bytes) {
  const value = JSON.parse(bytes.toString("utf8"));
  // This fixed ASCII schema has only strings and byte arrays. Compare the
  // exact Rust canonical bytes, rejecting duplicate keys and extra fields.
  if (!value || Object.keys(value).length !== fields.length ||
      !fields.every(key => Object.hasOwn(value, key)) ||
      !bytes.equals(Buffer.from(JSON.stringify(value, fields))) ||
      value.operation !== purpose || !/^[0-9a-f]{64}$/.test(value.kit_id) ||
      typeof value.brain_id !== "string" || !uuid.test(value.brain_id) ||
      !deviceId(value.archive_device) || !deviceId(value.new_device) ||
      value.new_device === value.archive_device) throw Error("Unexpected request");
  for (const field of ["archive_history_sha256", "archive_key", "backup_sha256", "brain_identity_sha256", "challenge", "new_key"]) {
    hex32(value[field]);
  }
  if (hex32(value.archive_key) === hex32(value.new_key)) throw Error("Expected a fresh device key");
  return value;
}

async function authorize({request, kit, unlock, backup}, readDeviceConfirmation) {
  const bytes = await readSmall(request, 16 * 1024);
  const candidate = parseRequest(bytes);
  return withKit(kit, unlock, async members => {
    if (!members["selected-backup.json"]) throw Error("Kit has no independent archive checkpoint");
    const selected = JSON.parse(Buffer.from(members["selected-backup.json"]).toString("utf8"));
    const key = createPrivateKey({key: members["bootstrap.pk8"], type: "pkcs8", format: "der"});
    if (key.asymmetricKeyType !== "ed25519") throw Error("Unexpected recovery authority");
    const publicKey = createPublicKey(key).export({format: "jwk"});
    const identity = hex32(candidate.brain_identity_sha256);
    if (candidate.kit_id !== sha256(Buffer.from(publicKey.x, "base64url")) ||
        selected.version !== 1 || !Number.isSafeInteger(selected.generation) || selected.generation < 1 ||
        selected.brain_id !== candidate.brain_id || selected.writer_device !== candidate.archive_device ||
        hex32(selected.writer_key) !== hex32(candidate.archive_key) ||
        hex32(selected.history_sha256) !== hex32(candidate.archive_history_sha256) ||
        hex32(selected.brain_identity_sha256) !== identity ||
        Buffer.from(members["brain-identity.sha256"]).toString("hex") !== identity) throw Error("Kit checkpoint mismatch");
    const digest = createHash("sha256");
    const handle = await fs.promises.open(backup, "r");
    try {
      if (!(await handle.stat()).isFile()) throw Error("Expected a backup file");
      const buffer = Buffer.alloc(256 * 1024);
      for (;;) {
        const {bytesRead} = await handle.read(buffer);
        if (!bytesRead) break;
        digest.update(buffer.subarray(0, bytesRead));
      }
    } finally { await handle.close(); }
    if (digest.digest("hex") !== hex32(candidate.backup_sha256)) throw Error("Wrong backup");
    const fingerprint = sha256(Buffer.from(candidate.new_key));
    const review = [
      "Authorize the first device after loss of all trusted devices?",
      "Compare the replacement device ID and full fingerprint with its public request.",
      "Brain " + candidate.brain_id, "Recovery Kit ID " + candidate.kit_id,
      "Archived device " + candidate.archive_device,
      "Generation " + selected.generation, "Backup SHA-256 " + hex32(candidate.backup_sha256),
      "Archived history SHA-256 " + hex32(candidate.archive_history_sha256),
      "New device " + candidate.new_device, "New key SHA-256 " + fingerprint,
    ].join("\n");
    if (await readDeviceConfirmation(review) !== candidate.new_device) throw Error("Approval cancelled");
    return {request: candidate, signature: Array.from(sign(null, bytes, key))};
  });
}

async function main() {
  const paths = process.argv.slice(2);
  if ((paths.length !== 0 && paths.length !== 5) || !process.stdin.isTTY || !process.stdout.isTTY) {
    throw Error("Use an interactive terminal: authorize REQUEST KIT UNLOCK_FILE BACKUP NEW_APPROVAL_FILE");
  }
  if (paths.length === 0) {
    const terminal = createInterface({input: process.stdin, output: process.stdout});
    try {
      process.stdout.write("Offline first-device recovery. Disconnect from the network before using your Kit.\n");
      for (const label of ["Public challenge", "Encrypted Kit", "Kit unlock file", "Selected encrypted backup", "New approval file"]) {
        const value = await terminal.question(label + " (full path, Enter to cancel): ");
        if (!value) throw Error("Approval cancelled");
        paths.push(value);
      }
    } finally { terminal.close(); }
  }
  const [request, kit, unlock, backup, output] = paths;
  const approval = await authorize({request, kit, unlock, backup}, async review => {
    const terminal = createInterface({input: process.stdin, output: process.stdout});
    try {
      return await terminal.question(review + "\nType the full new device ID to approve, or Enter to cancel:\n");
    } finally { terminal.close(); }
  });
  const temp = await fs.promises.mkdtemp(path.join(path.dirname(output), ".cryptex-approval-"));
  const staged = path.join(temp, "approval.json");
  try {
    await fs.promises.writeFile(staged, JSON.stringify(approval) + "\n", {flag: "wx", mode: 0o600, flush: true});
    await fs.promises.link(staged, output);
  } finally {
    await fs.promises.rm(staged, {force: true});
    await fs.promises.rmdir(temp);
  }
  process.stdout.write("Signed approval saved. Return the approval file to Cryptex.\n");
}
if (require.main === module) main().catch(error => {
  const allowed = ["Approval cancelled", "Wrong backup", "Kit checkpoint mismatch",
    "Kit has no independent archive checkpoint", "Unexpected request"];
  const message = allowed.includes(error.message) || error.message.startsWith("Use an interactive terminal:") ?
    error.message : "Could not verify inputs or publish a new approval file.";
  process.stderr.write(message + "\n");
  process.exitCode = 1;
});
module.exports = {authorize};
