// Disposable P1 runner. The writer receives only the recovery public key.
import { readFile, writeFile } from "node:fs/promises";
import { Readable } from "node:stream";
import { pipeline } from "node:stream/promises";
import * as pgp from "openpgp";

const config = { v6Keys: true, aeadProtect: true };
const [operation, keyFile, secondKeyFile] = process.argv.slice(2);

async function run() {
  if (operation === "keygen") {
    const keys = await pgp.generateKey({
      type: "curve25519", userIDs: [{ name: "Disposable recovery recipient" }], config,
    });
    await writeFile(secondKeyFile, keys.privateKey, { flag: "wx", mode: 0o600 });
    await writeFile(keyFile, keys.publicKey, { flag: "wx" });
    return;
  }
  if (operation === "lock-kit" || operation === "unlock-kit") {
    // Test fixture models a separately held, random owner unlock credential.
    const password = (await readFile(keyFile, "utf8")).trim();
    if (!/^[0-9a-f]{64}$/.test(password)) throw new Error("Invalid fixture credential");
    const data = new Uint8Array(await readBoundedKitInput());
    if (operation === "lock-kit") {
      const output = await pgp.encrypt({
        message: await pgp.createMessage({binary: data}), passwords: [password], format: "binary", config,
      });
      await pipeline(Readable.from([output]), process.stdout);
    } else {
      const message = await pgp.readMessage({binaryMessage: data});
      const encrypted = message.packets.filterByTag(pgp.enums.packet.symEncryptedIntegrityProtectedData);
      if (encrypted.length !== 1 || encrypted[0].version !== 2) throw new Error("Invalid kit envelope");
      const {data: plaintext} = await pgp.decrypt({message, passwords: [password], format: "binary",
        config: {...config, maxDecompressedMessageSize: 128 * 1024}});
      await pipeline(Readable.from([plaintext]), process.stdout);
    }
    return;
  }
  const armoredKey = await readFile(keyFile, "utf8");
  const input = Readable.toWeb(process.stdin);
  let output;
  let signatures;
  if (operation === "encrypt" || operation === "sign-encrypt") {
    const signingKeys = operation === "sign-encrypt"
      ? await pgp.readPrivateKey({ armoredKey: await readFile(secondKeyFile, "utf8") })
      : undefined;
    output = await pgp.encrypt({
      message: await pgp.createMessage({ binary: input }),
      encryptionKeys: await pgp.readKey({ armoredKey }), signingKeys, format: "binary", config,
    });
  } else if (operation === "decrypt" || operation === "decrypt-verify") {
    const message = await pgp.readMessage({ binaryMessage: input });
    const packets = message.packets.filterByTag(pgp.enums.packet.symEncryptedIntegrityProtectedData);
    if (packets.length !== 1 || packets[0].version !== 2) {
      throw new Error("Expected one RFC 9580 authenticated encrypted payload");
    }
    const verificationKeys = operation === "decrypt-verify"
      ? await pgp.readKey({ armoredKey: await readFile(secondKeyFile, "utf8") })
      : undefined;
    const result = await pgp.decrypt({
      message, decryptionKeys: await pgp.readPrivateKey({ armoredKey }), verificationKeys,
      expectSigned: Boolean(verificationKeys), format: "binary", config,
    });
    output = result.data;
    if (verificationKeys) signatures = result.signatures;
  } else {
    throw new Error("Unknown operation");
  }
  await pipeline(Readable.fromWeb(output), process.stdout);
  if (signatures) {
    if (signatures.length !== 1) throw new Error("Expected one backup writer signature");
    await signatures[0].verified;
  }
}

async function readBoundedKitInput() {
  const chunks = [];
  let size = 0;
  for await (const chunk of process.stdin) {
    size += chunk.length;
    if (size > 256 * 1024) throw new Error("Oversized kit");
    chunks.push(chunk);
  }
  return Buffer.concat(chunks, size);
}

try {
  await run();
} catch {
  process.stderr.write("M11 envelope operation failed\n");
  process.exitCode = 1;
}
