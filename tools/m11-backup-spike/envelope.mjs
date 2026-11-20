// Disposable P1 runner. The writer receives only the recovery public key.
import { readFile, writeFile } from "node:fs/promises";
import { Readable } from "node:stream";
import { pipeline } from "node:stream/promises";
import * as pgp from "openpgp";

const config = { v6Keys: true, aeadProtect: true };
const [operation, keyFile, secretFile] = process.argv.slice(2);

async function run() {
  if (operation === "keygen") {
    const keys = await pgp.generateKey({
      type: "curve25519", userIDs: [{ name: "Disposable recovery recipient" }], config,
    });
    await writeFile(secretFile, keys.privateKey, { flag: "wx", mode: 0o600 });
    await writeFile(keyFile, keys.publicKey, { flag: "wx" });
    return;
  }
  const armoredKey = await readFile(keyFile, "utf8");
  const input = Readable.toWeb(process.stdin);
  let output;
  if (operation === "encrypt") {
    output = await pgp.encrypt({
      message: await pgp.createMessage({ binary: input }),
      encryptionKeys: await pgp.readKey({ armoredKey }), format: "binary", config,
    });
  } else if (operation === "decrypt") {
    const message = await pgp.readMessage({ binaryMessage: input });
    const packets = message.packets.filterByTag(pgp.enums.packet.symEncryptedIntegrityProtectedData);
    if (packets.length !== 1 || packets[0].version !== 2) {
      throw new Error("Expected one RFC 9580 authenticated encrypted payload");
    }
    const result = await pgp.decrypt({
      message, decryptionKeys: await pgp.readPrivateKey({ armoredKey }), format: "binary", config,
    });
    output = result.data;
  } else {
    throw new Error("Unknown operation");
  }
  await pipeline(Readable.fromWeb(output), process.stdout);
}

try {
  await run();
} catch {
  process.stderr.write("M11 envelope operation failed\n");
  process.exitCode = 1;
}
