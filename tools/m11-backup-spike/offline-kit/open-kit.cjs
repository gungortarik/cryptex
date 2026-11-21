"use strict";
const fs = require("node:fs");
const pgp = require("./openpgp.min.cjs");
const {authenticatedMessage, unpackKit} = require("./restore.cjs");

async function readSmall(file, max) {
  const handle = await fs.promises.open(file, "r");
  try {
    const stat = await handle.stat();
    if (!stat.isFile() || !stat.size || stat.size > max) throw Error("Invalid input size");
    const buffer = Buffer.alloc(stat.size);
    let offset = 0;
    while (offset < buffer.length) {
      const {bytesRead} = await handle.read(buffer, offset, buffer.length - offset, offset);
      if (!bytesRead) throw Error("Incomplete input");
      offset += bytesRead;
    }
    if ((await handle.stat()).size !== buffer.length) throw Error("Input changed");
    return buffer;
  } finally { await handle.close(); }
}

async function withKit(kit, unlock, use) {
  const credential = await readSmall(unlock, 64);
  let decrypted;
  let members;
  try {
    if (!/^[0-9a-f]{64}$/.test(credential.toString("latin1"))) throw Error("Invalid unlock file");
    const message = await authenticatedMessage(await readSmall(kit, 256 * 1024));
    decrypted = (await pgp.decrypt({message, passwords: [credential.toString("latin1")], format: "binary",
      config: {maxDecompressedMessageSize: 128 * 1024}})).data;
    members = unpackKit(decrypted);
    return await use(members, credential);
  } finally {
    credential.fill(0);
    decrypted?.fill(0);
    if (members) for (const data of Object.values(members)) data.fill(0);
  }
}

module.exports = {readSmall, withKit};
