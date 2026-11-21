"use strict";
const {createPrivateKey, createPublicKey, randomBytes, sign, verify} = require("node:crypto");

async function verifiedRecipient(pgp, secret, advertised, bootstrapDer) {
  const config = {v6Keys: true, aeadProtect: true};
  const privateKey = await pgp.readPrivateKey({armoredKey: secret});
  const publicKey = privateKey.toPublic();
  const supplied = await pgp.readKeys({armoredKeys: advertised});
  if (supplied.length !== 1 || supplied[0].isPrivate() ||
      !Buffer.from(publicKey.write()).equals(Buffer.from(supplied[0].write()))) {
    throw new Error("Recovery certificate mismatch");
  }
  // Exercise the encryption subkey, including whether the secret exists
  // and is usable. The public certificate alone cannot establish that.
  const bootstrap = createPrivateKey({key: bootstrapDer, format: "der", type: "pkcs8"});
  if (bootstrap.asymmetricKeyType !== "ed25519") throw new Error("Invalid bootstrap key");
  const challenge = randomBytes(32);
  if (!verify(null, challenge, createPublicKey(bootstrap), sign(null, challenge, bootstrap))) {
    throw new Error("Recovery bootstrap check failed");
  }
  const cipher = await pgp.encrypt({message: await pgp.createMessage({binary: challenge}),
    encryptionKeys: publicKey, format: "binary", config});
  const opened = await pgp.decrypt({message: await pgp.readMessage({binaryMessage: cipher}),
    decryptionKeys: privateKey, format: "binary", config});
  if (!Buffer.from(opened.data).equals(challenge)) throw new Error("Recovery check failed");
  return publicKey.armor();
}

module.exports = {verifiedRecipient};
