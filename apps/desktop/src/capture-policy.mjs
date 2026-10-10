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


/** @typedef {{ captureId: string, text: string, classification: string }} CaptureSnapshot */

/**
 * Pure submission policy / state machine for Quick Capture.
 * Ordinary Save never overwrites an unresolved pending snapshot.
 */
export function createCaptureSubmissionState() {
  return {
    ui: { kind: "idle" },
    pending: null,
    unresolvedArchive: [],
    generation: 0,
    notice: null,
    draftText: "",
    classification: "",
  };
}

export function hasUnresolvedSubmission(state) {
  return Boolean(
    state.pending &&
      (state.ui.kind === "submitting" ||
        state.ui.kind === "resolving" ||
        state.ui.kind === "uncertain"),
  );
}

export function isCaptureBusy(state) {
  return state.ui.kind === "submitting" || state.ui.kind === "resolving";
}

/**
 * Decide whether ordinary Save may begin a new capture_id submission.
 * Does not mutate state.
 */
export function evaluateOrdinarySave(state, { text, classification } = {}) {
  if (isCaptureBusy(state)) {
    return { allowed: false, code: "busy", message: "A capture request is already in flight." };
  }
  if (hasUnresolvedSubmission(state)) {
    return {
      allowed: false,
      code: "unresolved_blocks_save",
      message:
        "An earlier capture is still unresolved. Use Resolve, or deliberately start a new capture after acknowledging possible duplication.",
      pendingCaptureId: state.pending.captureId,
      canResolve: state.ui.kind === "uncertain" ? Boolean(state.ui.canResolve) : true,
      receiptMissing:
        state.ui.kind === "uncertain" && state.ui.canResolve === false,
    };
  }
  const validationError = validateCaptureDraft({
    text: text ?? state.draftText,
    classification: classification ?? state.classification,
  });
  if (validationError) {
    return { allowed: false, code: "validation", message: validationError };
  }
  return { allowed: true, code: "ok" };
}

/**
 * Deliberate new submission while an unresolved pending exists.
 * Requires explicit acknowledgeDuplication; never auto-triggered by ReceiptMissing.
 */
export function evaluateDeliberateNewSave(state, { acknowledgeDuplication, text, classification } = {}) {
  if (isCaptureBusy(state)) {
    return { allowed: false, code: "busy", message: "A capture request is already in flight." };
  }
  if (!hasUnresolvedSubmission(state)) {
    return evaluateOrdinarySave(state, { text, classification });
  }
  if (!acknowledgeDuplication) {
    return {
      allowed: false,
      code: "ack_required",
      message:
        "Starting a new capture while one is unresolved requires explicit acknowledgment of possible duplication.",
      pendingCaptureId: state.pending.captureId,
    };
  }
  const validationError = validateCaptureDraft({
    text: text ?? state.draftText,
    classification: classification ?? state.classification,
  });
  if (validationError) {
    return { allowed: false, code: "validation", message: validationError };
  }
  return {
    allowed: true,
    code: "ok_with_archive",
    archivePrevious: true,
    previousCaptureId: state.pending.captureId,
  };
}

/**
 * Apply a submission-policy event. Returns next state (immutable update).
 * Events: draft, save, save_deliberate, transport_uncertain, response, resolve_start, discard
 */
