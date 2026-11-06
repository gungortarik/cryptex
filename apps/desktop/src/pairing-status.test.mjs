import assert from "node:assert/strict";
import test from "node:test";
import { pairingRejectionStatus } from "./pairing-status.mjs";

test("distinguishes a held trust record from a rejected transcript", () => {
  for (const code of ["invalid_transcript", "approval_anchor_held", "approval_transaction_conflict"]) {
    assert.ok(pairingRejectionStatus(code).includes(code));
  }
  assert.equal(pairingRejectionStatus("unknown daemon detail"), pairingRejectionStatus(null));
});
