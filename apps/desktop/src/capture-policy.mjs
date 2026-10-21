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
/** @typedef {{ captureId: string, text: string, classification: string, canResolve?: boolean }} CaptureSnapshot */

/**
 * Pure submission policy / state machine for Quick Capture.
 * Ordinary Save never overwrites an unresolved pending snapshot.
 * Deliberate new Save archives the previous unresolved capture for resolve-only tracking.
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
    /** When set, an in-flight resolve targets an archived capture_id (pending unchanged). */
    resolvingArchiveId: null,
  };
}

export function hasArchivedUnresolved(state) {
  return Array.isArray(state.unresolvedArchive) && state.unresolvedArchive.length > 0;
}

export function hasUnresolvedSubmission(state) {
  const pendingUnresolved = Boolean(
    state.pending &&
      (state.ui.kind === "submitting" ||
        state.ui.kind === "resolving" ||
        state.ui.kind === "uncertain"),
  );
  return pendingUnresolved || hasArchivedUnresolved(state);
}

export function isCaptureBusy(state) {
  return state.ui.kind === "submitting" || state.ui.kind === "resolving";
}

/** Session protection for draft discard / native Quit. */
export function sessionNeedsProtection(state) {
  const draftDirty = String(state.draftText ?? "").trim().length > 0;
  return draftDirty || hasUnresolvedSubmission(state);
}

/**
 * Decide whether ordinary Save may begin a new capture_id submission.
 * Does not mutate state.
 */