export function reduceCaptureSubmission(state, event) {
  switch (event.type) {
    case "draft": {
      return {
        ...state,
        draftText: event.text ?? state.draftText,
        classification: event.classification ?? state.classification,
        notice: null,
      };
    }
    case "save": {
      const decision = evaluateOrdinarySave(state, {
        text: event.text ?? state.draftText,
        classification: event.classification ?? state.classification,
      });
      if (!decision.allowed) {
        // Validation while unresolved must not erase uncertain UI.
        if (decision.code === "validation" && hasUnresolvedSubmission(state)) {
          return { ...state, notice: decision.message };
        }
        if (decision.code === "unresolved_blocks_save" || decision.code === "busy") {
          return { ...state, notice: decision.message };
        }
        // Idle validation failure — rejected without wiping a non-existent pending.
        return {
          ...state,
          notice: decision.message,
          ui:
            state.ui.kind === "uncertain"
              ? state.ui
              : { kind: "rejected", message: decision.message, canRetryAsNew: false },
        };
      }
      const captureId = event.captureId;
      const snapshot = {
        captureId,
        text: event.text ?? state.draftText,
        classification: event.classification ?? state.classification,
      };
      return {
        ...state,
        pending: snapshot,
        generation: state.generation + 1,
        notice: null,
        ui: { kind: "submitting", captureId },
        draftText: snapshot.text,
        classification: snapshot.classification,
      };
    }
    case "save_deliberate": {
      const decision = evaluateDeliberateNewSave(state, {
        acknowledgeDuplication: event.acknowledgeDuplication,
        text: event.text ?? state.draftText,
        classification: event.classification ?? state.classification,
      });
      if (!decision.allowed) {
        return { ...state, notice: decision.message };
      }
      const archive = [...state.unresolvedArchive];
      if (decision.archivePrevious && state.pending) {
        archive.push({ ...state.pending });
      }
      const captureId = event.captureId;
      const snapshot = {
        captureId,
        text: event.text ?? state.draftText,
        classification: event.classification ?? state.classification,
      };
      return {
        ...state,
        unresolvedArchive: archive,
        pending: snapshot,
        generation: state.generation + 1,
        notice: null,
        ui: { kind: "submitting", captureId },
        draftText: snapshot.text,
        classification: snapshot.classification,
      };
    }
    case "transport_uncertain": {
      if (event.generation !== state.generation) {
        return state;
      }
      if (!state.pending || state.pending.captureId !== event.captureId) {
        return state;
      }
      return {
        ...state,
        notice: null,
        ui: {
          kind: "uncertain",
          captureId: event.captureId,
          message:
            event.message ||
            "Capture outcome is uncertain after transport failure. The draft was kept. Use Resolve.",
          canResolve: true,
        },
      };
    }
    case "response": {
      // Late responses apply only to matching generation OR archived capture ids.
      const interpreted = interpretCaptureWire(event.response || {});
      const matchesPending =
        state.pending &&
        (event.generation === state.generation ||
          event.response?.captureId === state.pending.captureId) &&
        (!event.captureId || event.captureId === state.pending.captureId);

      if (matchesPending && event.generation === state.generation) {
        if (interpreted.isCommitted) {
          const liveText = event.liveText ?? state.draftText;
          const liveClassification = event.liveClassification ?? state.classification;
          const clear = shouldClearDraftAfterCommit({
            pendingSnapshot: state.pending,
            liveText,
            liveClassification,
            responseCaptureId: event.response?.captureId ?? state.pending.captureId,
          });
          return {
            ...state,
            pending: null,
            notice: null,
            ui: { kind: "committed", message: interpreted.message },
            draftText: clear ? "" : state.draftText,
            classification: clear ? "" : state.classification,
          };
        }
        if (interpreted.isReceiptMissing) {
          return {
            ...state,
            notice: null,
            ui: {
              kind: "uncertain",
              captureId: state.pending.captureId,
              message: interpreted.message,
              canResolve: false,
            },
          };
        }
        if (interpreted.isCaptureIdConflict || interpreted.isRejected) {
          // Keep pending snapshot for Resolve / deliberate-new; surface rejected without losing id.
          return {
            ...state,
            notice: null,
            ui: {
              kind: "uncertain",
              captureId: state.pending.captureId,
              message: interpreted.isCaptureIdConflict
                ? `${interpreted.message} A deliberate new submission will use a new capture id.`
                : `${interpreted.message} The unresolved capture id was kept.`,
              canResolve: !interpreted.isCaptureIdConflict,
            },
          };
        }
        return {
          ...state,
          notice: null,
          ui: {
            kind: "uncertain",
            captureId: state.pending.captureId,
            message: interpreted.message,
            canResolve: true,
          },
        };
      }

      // Late committed for an archived unresolved id: drop from archive only.
      if (interpreted.isCommitted && event.response?.captureId) {
        const id = event.response.captureId;
        const idx = state.unresolvedArchive.findIndex((item) => item.captureId === id);
        if (idx >= 0) {
          const unresolvedArchive = state.unresolvedArchive.slice();
          unresolvedArchive.splice(idx, 1);
          return {
            ...state,
            unresolvedArchive,
            notice: `An earlier unresolved capture (${id}) later reported as saved.`,
          };
        }
      }

      // Stale / late non-matching response: ignore (do not clobber current UI).
      return state;
    }
    case "resolve_start": {
      if (!state.pending || isCaptureBusy(state)) {
        return state;
      }
      if (state.ui.kind === "uncertain" && state.ui.canResolve === false) {
        return {
          ...state,
          notice:
            "ReceiptMissing must not auto-resubmit. Start a deliberate new submission with a new capture id if you accept possible duplication.",
        };
      }
      return {
        ...state,
        generation: state.generation + 1,
        notice: null,
        ui: { kind: "resolving", captureId: state.pending.captureId },
      };
    }
    case "discard": {
      if (isCaptureBusy(state)) {
        return { ...state, notice: "Cannot discard while a capture request is in flight." };
      }
      return createCaptureSubmissionState();
    }
    default:
      return state;
  }
}

/** Native quit guard policy (session-only). Forced kill / OS shutdown are out of scope. */
export function shouldPromptBeforeNativeQuit({ draftDirty, hasUnresolved }) {
  return Boolean(draftDirty || hasUnresolved);
}

export const NATIVE_QUIT_WARNING =
  "Discard unsaved or unresolved capture input and quit? Drafts are session-only and are not written to disk. Cancel keeps Cryptex open.";

export function redactedCaptureTransportMessage(code) {
  switch (code) {
    case "capture_transport_unavailable":
      return "Cryptex could not reach the local capture service. Outcome is uncertain. The draft was kept. Use Resolve.";
    case "capture_transport_timeout":
      return "The capture request timed out. Outcome is uncertain (not cancelled). The draft was kept. Use Resolve.";
    case "capture_rejected":
      return "The capture request was rejected. The draft was kept.";
    case "capture_resolve_transport_unavailable":
      return "Cryptex could not reach the local capture service to resolve. Outcome remains uncertain. The draft was kept.";
    case "capture_resolve_rejected":
      return "Resolve was rejected. The draft was kept. Outcome remains uncertain.";
    default:
      return "Capture transport failed. Outcome is uncertain. The draft was kept. Use Resolve.";
  }
}
