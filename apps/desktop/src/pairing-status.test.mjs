import assert from "node:assert/strict";
import test from "node:test";
import { completionMessages, pairingRejectionStatus, reviewMessages, swappedPairingField } from "./pairing-status.mjs";

test("initiator review takes the offer and proof, preserving signed text", () => {
  const offer = '{"message_type":"offer", "payload":{}}';
  assert.deepEqual(reviewMessages("initiator", offer + "\r\n", "proof", "unused"), [offer, "proof"]);
  assert.deepEqual(reviewMessages("initiator", offer, " ", ""), []);
});

test("responder review requires a separate offer, proof and initiator approval in order", () => {
  assert.deepEqual(reviewMessages("responder", "offer", "proof", "approval"), ["offer", "proof", "approval"]);
  assert.deepEqual(reviewMessages("responder", "offer", "proof", ""), []);
  assert.deepEqual(reviewMessages("responder", "offer\nproof", "proof", "approval"), []);
});

test("identifies swapped offer and Brain identity without treating partial input as verified", () => {
  const offer = '{"message_type":"offer","brain_id":"example"}';
  const identity = '{"schema_version":1,"brain_id":"example"}';
  assert.match(swappedPairingField(offer, "identity"), /field above/);
  assert.match(swappedPairingField(identity, "offer"), /field below/);
  assert.equal(swappedPairingField(offer, "offer"), "");
  assert.equal(swappedPairingField(identity, "identity"), "");
  assert.equal(swappedPairingField("{", "identity"), "");
  assert.equal(swappedPairingField("null", "offer"), "");
});

test("resuming requires four complete lines and preserves their order", () => {
  assert.deepEqual(completionMessages("offer\r\nproof\r\na1\r\na2\r\n"), ["offer", "proof", "a1", "a2"]);
  assert.deepEqual(completionMessages("offer\nproof\na1"), []);
});

test("distinguishes a held trust record from a rejected transcript", () => {
  for (const code of ["invalid_transcript", "approval_anchor_held", "approval_transaction_conflict"]) {
    assert.ok(pairingRejectionStatus(code).includes(code));
  }
  assert.equal(pairingRejectionStatus("unknown daemon detail"), pairingRejectionStatus(null));
});