export function evaluateOrdinarySave(state, { text, classification } = {}) {
  if (isCaptureBusy(state)) {
    return { allowed: false, code: "busy", message: "A capture request is already in flight." };
  }
  if (
    state.pending &&
    (state.ui.kind === "submitting" ||
      state.ui.kind === "resolving" ||
      state.ui.kind === "uncertain")
  ) {
    return {
      allowed: false,
      code: "unresolved_blocks_save",
      message:
        "An earlier capture is still unresolved. Use Resolve, or deliberately start a new capture after acknowledging possible duplication.",
      pendingCaptureId: state.pending.captureId,
      canResolve: state.ui.kind === "uncertain" ? Boolean(state.ui.canResolve) : true,
      receiptMissing: state.ui.kind === "uncertain" && state.ui.canResolve === false,
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
 * Previous pending is archived for resolve-only tracking (never silently abandoned).
 */
export function evaluateDeliberateNewSave(state, { acknowledgeDuplication, text, classification } = {}) {
  if (isCaptureBusy(state)) {
    return { allowed: false, code: "busy", message: "A capture request is already in flight." };
  }
  const pendingUnresolved =
    state.pending &&
    (state.ui.kind === "submitting" ||
      state.ui.kind === "resolving" ||
      state.ui.kind === "uncertain");
  if (!pendingUnresolved) {
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

function archiveEntryFromPending(pending, ui) {
  return {
    captureId: pending.captureId,
    text: pending.text,
    classification: pending.classification,
    canResolve: !(ui.kind === "uncertain" && ui.canResolve === false),
  };
}

function removeArchiveId(archive, captureId) {
  return (archive || []).filter((item) => item.captureId !== captureId);
}

/**
 * Apply a submission-policy event. Returns next state (immutable update).
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
        if (decision.code === "validation" && hasUnresolvedSubmission(state)) {
          return { ...state, notice: decision.message };
        }
        if (decision.code === "unresolved_blocks_save" || decision.code === "busy") {
          return { ...state, notice: decision.message };
        }
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
        resolvingArchiveId: null,
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
      const archive = [...(state.unresolvedArchive || [])];
      if (decision.archivePrevious && state.pending) {
        archive.push(archiveEntryFromPending(state.pending, state.ui));
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
        resolvingArchiveId: null,
        ui: { kind: "submitting", captureId },
        draftText: snapshot.text,
        classification: snapshot.classification,
      };
    }
    case "transport_uncertain": {
      if (event.generation !== state.generation) {
        return state;
      }
      if (state.resolvingArchiveId && event.captureId === state.resolvingArchiveId) {
        return {
          ...state,
          resolvingArchiveId: null,
          ui:
            state.pending && state.ui.kind === "resolving"
              ? {
                  kind: "uncertain",
                  captureId: state.pending.captureId,
                  message: state.ui.message || "Earlier capture still unresolved.",
                  canResolve: true,
                }
              : state.ui.kind === "resolving"
                ? { kind: "idle" }
                : state.ui,
          notice:
            event.message ||
            "Resolve transport failed for an archived capture. It remains tracked.",
        };
      }
      if (!state.pending || state.pending.captureId !== event.captureId) {
        return state;
      }
      return {
        ...state,
        notice: null,
        resolvingArchiveId: null,
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
      const interpreted = interpretCaptureWire(event.response || {});
      const responseId =
        event.captureId || event.response?.captureId || state.pending?.captureId || null;

      // Archived capture response (including late committed) — always persist archive updates.
      const archiveIdx = (state.unresolvedArchive || []).findIndex(
        (item) => item.captureId === responseId,
      );
      const isArchiveTarget =
        archiveIdx >= 0 &&
        (event.generation !== state.generation ||
          state.resolvingArchiveId === responseId ||
          !state.pending ||
          state.pending.captureId !== responseId);

      if (isArchiveTarget) {
        if (interpreted.isCommitted) {
          const unresolvedArchive = removeArchiveId(state.unresolvedArchive, responseId);
          return {
            ...state,
            unresolvedArchive,
            resolvingArchiveId:
              state.resolvingArchiveId === responseId ? null : state.resolvingArchiveId,
            notice: `An earlier unresolved capture (${responseId}) later reported as saved.`,
            // Do not clear current draft/pending for B.
          };
        }
        if (interpreted.isReceiptMissing) {
          const unresolvedArchive = (state.unresolvedArchive || []).map((item) =>
            item.captureId === responseId ? { ...item, canResolve: false } : item,
          );
          return {
            ...state,
            unresolvedArchive,
            resolvingArchiveId:
              state.resolvingArchiveId === responseId ? null : state.resolvingArchiveId,
            notice:
              "No durable receipt for an archived capture id. It remains tracked; do not auto-resubmit.",
          };
        }
        // Other archive outcomes: keep tracked, clear resolving marker.
        return {
          ...state,
          resolvingArchiveId:
            state.resolvingArchiveId === responseId ? null : state.resolvingArchiveId,
          notice: interpreted.message,
        };
      }

      const matchesPending =
        state.pending &&
        event.generation === state.generation &&
        (!event.captureId || event.captureId === state.pending.captureId);

      if (matchesPending) {
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
            notice: hasArchivedUnresolved(state)
              ? `${interpreted.message} Earlier unresolved capture(s) remain tracked.`
              : null,
            resolvingArchiveId: null,
            ui: { kind: "committed", message: interpreted.message },
            draftText: clear ? "" : state.draftText,
            classification: clear ? "" : state.classification,
            // Archive preserved — A stays protected after B commits.
            unresolvedArchive: [...(state.unresolvedArchive || [])],
          };
        }
        if (interpreted.isReceiptMissing) {
          return {
            ...state,
            notice: null,
            resolvingArchiveId: null,
            ui: {
              kind: "uncertain",
              captureId: state.pending.captureId,
              message: interpreted.message,
              canResolve: false,
            },
          };
        }
        if (interpreted.isCaptureIdConflict || interpreted.isRejected) {
          return {
            ...state,
            notice: null,
            resolvingArchiveId: null,
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
          resolvingArchiveId: null,
          ui: {
            kind: "uncertain",
            captureId: state.pending.captureId,
            message: interpreted.message,
            canResolve: true,
          },
        };
      }

      // Stale / non-matching: ignore without clobbering.
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
        resolvingArchiveId: null,
        ui: { kind: "resolving", captureId: state.pending.captureId },
      };
    }
    case "resolve_archive_start": {
      if (isCaptureBusy(state)) {
        return { ...state, notice: "A capture request is already in flight." };
      }
      const captureId = event.captureId;
      const entry = (state.unresolvedArchive || []).find((item) => item.captureId === captureId);
      if (!entry) {
        return { ...state, notice: "That archived capture is no longer tracked." };
      }
      if (entry.canResolve === false) {
        return {
          ...state,
          notice:
            "ReceiptMissing must not auto-resubmit. This archived capture stays tracked until discarded or a deliberate new capture is acknowledged.",
        };
      }
      return {
        ...state,
        generation: state.generation + 1,
        notice: null,
        resolvingArchiveId: captureId,
        ui: { kind: "resolving", captureId },
      };
    }
    case "discard": {
      if (isCaptureBusy(state)) {
        return { ...state, notice: "Cannot discard while a capture request is in flight." };
      }
      return createCaptureSubmissionState();
    }
    case "discard_archive": {
      if (isCaptureBusy(state)) {
        return { ...state, notice: "Cannot discard while a capture request is in flight." };
      }
      if (!event.acknowledgeAbandon) {
        return {
          ...state,
          notice: "Discarding an unresolved archived capture requires explicit acknowledgment.",
        };
      }
      return {
        ...state,
        unresolvedArchive: removeArchiveId(state.unresolvedArchive, event.captureId),
        notice: null,
      };
    }
    default:
      return state;
  }
}

/** Native quit protection policy (session-only). Forced kill / OS shutdown are out of scope. */
export function shouldPromptBeforeNativeQuit({ draftDirty, hasUnresolved, hasArchived }) {
  return Boolean(draftDirty || hasUnresolved || hasArchived);
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
