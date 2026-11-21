// The offline utility publishes the public certificate and the exact Brain identity digest.
"use strict";
const fs = require("node:fs/promises");
const path = require("node:path");
const {createHash} = require("node:crypto");
const {createInterface} = require("node:readline/promises");
const pgp = require("./openpgp.min.cjs");
const {withKit} = require("./open-kit.cjs");
const {verifiedRecipient} = require("./verified-recipient.cjs");

async function exportRecipient(kit, unlock, folder) {
  const {certificate, identity} = await withKit(kit, unlock, async members => {
    if (members["brain-identity.sha256"].length !== 32) throw Error("Invalid Brain identity");
    const certificate = await verifiedRecipient(pgp,
      Buffer.from(members["recovery.sec"]).toString("ascii"),
      Buffer.from(members["recovery.pub"]).toString("ascii"), members["bootstrap.pk8"]);
    return {certificate, identity: Buffer.from(members["brain-identity.sha256"])};
  });
  // Only a new directory is eligible. Publish the certificate last so an
  // interrupted export with just the identity cannot be enrolled.
  await fs.mkdir(folder, {mode: 0o700});
  const options = {flag: "wx", mode: 0o600, flush: true};
  await fs.writeFile(path.join(folder, "brain-identity.sha256"), identity, options);
  await fs.writeFile(path.join(folder, "recovery.pub"), certificate, options);
  return createHash("sha256").update(certificate).digest("hex");
}

async function main() {
  if (!process.stdin.isTTY || !process.stdout.isTTY) throw Error("Interactive terminal required");
  const terminal = createInterface({input: process.stdin, output: process.stdout});
  try {
    process.stdout.write("Disconnect this computer before opening the Recovery Kit.\n");
    const paths = [];
    for (const label of ["Encrypted Kit", "Kit unlock file", "New folder for the public certificate"]) {
      const selected = await terminal.question(label + " (full path, Enter to cancel): ");
      if (!path.isAbsolute(selected)) throw Error("Absolute path required");
      paths.push(selected);
    }
    const digest = await exportRecipient(...paths);
    process.stdout.write("Public certificate saved. Compare this SHA-256 with Cryptex during enrollment.\n" + digest + "\n");
  } finally { terminal.close(); }
}

if (require.main === module) main().catch(() => {
  process.stderr.write("Could not verify the Kit or export to a new folder.\n");
  process.exitCode = 1;
});
module.exports = {exportRecipient};
