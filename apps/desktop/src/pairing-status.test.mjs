import assert from "node:assert/strict";
import test from "node:test";
import { completionMessages, pairingRejectionStatus } from "./pairing-status.mjs";

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
