import assert from "node:assert/strict";
import test from "node:test";
import {
  CLASSIFICATIONS,
  MAX_CAPTURE_TEXT_BYTES,
  createCaptureSubmissionGate,
  formatCaptureOutcomeMessage,
  interpretCaptureWire,
  newCaptureId,
  shouldClearDraftAfterCommit,
  utf8ByteLength,
  validateCaptureDraft,
} from "./capture-policy.mjs";

test("classification is required with no invented default", () => {
  assert.equal(
    validateCaptureDraft({ text: "note", classification: "" }),
    "Choose a classification before saving.",
  );
  assert.equal(
    validateCaptureDraft({ text: "note", classification: "personal" }),
    "Choose a classification before saving.",
  );
  assert.deepEqual([...CLASSIFICATIONS], [
    "public",
    "internal",
    "private",
    "restricted",
  ]);
});

test("utf-8 byte limit does not silently truncate", () => {
  const text = "\u4e16\u754c".repeat(10);
  assert.equal(utf8ByteLength(text), new TextEncoder().encode(text).length);
  assert.ok(utf8ByteLength(text) > text.length);
  const oversized = "x".repeat(MAX_CAPTURE_TEXT_BYTES + 1);
  assert.match(
    validateCaptureDraft({ text: oversized, classification: "public" }) ?? "",
    /byte limit/,
  );
  const exact = "x".repeat(MAX_CAPTURE_TEXT_BYTES);
  assert.equal(validateCaptureDraft({ text: exact, classification: "internal" }), null);
});

test("submission gate ignores stale generations", () => {
  const gate = createCaptureSubmissionGate();
  const first = gate.begin();
  const second = gate.begin();
  assert.equal(gate.isCurrent(first), false);
  assert.equal(gate.isCurrent(second), true);
  gate.invalidate();
  assert.equal(gate.isCurrent(second), false);
});

test("committed clear only when live draft still matches snapshot", () => {
  const snapshot = {
    captureId: "11111111-1111-1111-1111-111111111111",
    text: "original",
    classification: "internal",
  };
  assert.equal(
    shouldClearDraftAfterCommit({
      pendingSnapshot: snapshot,
      liveText: "original",
      liveClassification: "internal",
      responseCaptureId: snapshot.captureId,
    }),
    true,
  );
  assert.equal(
    shouldClearDraftAfterCommit({
      pendingSnapshot: snapshot,
      liveText: "edited during submit",
      liveClassification: "internal",
      responseCaptureId: snapshot.captureId,
    }),
    false,
  );
});

test("receipt missing and conflict messaging retain draft semantics", () => {
  assert.match(
    formatCaptureOutcomeMessage({
      outcome: "uncertain",
      uncertainReason: "receipt_missing",
    }),
    /new capture id/,
  );
  assert.match(
    formatCaptureOutcomeMessage({
      outcome: "rejected",
      rejectCode: "capture_id_conflict",
    }),
    /not proof/,
  );
  assert.match(
    formatCaptureOutcomeMessage({ outcome: "committed" }),
    /not claimed updated/,
  );
  assert.match(
    formatCaptureOutcomeMessage({ outcome: "uncertain" }),
    /Resolve/,
  );
  const missing = interpretCaptureWire({
    outcome: "uncertain",
    uncertainReason: "receipt_missing",
  });
  assert.equal(missing.isReceiptMissing, true);
  assert.equal(missing.isCommitted, false);
});

test("capture id generation is uuid-shaped", () => {
  const id = newCaptureId(() => "aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee");
  assert.equal(id, "aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee");
  assert.match(newCaptureId(), /^[0-9a-f-]{36}$/i);
});

test("null bytes are rejected without mutating text", () => {
  const text = "ok\u0000no";
  assert.match(validateCaptureDraft({ text, classification: "private" }) ?? "", /control/);
  assert.equal(text, "ok\u0000no");
});
