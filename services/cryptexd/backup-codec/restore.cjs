// Offline, owner-invoked sandbox verification. The sealed Kit and credential
// arrive on stdin. Only the decrypted backup envelope can leave this process.
"use strict";
const pgp = require("./openpgp.min.cjs");
const { unzipSync } = require("./fflate.cjs");
const { Readable, Transform } = require("node:stream");
const { pipeline } = require("node:stream/promises");

const kitLimit = 128 * 1024;
const required = ["brain-identity.sha256", "recovery.sec", "recovery.pub", "writer.pub", "bootstrap.pk8"];
function unpackKit(bytes) {
  if (bytes.length > kitLimit || bytes.length < 22) throw new Error("Invalid Kit");
  const seen = new Set();
  let total = 0;
  const members = unzipSync(bytes, { filter({ name, compression, size, originalSize }) {
    total += size;
    if ((!required.includes(name) && name !== "selected-backup.json") || seen.has(name) ||
        compression !== 0 || size !== originalSize || !size || total > kitLimit) throw new Error("Invalid Kit");
    seen.add(name);
    return true;
  }});
  if (!required.every(name => seen.has(name))) throw new Error("Incomplete Kit");
  return members;
}

async function authenticatedMessage(binaryMessage) {
  const message = await pgp.readMessage({ binaryMessage });
  const encrypted = message.packets.filterByTag(pgp.enums.packet.symEncryptedIntegrityProtectedData);
  if (encrypted.length !== 1 || encrypted[0].version !== 2) throw new Error("Invalid encrypted envelope");
  return message;
}

async function restore() {
  const input = process.stdin[Symbol.asyncIterator]();
  let rest = Buffer.alloc(0);
  async function read(length) {
    const out = Buffer.alloc(length);
    let offset = 0;
    while (offset < length) {
      if (!rest.length) {
        const next = await input.next();
        if (next.done) throw new Error("Incomplete input");
        rest = next.value;
      }
      const used = rest.copy(out, offset, 0, length - offset);
      rest = rest.subarray(used);
      offset += used;
    }
    return out;
  }
  const length = (await read(4)).readUInt32BE();
  if (length <= 104 || length > 104 + 2 * kitLimit) throw new Error("Invalid Kit input");
  const header = await read(length);
  const password = header.toString("latin1", 32, 96);
  if (!/^[0-9a-f]{64}$/.test(password)) throw new Error("Invalid unlock file");
  const limit = Number(header.readBigUInt64BE(96));
  if (!Number.isSafeInteger(limit) || limit <= 0) throw new Error("Invalid backup length");
  const kit = await pgp.decrypt({message: await authenticatedMessage(header.subarray(104)),
    passwords: [password], format: "binary", config: {maxDecompressedMessageSize: kitLimit}});
  const members = unpackKit(kit.data);
  if (!header.subarray(0, 32).equals(members["brain-identity.sha256"])) throw new Error("Wrong Brain");
  const key = await pgp.readPrivateKey({armoredKey: Buffer.from(members["recovery.sec"]).toString("utf8")});
  const recipients = await pgp.readKeys({armoredKeys: Buffer.from(members["recovery.pub"]).toString("utf8")});
  if (recipients.length !== 1 || recipients[0].isPrivate() || !Buffer.from(key.toPublic().write()).equals(recipients[0].write())) {
    throw new Error("Wrong recipient");
  }
  async function* ciphertext() {
    if (rest.length) yield rest;
    yield* input;
  }
  const message = await authenticatedMessage(Readable.toWeb(Readable.from(ciphertext(), {objectMode: false})));
  const opened = await pgp.decrypt({message, decryptionKeys: key, format: "binary",
    config: {maxDecompressedMessageSize: limit}});
  let bytes = 0;
  const bounded = new Transform({transform(chunk, encoding, done) {
    bytes += chunk.length;
    done(bytes > limit ? new Error("Oversized recovery archive") : null, chunk);
  }});
  await pipeline(Readable.fromWeb(opened.data), bounded, process.stdout);
  // The manual export format uses only the daemon's signed inner inventory.
  if (opened.signatures.length) throw new Error("Unexpected outer signature");
}

restore().catch(() => { process.exitCode = 1; });
