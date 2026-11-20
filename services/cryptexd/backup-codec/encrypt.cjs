// Input is the public certificate length (uint32 BE), its UTF-8 armor, then
// the daemon's completed archive. Only the encrypted message goes to stdout.
"use strict";
const pgp = require("./openpgp.min.cjs");
const { Readable } = require("node:stream");
const { pipeline } = require("node:stream/promises");

async function encrypt() {
  const input = process.stdin[Symbol.asyncIterator]();
  let remainder = Buffer.alloc(0);
  async function readExactly(length) {
    const bytes = Buffer.alloc(length);
    let offset = 0;
    while (offset < length) {
      if (!remainder.length) {
        const {done, value} = await input.next();
        if (done) throw new Error("Incomplete input");
        remainder = value;
      }
      const copied = remainder.copy(bytes, offset, 0, length - offset);
      offset += copied;
      remainder = remainder.subarray(copied);
    }
    return bytes;
  }
  const size = (await readExactly(4)).readUInt32BE();
  if (!size || size > 64 * 1024) throw new Error("Invalid certificate length");
  const key = await pgp.readKey({armoredKey: (await readExactly(size)).toString("utf8")});
  if (key.isPrivate()) throw new Error("Public certificate required");
  async function* archive() {
    if (remainder.length) yield remainder;
    yield* input;
  }
  const stream = Readable.toWeb(Readable.from(archive(), {objectMode: false}));
  const message = await pgp.createMessage({binary: stream});
  const encrypted = await pgp.encrypt({message, encryptionKeys: key, format: "binary",
    config: {v6Keys: true, aeadProtect: true}});
  await pipeline(Readable.fromWeb(encrypted), process.stdout);
}

encrypt().catch(() => {
  process.stderr.write("Backup encryption failed\n");
  process.exitCode = 1;
});
