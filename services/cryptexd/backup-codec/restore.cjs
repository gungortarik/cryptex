// Offline, owner-invoked sandbox verification. The sealed Kit and credential
// arrive on stdin. A Kit inspection returns only its public bootstrap key.
"use strict";
const pgp = require("./openpgp.min.cjs");
const { unzipSync } = require("./fflate.cjs");
const { Readable, Transform } = require("node:stream");
const { pipeline } = require("node:stream/promises");
const {createHash, createPrivateKey, createPublicKey} = require("node:crypto");

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
  if (length <= 105 || length > 137 + 2 * kitLimit) throw new Error("Invalid Kit input");
  const header = await read(length);
  const password = header.toString("latin1", 32, 96);
  if (!/^[0-9a-f]{64}$/.test(password)) throw new Error("Invalid unlock file");
  const limit = Number(header.readBigUInt64BE(96));
  if (!Number.isSafeInteger(limit) || limit <= 0) throw new Error("Invalid backup length");
  const paired = header[104] === 1;
  const inspect = header[104] === 2;
  if (!paired && !inspect && header[104] !== 0) throw new Error("Unknown recovery scope");
  const kit = await pgp.decrypt({message: await authenticatedMessage(header.subarray(paired || inspect ? 137 : 105)),
    passwords: [password], format: "binary", config: {maxDecompressedMessageSize: kitLimit}});
  const members = unpackKit(kit.data);
  if (!header.subarray(0, 32).equals(members["brain-identity.sha256"])) throw new Error("Wrong Brain");
  if (paired || inspect) {
    const bootstrap = createPrivateKey({key: Buffer.from(members["bootstrap.pk8"]),format:"der",type:"pkcs8"});
    const pub = createPublicKey(bootstrap).export({format:"jwk"});
    if (pub.crv !== "Ed25519") throw new Error("Invalid bootstrap key");
    const pin = Buffer.from(pub.x, "base64url");
    if (inspect) {
      // Rust hashes the canonical, typed selection. Its fields are ASCII
      // identifiers, unsigned integers and byte arrays, with no nested objects.
      const selection = JSON.parse(Buffer.from(members["selected-backup.json"]).toString("utf8"));
      if (!Number.isSafeInteger(selection.generation) || selection.generation < 1 ||
          limit !== 1 || rest.length || !(await input.next()).done) throw new Error("Invalid checkpoint");
      const canonical = JSON.stringify(selection, Object.keys(selection).sort());
      const digest = createHash("sha256").update(canonical).digest();
      if (!header.subarray(105,137).equals(digest)) throw new Error("Wrong checkpoint");
      process.stdout.end(pin);
      return;
    }
    if (!header.subarray(105,137).equals(pin)) {
      throw new Error("Wrong first-device authority");
    }
  }
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
  const verificationKeys = paired ? await pgp.readKey({armoredKey: Buffer.from(members["writer.pub"]).toString("utf8")}) : undefined;
  const opened = await pgp.decrypt({message, decryptionKeys: key, verificationKeys, format: "binary",
    config: {maxDecompressedMessageSize: limit}});
  let bytes = 0;
  const bounded = new Transform({transform(chunk, encoding, done) {
    bytes += chunk.length;
    done(bytes > limit ? new Error("Oversized recovery archive") : null, chunk);
  }});
  await pipeline(Readable.fromWeb(opened.data), bounded, process.stdout, {end: !paired});
  if (opened.signatures.length > 1 || (!paired && opened.signatures.length)) throw new Error("Unexpected outer signature");
  if (opened.signatures.length) await opened.signatures[0].verified;
  // The trailing tag exists only in the private helper pipe for a paired
  // restore. It distinguishes a verified legacy signature from an inner inventory.
  if (paired) process.stdout.end(Buffer.from([opened.signatures.length]));
}

if (require.main === module) restore().catch(() => { process.exitCode = 1; });
module.exports = {authenticatedMessage, unpackKit};
