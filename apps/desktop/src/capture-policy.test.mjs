import assert from "node:assert/strict";
import test from "node:test";
import {
  CLASSIFICATIONS,
  MAX_CAPTURE_TEXT_BYTES,
  NATIVE_QUIT_WARNING,
  createCaptureSubmissionGate,
  createCaptureSubmissionState,
  evaluateDeliberateNewSave,
  evaluateOrdinarySave,
  formatCaptureOutcomeMessage,
  hasArchivedUnresolved,
  hasUnresolvedSubmission,
  interpretCaptureWire,
  newCaptureId,
  redactedCaptureTransportMessage,
  reduceCaptureSubmission,
  sessionNeedsProtection,
  shouldClearDraftAfterCommit,
  shouldPromptBeforeNativeQuit,
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

test("timeout then ordinary Save is blocked and pending id is preserved", () => {
  let state = createCaptureSubmissionState();
  state = reduceCaptureSubmission(state, {
    type: "draft",
    text: "note",
    classification: "internal",
  });
  state = reduceCaptureSubmission(state, {
    type: "save",
    captureId: "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa",
  });
  state = reduceCaptureSubmission(state, {
    type: "transport_uncertain",
    generation: state.generation,
    captureId: "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa",
    message: "timed out",
  });
  assert.equal(state.ui.kind, "uncertain");
  assert.equal(state.pending.captureId, "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa");
  const blocked = evaluateOrdinarySave(state, {
    text: "note",
    classification: "internal",
  });
  assert.equal(blocked.allowed, false);
  assert.equal(blocked.code, "unresolved_blocks_save");
  const after = reduceCaptureSubmission(state, {
    type: "save",
    captureId: "bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb",
    text: "note",
    classification: "internal",
  });
  assert.equal(after.pending.captureId, "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa");
  assert.equal(after.ui.kind, "uncertain");
  assert.match(after.notice ?? "", /unresolved/i);
});

test("receipt_missing never auto-resubmits and blocks ordinary Save", () => {
  let state = createCaptureSubmissionState();
  state = reduceCaptureSubmission(state, {
    type: "draft",
    text: "note",
    classification: "private",
  });
  state = reduceCaptureSubmission(state, {
    type: "save",
    captureId: "cccccccc-cccc-4ccc-8ccc-cccccccccccc",
  });
  state = reduceCaptureSubmission(state, {
    type: "response",
    generation: state.generation,
    captureId: "cccccccc-cccc-4ccc-8ccc-cccccccccccc",
    response: { outcome: "uncertain", uncertainReason: "receipt_missing" },
  });
  assert.equal(state.ui.canResolve, false);
  const resolveBlocked = reduceCaptureSubmission(state, { type: "resolve_start" });
  assert.equal(resolveBlocked.ui.kind, "uncertain");
  assert.match(resolveBlocked.notice ?? "", /must not auto-resubmit/);
  assert.equal(evaluateOrdinarySave(resolveBlocked).allowed, false);
  assert.equal(
    evaluateDeliberateNewSave(resolveBlocked, { acknowledgeDuplication: false }).code,
    "ack_required",
  );
});

test("edits while unresolved keep pending snapshot for Resolve", () => {
  let state = createCaptureSubmissionState();
  state = reduceCaptureSubmission(state, {
    type: "draft",
    text: "original",
    classification: "internal",
  });
  state = reduceCaptureSubmission(state, {
    type: "save",
    captureId: "dddddddd-dddd-4ddd-8ddd-dddddddddddd",
  });
  state = reduceCaptureSubmission(state, {
    type: "transport_uncertain",
    generation: state.generation,
    captureId: "dddddddd-dddd-4ddd-8ddd-dddddddddddd",
  });
  state = reduceCaptureSubmission(state, {
    type: "draft",
    text: "edited locally",
    classification: "internal",
  });
  assert.equal(state.pending.text, "original");
  assert.equal(state.draftText, "edited locally");
  assert.equal(hasUnresolvedSubmission(state), true);
  assert.equal(evaluateOrdinarySave(state).allowed, false);
});

test("uncertain A then deliberate B then B committed keeps A tracked and protected", () => {
  let state = createCaptureSubmissionState();
  state = reduceCaptureSubmission(state, {
    type: "draft",
    text: "alpha",
    classification: "internal",
  });
  state = reduceCaptureSubmission(state, {
    type: "save",
    captureId: "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa",
  });
  state = reduceCaptureSubmission(state, {
    type: "transport_uncertain",
    generation: state.generation,
    captureId: "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa",
  });
  state = reduceCaptureSubmission(state, {
    type: "save_deliberate",
    acknowledgeDuplication: true,
    captureId: "bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb",
    text: "beta",
    classification: "internal",
  });
  assert.equal(state.unresolvedArchive.length, 1);
  assert.equal(state.unresolvedArchive[0].captureId, "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa");
  state = reduceCaptureSubmission(state, {
    type: "response",
    generation: state.generation,
    captureId: "bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb",
    response: {
      outcome: "committed",
      captureId: "bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb",
    },
    liveText: "beta",
    liveClassification: "internal",
  });
  assert.equal(state.ui.kind, "committed");
  assert.equal(state.pending, null);
  assert.equal(state.unresolvedArchive.length, 1);
  assert.equal(state.unresolvedArchive[0].captureId, "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa");
  assert.equal(hasArchivedUnresolved(state), true);
  assert.equal(sessionNeedsProtection(state), true);
  assert.equal(
    shouldPromptBeforeNativeQuit({
      draftDirty: false,
      hasUnresolved: hasUnresolvedSubmission(state),
      hasArchived: hasArchivedUnresolved(state),
    }),
    true,
  );
});

test("late A committed updates stored archive without clearing B draft", () => {
  let state = createCaptureSubmissionState();
  state = reduceCaptureSubmission(state, {
    type: "draft",
    text: "first",
    classification: "public",
  });
  state = reduceCaptureSubmission(state, {
    type: "save",
    captureId: "11111111-1111-4111-8111-111111111111",
  });
  const firstGeneration = state.generation;
  state = reduceCaptureSubmission(state, {
    type: "transport_uncertain",
    generation: firstGeneration,
    captureId: "11111111-1111-4111-8111-111111111111",
  });
  state = reduceCaptureSubmission(state, {
    type: "save_deliberate",
    acknowledgeDuplication: true,
    captureId: "22222222-2222-4222-8222-222222222222",
    text: "second-draft",
    classification: "public",
  });
  state = reduceCaptureSubmission(state, {
    type: "transport_uncertain",
    generation: state.generation,
    captureId: "22222222-2222-4222-8222-222222222222",
  });
  // User edits B's draft while A is archived.
  state = reduceCaptureSubmission(state, {
    type: "draft",
    text: "second-draft-edited",
    classification: "public",
  });
  assert.equal(state.unresolvedArchive.length, 1);
  const afterLate = reduceCaptureSubmission(state, {
    type: "response",
    generation: firstGeneration,
    captureId: "11111111-1111-4111-8111-111111111111",
    response: {
      outcome: "committed",
      captureId: "11111111-1111-4111-8111-111111111111",
    },
  });
  assert.equal(afterLate.pending.captureId, "22222222-2222-4222-8222-222222222222");
  assert.equal(afterLate.ui.kind, "uncertain");
  assert.equal(afterLate.draftText, "second-draft-edited");
  assert.equal(afterLate.unresolvedArchive.length, 0);
  assert.match(afterLate.notice ?? "", /later reported as saved/);
});

test("archived capture can be resolve-started without replacing pending B", () => {
  let state = createCaptureSubmissionState();
  state = reduceCaptureSubmission(state, {
    type: "draft",
    text: "a",
    classification: "private",
  });
  state = reduceCaptureSubmission(state, {
    type: "save",
    captureId: "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa",
  });
  state = reduceCaptureSubmission(state, {
    type: "transport_uncertain",
    generation: state.generation,
    captureId: "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa",
  });
  state = reduceCaptureSubmission(state, {
    type: "save_deliberate",
    acknowledgeDuplication: true,
    captureId: "bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb",
    text: "b",
    classification: "private",
  });
  state = reduceCaptureSubmission(state, {
    type: "transport_uncertain",
    generation: state.generation,
    captureId: "bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb",
  });
  const started = reduceCaptureSubmission(state, {
    type: "resolve_archive_start",
    captureId: "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa",
  });
  assert.equal(started.resolvingArchiveId, "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa");
  assert.equal(started.pending.captureId, "bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb");
  assert.equal(started.ui.kind, "resolving");
});

test("repeated Save clicks while busy are ignored", () => {
  let state = createCaptureSubmissionState();
  state = reduceCaptureSubmission(state, {
    type: "draft",
    text: "note",
    classification: "internal",
  });
  state = reduceCaptureSubmission(state, {
    type: "save",
    captureId: "eeeeeeee-eeee-4eee-8eee-eeeeeeeeeeee",
  });
  const mid = state;
  const again = reduceCaptureSubmission(mid, {
    type: "save",
    captureId: "ffffffff-ffff-4fff-8fff-ffffffffffff",
  });
  assert.equal(again.pending.captureId, "eeeeeeee-eeee-4eee-8eee-eeeeeeeeeeee");
  assert.equal(again.ui.kind, "submitting");
  assert.equal(again.generation, mid.generation);
});

test("rejected validation while uncertain does not erase prior uncertain outcome", () => {
  let state = createCaptureSubmissionState();
  state = reduceCaptureSubmission(state, {
    type: "draft",
    text: "kept",
    classification: "internal",
  });
  state = reduceCaptureSubmission(state, {
    type: "save",
    captureId: "99999999-9999-4999-8999-999999999999",
  });
  state = reduceCaptureSubmission(state, {
    type: "transport_uncertain",
    generation: state.generation,
    captureId: "99999999-9999-4999-8999-999999999999",
  });
  const rejected = reduceCaptureSubmission(state, {
    type: "save",
    captureId: "88888888-8888-4888-8888-888888888888",
    text: "",
    classification: "",
  });
  assert.equal(rejected.ui.kind, "uncertain");
  assert.equal(rejected.pending.captureId, "99999999-9999-4999-8999-999999999999");
  assert.ok(rejected.notice);
});

test("native quit prompt policy and redacted transport copy", () => {
  assert.equal(
    shouldPromptBeforeNativeQuit({ draftDirty: false, hasUnresolved: false, hasArchived: false }),
    false,
  );
  assert.equal(
    shouldPromptBeforeNativeQuit({ draftDirty: true, hasUnresolved: false, hasArchived: false }),
    true,
  );
  assert.equal(
    shouldPromptBeforeNativeQuit({ draftDirty: false, hasUnresolved: true, hasArchived: false }),
    true,
  );
  assert.equal(
    shouldPromptBeforeNativeQuit({ draftDirty: false, hasUnresolved: false, hasArchived: true }),
    true,
  );
  assert.match(NATIVE_QUIT_WARNING, /Cancel keeps Cryptex open/);
  assert.match(
    redactedCaptureTransportMessage("capture_transport_timeout"),
    /not cancelled/,
  );
  assert.equal(
    redactedCaptureTransportMessage("capture_transport_unavailable").includes("C:\\"),
    false,
  );
});
