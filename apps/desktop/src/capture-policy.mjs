export const CLASSIFICATIONS = Object.freeze([
  "public",
  "internal",
  "private",
  "restricted",
]);

/** Raw capture text limit (UTF-8 bytes). Independent of serialized IPC frame size. */
export const MAX_CAPTURE_TEXT_BYTES = 524288;

export function createCaptureSubmissionGate() {
  let generation = 0;
  return {
    begin() {
      generation += 1;
      return generation;
    },
    invalidate() {
      generation += 1;
    },
    isCurrent(requestGeneration) {
      return requestGeneration === generation;
    },
  };
}

/** Validate draft without mutating text. Returns null when valid. */
export function validateCaptureDraft({ text, classification }) {
  if (typeof classification !== "string" || classification.length === 0) {
    return "Choose a classification before saving.";
  }
  if (!CLASSIFICATIONS.includes(classification)) {
    return "Choose a classification before saving.";
  }
  if (typeof text !== "string") {
    return "Capture text is required.";
  }
  const bytes = new TextEncoder().encode(text).length;
  if (bytes === 0) {
    return "Write something to capture before saving.";
  }
  if (bytes > MAX_CAPTURE_TEXT_BYTES) {
    return `Text exceeds the ${MAX_CAPTURE_TEXT_BYTES} byte limit.`;
  }
  if (text.includes("\u0000")) {
    return "Capture text contains unsupported control characters.";
  }
  return null;
}

export function utf8ByteLength(text) {
  return new TextEncoder().encode(String(text ?? "")).length;
}

/**
 * Whether a Committed/DuplicateCommitted result may clear the live draft.
 * Newer edits after submission must never be cleared by an older completion.
 */
export function shouldClearDraftAfterCommit({
  pendingSnapshot,
  liveText,
  liveClassification,
  responseCaptureId,
}) {
  if (!pendingSnapshot) return false;
  if (responseCaptureId && responseCaptureId !== pendingSnapshot.captureId) {
    return false;
  }
  return (
    liveText === pendingSnapshot.text &&
    liveClassification === pendingSnapshot.classification
  );
}

/**
 * User-facing copy for redacted capture wire outcomes.
 * Wire uses lowercase outcome plus reject_code / uncertain_reason.
 * Never includes body, paths, seeds or credentials.
 */
export function formatCaptureOutcomeMessage({
  outcome,
  rejectCode,
  uncertainReason,
  error,
} = {}) {
  const code = rejectCode || "";
  const reason = uncertainReason || "";
  if (outcome === "committed") {
    return "Saved to durable memory. Search is not claimed updated in this slice.";
  }
  if (outcome === "uncertain" && reason === "receipt_missing") {
    return "No durable receipt for this capture id. The draft was kept. Do not assume the earlier attempt never wrote; a deliberate new submission needs a new capture id and may duplicate.";
  }
  if (outcome === "rejected" && code === "capture_id_conflict") {
    return "This capture id conflicts with a different request snapshot. The draft was kept. This is not proof that an earlier attempt never committed.";
  }
  if (outcome === "uncertain") {
    return (
      error ||
      "Capture outcome is uncertain (timeout, disconnect, or busy). The draft was kept. Use Resolve to ask the daemon."
    );
  }
  if (outcome === "rejected") {
    return error || "Capture was rejected. The draft was kept.";
  }
  return error || "Capture outcome could not be interpreted. The draft was kept.";
}

/** Normalize wire fields into UI decision flags. */
export function interpretCaptureWire(response) {
  const outcome = response?.outcome ?? null;
  const rejectCode = response?.rejectCode ?? response?.reject_code ?? null;
  const uncertainReason =
    response?.uncertainReason ?? response?.uncertain_reason ?? null;
  return {
    outcome,
    rejectCode,
    uncertainReason,
    isCommitted: outcome === "committed",
    isReceiptMissing: outcome === "uncertain" && uncertainReason === "receipt_missing",
    isCaptureIdConflict: outcome === "rejected" && rejectCode === "capture_id_conflict",
    isRejected: outcome === "rejected",
    isUncertain: outcome === "uncertain",
    message: formatCaptureOutcomeMessage({
      outcome,
      rejectCode,
      uncertainReason,
      error: response?.error,
    }),
  };
}

export function newCaptureId(randomUuid = globalThis.crypto?.randomUUID?.bind(globalThis.crypto)) {
  if (typeof randomUuid === "function") {
    return randomUuid();
  }
  const hex = Array.from({ length: 32 }, () =>
    Math.floor(Math.random() * 16).toString(16),
  ).join("");
  return `${hex.slice(0, 8)}-${hex.slice(8, 12)}-${hex.slice(12, 16)}-${hex.slice(16, 20)}-${hex.slice(20)}`;
}
