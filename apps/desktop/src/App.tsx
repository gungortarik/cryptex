import { invoke } from "@tauri-apps/api/core";
import { FormEvent, useEffect, useMemo, useRef, useState } from "react";
import {
  CLASSIFICATIONS,
  MAX_CAPTURE_TEXT_BYTES,
  createCaptureSubmissionGate,
  createCaptureSubmissionState,
  evaluateDeliberateNewSave,
  evaluateOrdinarySave,
  hasArchivedUnresolved,
  hasUnresolvedSubmission,
  newCaptureId,
  redactedCaptureTransportMessage,
  reduceCaptureSubmission,
  shouldPromptBeforeNativeQuit,
  utf8ByteLength,
} from "./capture-policy.mjs";
import { createSearchRequestGate, normalizeSearchText } from "./search-policy.mjs";

type SearchHit = {
  eventId: string;
  brainId: string;
  deviceId: string;
  deviceSeq: string;
  createdAt: string;
  eventType: string;
  classification: string;
  contentHash: string;
  signingKeyId: string;
  text: string;
};

type TimelineCursor = { createdAt: string; eventId: string };

type GuardedSearchResponse = {
  hits: SearchHit[] | null;
  holdReason: string | null;
  nextCursor: TimelineCursor | null;
  hasMore: boolean;
};

type VaultMaterializationResponse = {
  cancelled: boolean;
  noteCount: number | null;
  holdReason: string | null;
  openedInObsidian: boolean;
  openError: string | null;
};

type LocalEnrollmentResponse = {
  deviceId: string;
};

type ApprovalIdentity = {
  deviceId: string;
  signingKeyId: string;
  publicKeyFingerprintSha256: string;
};

type PeerApprovalSnapshot = {
  brainId: string;
  transactionId: string;
  role: string;
  operation: string;
  agreementHash: string | null;
  previousMessageHash: string | null;
  localIdentity: ApprovalIdentity;
  peerIdentity: ApprovalIdentity | null;
  transportAssociation: { localDeviceId: string; peerDeviceId: string } | null;
  restrictionTarget: string | null;
  restrictionReason: string | null;
  cutoffSeq: string | null;
  cutoffContentHash: string | null;
  lastAuthorizedSeq: string | null;
  lastAuthorizedContentHash: string | null;
};

type PeerApprovalPrepared = {
  sessionHandle: string;
  snapshot: PeerApprovalSnapshot;
};

type PeerApprovalResult = {
  accepted: boolean;
  error: string | null;
  signedMessage: string | null;
};

type OwnerSecurityOperationPrepared = {
  sessionHandle: string;
  snapshot: PeerApprovalSnapshot;
};

type OwnerSecurityOperationResult = {
  accepted: boolean;
  error: string | null;
};

type BootstrapExchangeResult = {
  accepted: boolean;
  error: string | null;
  transactionId: string | null;
  exchangeMessage: string | null;
};

type CaptureResponse = {
  accepted: boolean;
  outcome: string | null;
  rejectCode: string | null;
  uncertainReason: string | null;
  captureId: string | null;
  eventId: string | null;
  recordId: string | null;
  createdAt: string | null;
  classification: string | null;
  error: string | null;
};

type CaptureSnapshot = {
  captureId: string;
  text: string;
  classification: string;
  canResolve?: boolean;
};

type CaptureUiState =
  | { kind: "idle" }
  | { kind: "submitting"; captureId: string }
  | { kind: "resolving"; captureId: string }
  | { kind: "committed"; message: string }
  | { kind: "uncertain"; captureId: string; message: string; canResolve: boolean }
  | { kind: "rejected"; message: string; canRetryAsNew: boolean };

type ConnectionState = "checking" | "active" | "attention";

const HOLD_COPY: Record<string, string> = {
  local_enrollment_required:
    "This installation has not been enrolled as a local device yet. Set it up to verify and search this Brain's memory.",
  canonical_verification_unavailable:
    "Cryptex could not verify the complete local event history. Search is paused to protect your memory.",
  unsupported_semantics:
    "A verified event uses semantics this version cannot safely apply. Search is paused until the app is updated.",
  v2_semantics_not_integrated:
    "A structured event affects retrieval, but this build cannot safely interpret it yet. Search is paused.",
  unresolved_dependency:
    "Some knowledge dependencies are unresolved. Search is paused rather than returning an incomplete view.",
  materialization_state_missing_or_stale:
    "The local search index is missing or out of date. Cryptex is withholding results until it is rebuilt safely.",
  projection_does_not_match_canonical:
    "The local search index does not match verified memory. Results are withheld until it is repaired.",
};

function displayDate(value: string): string {
  const date = new Date(value);
  return Number.isNaN(date.getTime())
    ? value
    : new Intl.DateTimeFormat(undefined, { dateStyle: "medium" }).format(date);
}

function displayTimelineDate(value: string): string {
  const date = new Date(value);
  return Number.isNaN(date.getTime())
    ? value
    : new Intl.DateTimeFormat(undefined, {
        dateStyle: "medium",
        timeStyle: "short",
      }).format(date);
}

export function App() {
  const isMac = navigator.platform.toLowerCase().includes("mac");
  const [query, setQuery] = useState("");
  const [hits, setHits] = useState<SearchHit[]>([]);
  const [timelineMode, setTimelineMode] = useState(false);
  const [timelineHits, setTimelineHits] = useState<SearchHit[]>([]);
  const [timelineCursor, setTimelineCursor] = useState<TimelineCursor | null>(null);
  const [timelineHasMore, setTimelineHasMore] = useState(false);
  const [timelineLoading, setTimelineLoading] = useState(false);
  const [detailHit, setDetailHit] = useState<SearchHit | null>(null);
  const [detailLoading, setDetailLoading] = useState(false);
  const [holdReason, setHoldReason] = useState<string | null>(null);
  const [connection, setConnection] = useState<ConnectionState>("checking");
  const [searching, setSearching] = useState(false);
  const [vaultBusy, setVaultBusy] = useState(false);
  const [vaultStatus, setVaultStatus] = useState<string | null>(null);
  const [approvalRole, setApprovalRole] = useState<"initiator" | "responder">("initiator");
  const [approvalTranscript, setApprovalTranscript] = useState("");
  const [approvalPrepared, setApprovalPrepared] = useState<PeerApprovalPrepared | null>(null);
  const [approvedSnapshot, setApprovedSnapshot] = useState<PeerApprovalSnapshot | null>(null);
  const [approvalBusy, setApprovalBusy] = useState(false);
  const [approvalStatus, setApprovalStatus] = useState<string | null>(null);
  const [ownerOperationKind, setOwnerOperationKind] = useState<"device_restriction" | "self_retirement">("device_restriction");
  const [ownerTargetDeviceId, setOwnerTargetDeviceId] = useState("");
  const [ownerRestrictionReason, setOwnerRestrictionReason] = useState<"retired" | "compromised">("compromised");
  const [ownerCutoffSeq, setOwnerCutoffSeq] = useState("");
  const [ownerOperationPrepared, setOwnerOperationPrepared] = useState<OwnerSecurityOperationPrepared | null>(null);
  const [ownerOperationStatus, setOwnerOperationStatus] = useState<string | null>(null);
  const [signedApproval, setSignedApproval] = useState<string | null>(null);
  const [completionTranscript, setCompletionTranscript] = useState("");
  const [exchangeTransportId, setExchangeTransportId] = useState("");
  const [exchangeTransactionId, setExchangeTransactionId] = useState("");
  const [exchangeOffer, setExchangeOffer] = useState("");
  const [exchangeProof, setExchangeProof] = useState("");
  const [incomingOffer, setIncomingOffer] = useState("");
  const [incomingBrainIdentity, setIncomingBrainIdentity] = useState("");
  const [localBrainIdentity, setLocalBrainIdentity] = useState("");
  const [incomingProof, setIncomingProof] = useState("");
  const [incomingInitiatorApproval, setIncomingInitiatorApproval] = useState("");
  const [enrolling, setEnrolling] = useState(false);
  const [deviceId, setDeviceId] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const searchRef = useRef<HTMLInputElement>(null);
  const searchRequestGate = useRef(createSearchRequestGate()).current;
  const captureRequestGate = useRef(createCaptureSubmissionGate()).current;
  const captureSeq = useRef(0);
  const capturePolicyRef = useRef(createCaptureSubmissionState());
  const [draftText, setDraftText] = useState("");
  const [classification, setClassification] = useState("");
  const [captureUi, setCaptureUi] = useState<CaptureUiState>({ kind: "idle" });
  const [pendingSnapshot, setPendingSnapshot] = useState<CaptureSnapshot | null>(null);
  const [unresolvedArchive, setUnresolvedArchive] = useState<
    Array<CaptureSnapshot & { canResolve?: boolean }>
  >([]);
  const [captureNotice, setCaptureNotice] = useState<string | null>(null);
  const [confirmDeliberateNew, setConfirmDeliberateNew] = useState(false);
  const deliberateNewDialogRef = useRef<HTMLDialogElement>(null);
  const draftDirtyRef = useRef(false);
  const liveDraftRef = useRef({ text: "", classification: "" });
  const byteCount = useMemo(() => utf8ByteLength(draftText), [draftText]);
  const captureBusy =
    captureUi.kind === "submitting" || captureUi.kind === "resolving";

  async function materializeAndOpenVault() {
    setVaultBusy(true);
    setVaultStatus(null);
    try {
      const result = await invoke<VaultMaterializationResponse>(
        "p0_6_materialize_and_open_vault",
      );
      if (result.cancelled) return;
      if (result.holdReason) {
        setVaultStatus(`Vault refresh paused for safety: ${result.holdReason}.`);
      } else if (result.openedInObsidian) {
        setVaultStatus(
          `Vault refreshed with ${result.noteCount ?? 0} notes and opened in Obsidian. Use Obsidian’s Graph view to explore relationships.`,
        );
      } else {
        setVaultStatus(
          `Vault refreshed with ${result.noteCount ?? 0} notes, but Obsidian could not be opened. Open the selected vault in Obsidian and choose Graph view.`,
        );
      }
    } catch (error) {
      const code = String(error);
      setVaultStatus(
        code.includes("unsafe_vault_destination")
          ? "Choose a vault outside the Cryptex source and runtime folders."
          : code.includes("invalid_vault_destination")
            ? "Choose an existing folder for the Obsidian vault."
            : "Cryptex could not safely refresh the vault. Check that the device is enrolled and local history is verified.",
      );
    } finally {
      setVaultBusy(false);
    }
  }

  async function preparePeerApproval(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setApprovalBusy(true);
    setApprovalStatus(null);
    setSignedApproval(null);
    setCompletionTranscript("");
    setApprovedSnapshot(null);
    try {
      const messages = approvalTranscript.split(/\r?\n/).map((line) => line.trim()).filter(Boolean);
      const prepared = await invoke<PeerApprovalPrepared>("m8_prepare_peer_approval", {
        role: approvalRole,
        transcriptMessages: messages,
      });
      setApprovalPrepared(prepared);
      setApprovedSnapshot(prepared.snapshot);
      setApprovalStatus("Review every value below and compare the full identifiers with the other device over a trusted separate channel.");
    } catch {
      setApprovalPrepared(null);
      setApprovalStatus("Cryptex could not verify this signed transcript. No approval was created.");
    } finally {
      setApprovalBusy(false);
    }
  }

  async function createBootstrapOffer() {
    setApprovalBusy(true);
    setApprovalStatus(null);
    try {
      const transportDeviceId = await invoke<string>("m8_read_transport_device_id");
      setExchangeTransportId(transportDeviceId);
      const transactionId = exchangeTransactionId || await invoke<string>("m8_new_bootstrap_transaction_id");
      setExchangeTransactionId(transactionId);
      const result = await invoke<BootstrapExchangeResult>("m8_create_bootstrap_offer", {
        transactionId,
        transportDeviceId,
      });
      if (!result.accepted || !result.exchangeMessage) {
        setApprovalStatus(`Offer was not created. ${result.error ?? "No signed message was returned."}`);
        return;
      }
      setExchangeOffer(result.exchangeMessage);
      setApprovalStatus(`Signed offer saved in the daemon journal. Transaction ${result.transactionId ?? "unknown"}. No transport or sync was started.`);
    } catch (error) {
      setApprovalStatus(`Offer was not created: ${String(error)}`);
    } finally {
      setApprovalBusy(false);
    }
  }

  async function readBootstrapBrainIdentity() {
    setApprovalBusy(true);
    setApprovalStatus(null);
    try {
      const identity = await invoke<string>("m8_read_bootstrap_brain_identity");
      setLocalBrainIdentity(identity);
      setApprovalStatus("Canonical public Brain identity loaded. It contains no signing key; copy it to the joining device over your separate trusted channel.");
    } catch {
      setApprovalStatus("Cryptex could not read the canonical Brain identity. No file was changed.");
    } finally {
      setApprovalBusy(false);
    }
  }

  async function createBootstrapProof() {
    setApprovalBusy(true);
    setApprovalStatus(null);
    try {
      const transportDeviceId = await invoke<string>("m8_read_transport_device_id");
      setExchangeTransportId(transportDeviceId);
      const result = await invoke<BootstrapExchangeResult>("m8_create_bootstrap_proof", {
        offerMessage: incomingOffer.trim(),
        brainIdentityJson: incomingBrainIdentity.trim(),
        transportDeviceId,
      });
      if (!result.accepted || !result.exchangeMessage) {
        setApprovalStatus(`Proof was not created. ${result.error ?? "No signed message was returned."}`);
        return;
      }
      setExchangeProof(result.exchangeMessage);
      setApprovalStatus(`Signed proof saved in the daemon journal for transaction ${result.transactionId ?? "unknown"}. No trust or sync was activated.`);
    } catch (error) {
      setApprovalStatus(`Proof was not created: ${String(error)}`);
    } finally {
      setApprovalBusy(false);
    }
  }

  function loadExchangeIntoApprovalReview() {
    const messages = approvalRole === "initiator"
      ? [exchangeOffer, incomingProof]
      : [incomingOffer, exchangeProof, incomingInitiatorApproval];
    if (messages.some((message) => !message.trim())) {
      setApprovalStatus("Complete the signed-message exchange fields first. Nothing was approved.");
      return;
    }
    setApprovalTranscript(messages.map((message) => message.trim()).join("\n"));
    setApprovalPrepared(null);
    setSignedApproval(null);
    setCompletionTranscript("");
    setApprovalStatus("Exchange messages loaded for verification. Verify and review will authenticate them before showing an approval.");
  }

  async function confirmPeerApproval() {
    if (!approvalPrepared) return;
    setApprovalBusy(true);
    setApprovalStatus(null);
    try {
      const result = await invoke<PeerApprovalResult>("m8_confirm_peer_approval", {
        sessionHandle: approvalPrepared.sessionHandle,
      });
      setApprovalPrepared(null);
      if (!result.accepted || !result.signedMessage) {
        setApprovalStatus("Approval was not signed. Review the transcript again before retrying.");
        return;
      }
      setSignedApproval(result.signedMessage);
      const prefix = approvalTranscript.split(/\r?\n/).map((line) => line.trim()).filter(Boolean);
      setCompletionTranscript([...prefix, result.signedMessage].join("\n"));
      setApprovalStatus("Signed approval created. It is not yet a durable trust record; no device was added to the trusted roster.");
    } catch {
      setApprovalPrepared(null);
      setApprovalStatus("Approval could not be completed. The confirmation expired or was consumed; prepare and review it again.");
    } finally {
      setApprovalBusy(false);
    }
  }

  async function cancelPeerApproval() {
    if (!approvalPrepared) return;
    setApprovalBusy(true);
    try {
      const result = await invoke<PeerApprovalResult>("m8_cancel_peer_approval", {
        sessionHandle: approvalPrepared.sessionHandle,
      });
      setApprovalPrepared(null);
      setApprovalStatus(result.accepted ? "Approval cancelled. No signature was created." : "Cancellation could not be confirmed. No trust change was made.");
    } catch {
      setApprovalPrepared(null);
      setApprovalStatus("Approval session closed. No trust change was made.");
    } finally {
      setApprovalBusy(false);
    }
  }

  async function completePeerApproval() {
    setApprovalBusy(true);
    setApprovalStatus(null);
    try {
      const messages = completionTranscript.split(/\r?\n/).map((line) => line.trim()).filter(Boolean);
      const result = await invoke<PeerApprovalResult>("m8_complete_peer_approval", {
        transcriptMessages: messages,
      });
      if (!result.accepted) {
        setApprovalStatus("Cryptex could not safely record this pairing. The transcript was kept for review; no trust change was confirmed.");
        return;
      }
      setApprovalStatus("Pairing recorded in canonical local history. Device sync is not started by this action.");
    } catch {
      setApprovalStatus("Pairing could not be recorded. Keep the exact four signed messages and retry after checking the transaction status.");
    } finally {
      setApprovalBusy(false);
    }
  }

  async function prepareOwnerSecurityOperation(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setApprovalBusy(true);
    setOwnerOperationStatus(null);
    try {
      const cutoff = ownerOperationKind === "device_restriction" ? Number(ownerCutoffSeq) : null;
      if (ownerOperationKind === "device_restriction"
        && (!Number.isSafeInteger(cutoff) || cutoff === null || cutoff < 0)) {
        setOwnerOperationStatus("Enter an exact cutoff sequence number from the reviewed device history.");
        return;
      }
      const prepared = await invoke<OwnerSecurityOperationPrepared>("m8_prepare_owner_security_operation", {
        input: ownerOperationKind === "self_retirement"
          ? { kind: ownerOperationKind, targetDeviceId: null, reason: null, cutoffSeq: null }
          : {
              kind: ownerOperationKind,
              targetDeviceId: ownerTargetDeviceId.trim(),
              reason: ownerRestrictionReason,
              cutoffSeq: cutoff,
            },
      });
      setOwnerOperationPrepared(prepared);
      setOwnerOperationStatus("Review the complete device identity and exact cutoff below before confirming.");
    } catch {
      setOwnerOperationPrepared(null);
      setOwnerOperationStatus("Cryptex could not verify this owner operation against current canonical trust history. Nothing was changed.");
    } finally {
      setApprovalBusy(false);
    }
  }

  async function confirmOwnerSecurityOperation() {
    if (!ownerOperationPrepared) return;
    setApprovalBusy(true);
    setOwnerOperationStatus(null);
    try {
      const result = await invoke<OwnerSecurityOperationResult>("m8_confirm_owner_security_operation", {
        sessionHandle: ownerOperationPrepared.sessionHandle,
      });
      setOwnerOperationPrepared(null);
      setOwnerOperationStatus(result.accepted
        ? "Security operation is anchored in canonical history. The daemon will enforce the new trust cutoff."
        : `Operation was not completed; the daemon held it for review. ${result.error ?? "No further detail was returned."}`);
    } catch {
      setOwnerOperationPrepared(null);
      setOwnerOperationStatus("Confirmation could not be verified. Check the daemon status before retrying; the request may be held for recovery.");
    } finally {
      setApprovalBusy(false);
    }
  }

  async function cancelOwnerSecurityOperation() {
    if (!ownerOperationPrepared) return;
    setApprovalBusy(true);
    try {
      const result = await invoke<OwnerSecurityOperationResult>("m8_cancel_owner_security_operation", {
        sessionHandle: ownerOperationPrepared.sessionHandle,
      });
      setOwnerOperationPrepared(null);
      setOwnerOperationStatus(result.accepted
        ? "Operation cancelled. No canonical event was written."
        : `Cancellation could not be confirmed. ${result.error ?? "No further detail was returned."}`);
    } catch {
      setOwnerOperationPrepared(null);
      setOwnerOperationStatus("Approval session closed. No trust change was confirmed.");
    } finally {
      setApprovalBusy(false);
    }
  }

  useEffect(() => {
    void invoke("p0_3_ipc_health")
      .then(() => setConnection("active"))
      .catch(() => setConnection("attention"));

    function onShortcut(event: KeyboardEvent) {
      if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === "k") {
        event.preventDefault();
        searchRef.current?.focus();
      }
    }
    window.addEventListener("keydown", onShortcut);
    return () => window.removeEventListener("keydown", onShortcut);
  }, []);

  useEffect(() => {
    liveDraftRef.current = { text: draftText, classification };
    capturePolicyRef.current = {
      ...capturePolicyRef.current,
      draftText,
      classification,
    };
  }, [draftText, classification]);

  useEffect(() => {
    const pendingUnresolved =
      pendingSnapshot !== null &&
      (captureUi.kind === "submitting" ||
        captureUi.kind === "resolving" ||
        captureUi.kind === "uncertain");
    const archived = unresolvedArchive.length > 0;
    const protect =
      draftText.trim().length > 0 || pendingUnresolved || archived || captureBusy;
    draftDirtyRef.current = protect;
    const shouldPrompt = shouldPromptBeforeNativeQuit({
      draftDirty: draftText.trim().length > 0,
      hasUnresolved: pendingUnresolved || archived,
      hasArchived: archived,
    });
    void invoke("p0_5_set_capture_session_guard", {
      dirty: shouldPrompt,
      syncFailed: false,
    }).catch(() => {
      // Fail closed on sync errors — native Quit must confirm.
      void invoke("p0_5_set_capture_session_guard", {
        dirty: true,
        syncFailed: true,
      }).catch(() => {});
    });
  }, [draftText, pendingSnapshot, unresolvedArchive, captureBusy, captureUi]);

  useEffect(() => {
    const onBeforeUnload = (event: BeforeUnloadEvent) => {
      if (!draftDirtyRef.current) return;
      event.preventDefault();
      event.returnValue = "";
    };
    window.addEventListener("beforeunload", onBeforeUnload);
    return () => window.removeEventListener("beforeunload", onBeforeUnload);
  }, []);

  async function search(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setTimelineMode(false);
    setDetailHit(null);
    setDetailLoading(false);
    const text = normalizeSearchText(query);
    const generation = searchRequestGate.begin();
    setHits([]);
    setHoldReason(null);
    if (!text) {
      setSearching(false);
      setNotice("Enter a phrase to search your local notes.");
      return;
    }
    setSearching(true);
    setNotice(null);
    try {
      const response = await invoke<GuardedSearchResponse>("p0_4_guarded_search", {
        query: { text, limit: 20 },
      });
      if (!searchRequestGate.isCurrent(generation)) return;
      if (response.holdReason) {
        setConnection("attention");
        setHoldReason(response.holdReason);
        return;
      }
      setConnection("active");
      setHits(response.hits ?? []);
    } catch {
      if (!searchRequestGate.isCurrent(generation)) return;
      setConnection("attention");
      setNotice("Cryptex could not complete a guarded search. No results were kept on screen.");
    } finally {
      if (searchRequestGate.isCurrent(generation)) setSearching(false);
    }
  }

  async function loadTimeline(cursor: TimelineCursor | null) {
    const generation = searchRequestGate.begin();
    setDetailHit(null);
    setDetailLoading(false);
    setTimelineLoading(true);
    setHoldReason(null);
    setNotice(null);
    if (!cursor) {
      setTimelineHits([]);
      setTimelineCursor(null);
      setTimelineHasMore(false);
    }
    try {
      const response = await invoke<GuardedSearchResponse>("p0_5_guarded_timeline", {
        query: { limit: 20, cursor },
      });
      if (!searchRequestGate.isCurrent(generation)) return;
      if (response.holdReason) {
        setConnection("attention");
        setHoldReason(response.holdReason);
        setTimelineHits([]);
        setTimelineCursor(null);
        setTimelineHasMore(false);
        setDetailHit(null);
        return;
      }
      setConnection("active");
      const page = response.hits ?? [];
      setTimelineHits((current) => (cursor ? [...current, ...page] : page));
      setTimelineCursor(response.nextCursor ?? null);
      setTimelineHasMore(response.hasMore);
    } catch {
      if (!searchRequestGate.isCurrent(generation)) return;
      setConnection("attention");
      setNotice("Cryptex could not load a guarded timeline page. No results were kept on screen.");
      setTimelineHits([]);
      setTimelineCursor(null);
      setTimelineHasMore(false);
      setDetailHit(null);
    } finally {
      if (searchRequestGate.isCurrent(generation)) setTimelineLoading(false);
    }
  }

  function openTimeline() {
    searchRequestGate.invalidate();
    setHits([]);
    setDetailHit(null);
    setDetailLoading(false);
    setTimelineMode(true);
    void loadTimeline(null);
  }

  async function openEventDetail(eventId: string) {
    const generation = searchRequestGate.begin();
    setDetailLoading(true);
    setDetailHit(null);
    setHoldReason(null);
    setNotice(null);
    try {
      const response = await invoke<GuardedSearchResponse>("p0_5_guarded_event_detail", {
        eventId,
      });
      if (!searchRequestGate.isCurrent(generation)) return;
      if (response.holdReason) {
        setConnection("attention");
        setHoldReason(response.holdReason);
        return;
      }
      setConnection("active");
      setDetailHit(response.hits?.[0] ?? null);
      if (!response.hits?.length) setNotice("This note is no longer available in the verified local timeline.");
    } catch {
      if (!searchRequestGate.isCurrent(generation)) return;
      setConnection("attention");
      setNotice("Cryptex could not open this guarded note. No content was kept on screen.");
    } finally {
      if (searchRequestGate.isCurrent(generation)) setDetailLoading(false);
    }
  }

  async function enrollThisDevice() {
    setEnrolling(true);
    setNotice(null);
    try {
      const response = await invoke<LocalEnrollmentResponse>("p0_4_enroll_local_device");
      setDeviceId(response.deviceId);
      setHoldReason(null);
      setConnection("active");
      setNotice("This device is enrolled. Search again to check whether local memory is ready.");
    } catch (error) {
      const code = String(error);
      const message =
        code.includes("brain_identity_unavailable")
          ? "A Brain identity is not available in this runtime yet. Device enrollment was not completed."
          : code.includes("unbound_existing_device_state")
            ? "Existing local event data has no verified device binding. Cryptex left it untouched; recovery review is required."
            : code.includes("secure_store_unavailable")
              ? "The operating-system secure store could not complete enrollment. No replacement identity was created."
              : "Cryptex could not enroll this device. Existing identity and event data were left unchanged.";
      setNotice(message);
    } finally {
      setEnrolling(false);
    }
  }

  function syncUiFromPolicy(next: ReturnType<typeof createCaptureSubmissionState>) {
    capturePolicyRef.current = next;
    setPendingSnapshot(next.pending);
    setUnresolvedArchive([...(next.unresolvedArchive || [])]);
    setCaptureNotice(next.notice);
    setDraftText(next.draftText);
    setClassification(next.classification);
    if (next.ui.kind === "idle") {
      setCaptureUi({ kind: "idle" });
    } else if (next.ui.kind === "submitting") {
      setCaptureUi({ kind: "submitting", captureId: next.ui.captureId });
    } else if (next.ui.kind === "resolving") {
      setCaptureUi({ kind: "resolving", captureId: next.ui.captureId });
    } else if (next.ui.kind === "committed") {
      setCaptureUi({ kind: "committed", message: next.ui.message });
    } else if (next.ui.kind === "uncertain") {
      setCaptureUi({
        kind: "uncertain",
        captureId: next.ui.captureId,
        message: next.ui.message,
        canResolve: next.ui.canResolve,
      });
    } else if (next.ui.kind === "rejected") {
      setCaptureUi({
        kind: "rejected",
        message: next.ui.message,
        canRetryAsNew: next.ui.canRetryAsNew,
      });
    }
  }

  function currentPolicyState() {
    const state = {
      ...capturePolicyRef.current,
      draftText,
      classification,
      pending: pendingSnapshot ?? capturePolicyRef.current.pending,
      unresolvedArchive:
        unresolvedArchive.length > 0
          ? unresolvedArchive
          : capturePolicyRef.current.unresolvedArchive || [],
      generation: captureSeq.current,
      notice: captureNotice,
    };
    state.ui =
      captureUi.kind === "idle"
        ? { kind: "idle" }
        : captureUi.kind === "submitting"
          ? { kind: "submitting", captureId: captureUi.captureId }
          : captureUi.kind === "resolving"
            ? { kind: "resolving", captureId: captureUi.captureId }
            : captureUi.kind === "committed"
              ? { kind: "committed", message: captureUi.message }
              : captureUi.kind === "uncertain"
                ? {
                    kind: "uncertain",
                    captureId: captureUi.captureId,
                    message: captureUi.message,
                    canResolve: captureUi.canResolve,
                  }
                : {
                    kind: "rejected",
                    message: captureUi.message,
                    canRetryAsNew: captureUi.canRetryAsNew,
                  };
    return state;
  }

  function applyCaptureResponse(response: CaptureResponse, snapshot: CaptureSnapshot, generation: number) {
    const next = reduceCaptureSubmission(currentPolicyState(), {
      type: "response",
      generation,
      captureId: snapshot.captureId,
      response,
      liveText: liveDraftRef.current.text,
      liveClassification: liveDraftRef.current.classification,
    });
    // Persist archive/pending/draft updates even for late/stale generations.
    if (captureRequestGate.isCurrent(generation)) {
      captureSeq.current = next.generation;
    }
    syncUiFromPolicy(next);
  }

  async function submitCapture(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const state = currentPolicyState();
    const ordinary = evaluateOrdinarySave(state, {
      text: draftText,
      classification,
    });
    if (!ordinary.allowed) {
      if (ordinary.code === "unresolved_blocks_save") {
        setCaptureNotice(ordinary.message ?? null);
        setConfirmDeliberateNew(true);
        return;
      }
      const next = reduceCaptureSubmission(state, {
        type: "save",
        captureId: newCaptureId(),
        text: draftText,
        classification,
      });
      // reduceCaptureSubmission on blocked save only sets notice / rejected
      syncUiFromPolicy(next);
      return;
    }

    const captureId = newCaptureId();
    const generation = captureRequestGate.begin();
    const next = reduceCaptureSubmission(state, {
      type: "save",
      captureId,
      text: draftText,
      classification,
    });
    next.generation = generation;
    captureSeq.current = generation;
    syncUiFromPolicy(next);
    const snapshot = next.pending!;
    try {
      const response = await invoke<CaptureResponse>("p0_5_capture_memory", {
        body: {
          captureId,
          text: snapshot.text,
          classification: snapshot.classification,
        },
      });
      applyCaptureResponse(response, snapshot, generation);
    } catch (error) {
      if (!captureRequestGate.isCurrent(generation)) return;
      const code = error instanceof Error ? error.message : String(error);
      const message = redactedCaptureTransportMessage(code);
      const uncertain = reduceCaptureSubmission(currentPolicyState(), {
        type: "transport_uncertain",
        generation,
        captureId,
        message,
      });
      syncUiFromPolicy(uncertain);
    }
  }

  async function confirmDeliberateNewCapture() {
    setConfirmDeliberateNew(false);
    const state = currentPolicyState();
    const deliberateDecision = evaluateDeliberateNewSave(state, {
      acknowledgeDuplication: true,
      text: draftText,
      classification,
    });
    if (!deliberateDecision.allowed) {
      setCaptureNotice(deliberateDecision.message ?? null);
      return;
    }
    const captureId = newCaptureId();
    const generation = captureRequestGate.begin();
    const next = reduceCaptureSubmission(state, {
      type: "save_deliberate",
      acknowledgeDuplication: true,
      captureId,
      text: draftText,
      classification,
    });
    next.generation = generation;
    captureSeq.current = generation;
    syncUiFromPolicy(next);
    const snapshot = next.pending!;
    try {
      const response = await invoke<CaptureResponse>("p0_5_capture_memory", {
        body: {
          captureId,
          text: snapshot.text,
          classification: snapshot.classification,
        },
      });
      applyCaptureResponse(response, snapshot, generation);
    } catch (error) {
      if (!captureRequestGate.isCurrent(generation)) return;
      const code = error instanceof Error ? error.message : String(error);
      const message = redactedCaptureTransportMessage(code);
      const uncertain = reduceCaptureSubmission(currentPolicyState(), {
        type: "transport_uncertain",
        generation,
        captureId,
        message,
      });
      syncUiFromPolicy(uncertain);
    }
  }

  function cancelDeliberateNewCapture() {
    setConfirmDeliberateNew(false);
  }

  useEffect(() => {
    const dialog = deliberateNewDialogRef.current;
    if (!dialog) return;
    if (confirmDeliberateNew && !dialog.open) dialog.showModal();
    if (!confirmDeliberateNew && dialog.open) dialog.close();
  }, [confirmDeliberateNew]);

  async function resolvePendingCapture() {
    const state = currentPolicyState();
    if (!state.pending || isBusy(state)) return;
    const started = reduceCaptureSubmission(state, { type: "resolve_start" });
    if (started.notice && started.ui.kind === "uncertain" && !started.ui.canResolve) {
      syncUiFromPolicy(started);
      return;
    }
    if (started.ui.kind !== "resolving") {
      syncUiFromPolicy(started);
      return;
    }
    const generation = captureRequestGate.begin();
    started.generation = generation;
    captureSeq.current = generation;
    syncUiFromPolicy(started);
    const snapshot = started.pending!;
    try {
      const response = await invoke<CaptureResponse>("p0_5_resolve_capture", {
        body: { captureId: snapshot.captureId },
      });
      applyCaptureResponse(response, snapshot, generation);
    } catch (error) {
      if (!captureRequestGate.isCurrent(generation)) return;
      const code = error instanceof Error ? error.message : String(error);
      const message = redactedCaptureTransportMessage(
        code === "capture_resolve_transport_unavailable" ||
          code === "capture_resolve_rejected"
          ? code
          : "capture_resolve_transport_unavailable",
      );
      const uncertain = reduceCaptureSubmission(currentPolicyState(), {
        type: "transport_uncertain",
        generation,
        captureId: snapshot.captureId,
        message,
      });
      syncUiFromPolicy(uncertain);
    }
  }

  async function resolveArchivedCapture(captureId: string) {
    const state = currentPolicyState();
    if (isBusy(state)) return;
    const started = reduceCaptureSubmission(state, {
      type: "resolve_archive_start",
      captureId,
    });
    if (started.resolvingArchiveId !== captureId) {
      syncUiFromPolicy(started);
      return;
    }
    const generation = captureRequestGate.begin();
    started.generation = generation;
    captureSeq.current = generation;
    syncUiFromPolicy(started);
    try {
      const response = await invoke<CaptureResponse>("p0_5_resolve_capture", {
        body: { captureId },
      });
      applyCaptureResponse(
        response,
        { captureId, text: "", classification: "" },
        generation,
      );
    } catch (error) {
      if (!captureRequestGate.isCurrent(generation)) return;
      const code = error instanceof Error ? error.message : String(error);
      const message = redactedCaptureTransportMessage(
        code.startsWith("capture_resolve")
          ? code
          : "capture_resolve_transport_unavailable",
      );
      const uncertain = reduceCaptureSubmission(currentPolicyState(), {
        type: "transport_uncertain",
        generation,
        captureId,
        message,
      });
      syncUiFromPolicy(uncertain);
    }
  }

  function isBusy(state: ReturnType<typeof createCaptureSubmissionState>) {
    return state.ui.kind === "submitting" || state.ui.kind === "resolving";
  }

  function discardDraft() {
    const state = currentPolicyState();
    if (isBusy(state)) return;
    if (
      draftDirtyRef.current &&
      !window.confirm(
        "Discard unsaved or unresolved capture input? Drafts are not written to disk in this slice.",
      )
    ) {
      return;
    }
    captureRequestGate.invalidate();
    const next = reduceCaptureSubmission(state, { type: "discard" });
    captureSeq.current = next.generation;
    syncUiFromPolicy(next);
    void invoke("p0_5_set_capture_session_guard", { dirty: false }).catch(() => {});
  }

  const resolvingArchiveCaptureId =
    captureUi.kind === "resolving" ? captureUi.captureId : null;

  const holdCopy = holdReason
    ? HOLD_COPY[holdReason] ?? "Search is paused because Cryptex could not verify a safe retrieval state."
    : null;

  return (
    <main className="app-shell">
      <aside className="side-rail" aria-label="Primary">
        <a className="brand-mark" href="#home" aria-label="Cryptex home">
          C<span>.</span>
        </a>
        <div className="rail-rule" />
        <button
          className={`rail-action ${!timelineMode ? "is-selected" : ""}`}
          type="button"
          aria-label="Search memory"
          aria-pressed={!timelineMode}
          onClick={() => {
            searchRequestGate.invalidate();
            setTimelineMode(false);
            setTimelineLoading(false);
            setDetailHit(null);
            setDetailLoading(false);
          }}
        >
          <span aria-hidden="true">⌕</span>
        </button>
        <button
          className={`rail-action ${timelineMode ? "is-selected" : ""}`}
          type="button"
          aria-label="Open timeline"
          aria-pressed={timelineMode}
          onClick={openTimeline}
        >
          <span aria-hidden="true">◷</span>
        </button>
        <div className="rail-bottom" aria-hidden="true">M5</div>
      </aside>

      <section className="workspace" id="home">
        <header className="topbar">
          <div className="wordmark">Cryptex</div>
          <div className={`connection-state state-${connection}`} role="status">
            <span className="state-dot" />
            {connection === "checking" ? "Checking" : connection === "active" ? "Active" : "Attention"}
          </div>
        </header>

        <div className="content-column">
          <div className="page-heading">
            <p className="eyebrow">Your personal archive</p>
            <h1>Find what you remember.</h1>
            <p className="intro">
              Search stays on this device and works offline. No AI is needed to find a note.
            </p>
          </div>

          <section className="vault-panel" aria-labelledby="vault-heading">
            <div>
              <p className="panel-kicker">Obsidian</p>
              <h2 id="vault-heading">Your verified knowledge view</h2>
              <p className="capture-help">
                Refresh a generated copy from verified local memory, then explore it with Obsidian Graph. Cryptex keeps canonical memory separate.
              </p>
            </div>
            <button
              className="search-button"
              type="button"
              disabled={vaultBusy || connection !== "active"}
              onClick={() => void materializeAndOpenVault()}
            >
              {vaultBusy ? "Refreshing…" : "Choose vault, refresh & open"}
            </button>
            {vaultStatus ? <p className="capture-status" role="status">{vaultStatus}</p> : null}
          </section>

          <section className="bootstrap-panel" aria-labelledby="bootstrap-heading">
            <div>
              <p className="panel-kicker">Device trust · M8 Windows flow</p>
              <h2 id="bootstrap-heading">Review a signed pairing request</h2>
              <p className="capture-help">
                Exchange signed public messages by copy and paste. Compare full device IDs, key fingerprints, and transport IDs through a separate trusted channel. This flow does not start transport or sync.
              </p>
            </div>
            {!approvalPrepared ? (
              <div className="bootstrap-review" aria-labelledby="bootstrap-exchange-heading">
                <h3 id="bootstrap-exchange-heading">1. Create or receive the signed exchange</h3>
                <label htmlFor="bootstrap-role">Your role</label>
                <select
                  id="bootstrap-role"
                  value={approvalRole}
                  onChange={(event) => setApprovalRole(event.target.value as "initiator" | "responder")}
                  disabled={approvalBusy}
                >
                  <option value="initiator">Initiator · create offer</option>
                  <option value="responder">Responder · create proof</option>
                </select>
                <button className="capture-secondary" type="button" onClick={() => void invoke<string>("m8_read_transport_device_id").then((id) => { setExchangeTransportId(id); setApprovalStatus("Adapter identity verified by the daemon’s active local connection."); }).catch(() => { setExchangeTransportId(""); setApprovalStatus("No active, identity-verified adapter is available. Pairing was not created."); })} disabled={approvalBusy || connection !== "active"}>
                  {exchangeTransportId ? "Refresh verified adapter identity" : "Read verified adapter identity"}
                </button>
                {exchangeTransportId ? <p className="capture-help">Verified adapter DeviceId: <code>{exchangeTransportId}</code></p> : <p className="capture-help">Pairing requires the configured adapter to be active. Its DeviceId is read and rechecked by the daemon; it cannot be entered manually.</p>}
                <button className="capture-secondary" type="button" onClick={() => void readBootstrapBrainIdentity()} disabled={approvalBusy || connection !== "active"}>
                  {approvalBusy ? "Reading…" : "Show public Brain identity to share"}
                </button>
                {localBrainIdentity ? <label className="bootstrap-signed-message" htmlFor="bootstrap-brain-identity-output">Public Brain identity JSON · copy exactly<textarea id="bootstrap-brain-identity-output" value={localBrainIdentity} readOnly rows={4} /></label> : null}
                {approvalRole === "initiator" ? (
                  <>
                    <label htmlFor="bootstrap-transaction-id">Transaction ID · keep it to resume after restart</label>
                    <input
                      id="bootstrap-transaction-id"
                      value={exchangeTransactionId}
                      onChange={(event) => {
                        setExchangeTransactionId(event.target.value.trim());
                        setExchangeOffer("");
                      }}
                      placeholder="Generated once, or paste a saved transaction ID"
                      maxLength={64}
                      disabled={approvalBusy}
                    />
                    <button className="search-button" type="button" onClick={() => void createBootstrapOffer()} disabled={approvalBusy || connection !== "active"}>
                      {approvalBusy ? "Creating…" : exchangeTransactionId ? "Create or resume signed offer" : "Create signed offer"}
                    </button>
                    {exchangeTransactionId ? <button className="capture-secondary" type="button" onClick={() => { setExchangeTransactionId(""); setExchangeOffer(""); setIncomingProof(""); setApprovalTranscript(""); }} disabled={approvalBusy}>Start a new transaction</button> : null}
                    {exchangeOffer ? <label className="bootstrap-signed-message" htmlFor="bootstrap-offer-output">Offer · send to the responder<textarea id="bootstrap-offer-output" value={exchangeOffer} readOnly rows={4} /></label> : null}
                    <label htmlFor="bootstrap-proof-input">Responder’s signed proof</label>
                    <textarea id="bootstrap-proof-input" value={incomingProof} onChange={(event) => setIncomingProof(event.target.value)} rows={4} maxLength={65536} disabled={approvalBusy} />
                  </>
                ) : (
                  <>
                    <label htmlFor="bootstrap-offer-input">Initiator’s signed offer</label>
                    <textarea id="bootstrap-offer-input" value={incomingOffer} onChange={(event) => setIncomingOffer(event.target.value)} rows={4} maxLength={65536} disabled={approvalBusy} />
                    <label htmlFor="bootstrap-brain-identity">Existing Brain identity JSON · copy exactly from the initiating device</label>
                    <textarea id="bootstrap-brain-identity" value={incomingBrainIdentity} onChange={(event) => setIncomingBrainIdentity(event.target.value)} rows={4} maxLength={4096} disabled={approvalBusy} />
                    <p className="capture-help">If this device has no Brain identity, the daemon will create the explicit join-existing-Brain setup from this exact public identity, only when no conflicting local state exists. A different local Brain is never replaced.</p>
                    <button className="search-button" type="button" onClick={() => void createBootstrapProof()} disabled={approvalBusy || connection !== "active" || !incomingOffer.trim() || !incomingBrainIdentity.trim()}>
                      {approvalBusy ? "Creating…" : exchangeProof ? "Resume signed proof" : "Verify offer and create proof"}
                    </button>
                    {exchangeProof ? <label className="bootstrap-signed-message" htmlFor="bootstrap-proof-output">Proof · return to the initiator<textarea id="bootstrap-proof-output" value={exchangeProof} readOnly rows={4} /></label> : null}
                    <label htmlFor="bootstrap-initiator-approval-input">Initiator’s signed approval</label>
                    <textarea id="bootstrap-initiator-approval-input" value={incomingInitiatorApproval} onChange={(event) => setIncomingInitiatorApproval(event.target.value)} rows={4} maxLength={65536} disabled={approvalBusy} />
                  </>
                )}
                <button className="capture-secondary" type="button" onClick={loadExchangeIntoApprovalReview} disabled={approvalBusy}>
                  Load messages for owner review
                </button>
              </div>
            ) : null}
            {!approvalPrepared ? (
              <form className="bootstrap-form" onSubmit={(event) => void preparePeerApproval(event)}>
                <h3>2. Verify and review before signing</h3>
                <p className="capture-help">The signed offer, proof, and any received approval are shown in this exact order. You can inspect or correct the lines before verification.</p>
                <label htmlFor="bootstrap-transcript">Signed messages · one compact JSON message per line</label>
                <textarea
                  id="bootstrap-transcript"
                  value={approvalTranscript}
                  onChange={(event) => setApprovalTranscript(event.target.value)}
                  placeholder={'Paste one complete signed JSON message per line'}
                  rows={5}
                  maxLength={1052672}
                  required
                  disabled={approvalBusy}
                />
                <button className="search-button" type="submit" disabled={approvalBusy || connection !== "active"}>
                  {approvalBusy ? "Verifying…" : "Verify and review"}
                </button>
              </form>
            ) : (
              <div className="bootstrap-review" aria-labelledby="bootstrap-review-heading">
                <h3 id="bootstrap-review-heading">Confirm this exact operation</h3>
                <p className="capture-help">Compare the full Brain, device IDs, key fingerprints, and transport IDs with the other device over a separate trusted channel. Approving signs this request only.</p>
                <dl>
                  <dt>Operation · role</dt><dd>{approvalPrepared.snapshot.operation} · {approvalPrepared.snapshot.role}</dd>
                  <dt>Brain ID</dt><dd><code>{approvalPrepared.snapshot.brainId}</code></dd>
                  <dt>Transaction</dt><dd><code>{approvalPrepared.snapshot.transactionId}</code></dd>
                  <dt>This device</dt><dd><code>{approvalPrepared.snapshot.localIdentity.deviceId}</code></dd>
                  <dt>This signing-key ID</dt><dd><code>{approvalPrepared.snapshot.localIdentity.signingKeyId}</code></dd>
                  <dt>This key fingerprint · SHA-256</dt><dd><code>{approvalPrepared.snapshot.localIdentity.publicKeyFingerprintSha256}</code></dd>
                  <dt>Peer device</dt><dd><code>{approvalPrepared.snapshot.peerIdentity?.deviceId ?? "—"}</code></dd>
                  <dt>Peer signing-key ID</dt><dd><code>{approvalPrepared.snapshot.peerIdentity?.signingKeyId ?? "—"}</code></dd>
                  <dt>Peer key fingerprint · SHA-256</dt><dd><code>{approvalPrepared.snapshot.peerIdentity?.publicKeyFingerprintSha256 ?? "—"}</code></dd>
                  <dt>Transport association</dt><dd><code>{approvalPrepared.snapshot.transportAssociation ? `${approvalPrepared.snapshot.transportAssociation.localDeviceId} ↔ ${approvalPrepared.snapshot.transportAssociation.peerDeviceId}` : "—"}</code></dd>
                  <dt>Agreement hash</dt><dd><code>{approvalPrepared.snapshot.agreementHash ?? "—"}</code></dd>
                  <dt>Previous-message hash</dt><dd><code>{approvalPrepared.snapshot.previousMessageHash ?? "—"}</code></dd>
                </dl>
                <div className="capture-confirm-actions">
                  <button className="capture-secondary" type="button" onClick={() => void cancelPeerApproval()} disabled={approvalBusy}>Cancel</button>
                  <button className="search-button" type="button" onClick={() => void confirmPeerApproval()} disabled={approvalBusy}>
                    {approvalBusy ? "Signing…" : "Approve and sign"}
                  </button>
                </div>
              </div>
            )}
            {approvalStatus ? <p className="capture-status" role="status">{approvalStatus}</p> : null}
            {signedApproval ? (
              <>
                {approvedSnapshot ? (
                  <div className="bootstrap-review" aria-labelledby="bootstrap-complete-heading">
                    <h3 id="bootstrap-complete-heading">Device to add after transcript verification</h3>
                    <dl>
                      <dt>Peer device</dt><dd><code>{approvedSnapshot.peerIdentity?.deviceId ?? "—"}</code></dd>
                      <dt>Peer signing-key ID</dt><dd><code>{approvedSnapshot.peerIdentity?.signingKeyId ?? "—"}</code></dd>
                      <dt>Peer key fingerprint · SHA-256</dt><dd><code>{approvedSnapshot.peerIdentity?.publicKeyFingerprintSha256 ?? "—"}</code></dd>
                      <dt>Transport association</dt><dd><code>{approvedSnapshot.transportAssociation ? `${approvedSnapshot.transportAssociation.localDeviceId} ↔ ${approvedSnapshot.transportAssociation.peerDeviceId}` : "—"}</code></dd>
                      <dt>Transaction</dt><dd><code>{approvedSnapshot.transactionId}</code></dd>
                    </dl>
                  </div>
                ) : null}
                <label className="bootstrap-signed-message" htmlFor="bootstrap-signed-message">
                  Signed message to return to the other device
                  <textarea id="bootstrap-signed-message" value={signedApproval} readOnly rows={4} />
                </label>
                <label className="bootstrap-signed-message" htmlFor="bootstrap-complete-transcript">
                  Complete transcript · paste the peer’s final signed approval as message 4 when needed
                  <textarea
                    id="bootstrap-complete-transcript"
                    value={completionTranscript}
                    onChange={(event) => setCompletionTranscript(event.target.value)}
                    rows={6}
                    maxLength={1052672}
                    disabled={approvalBusy}
                  />
                </label>
                <button
                  className="search-button"
                  type="button"
                  onClick={() => void completePeerApproval()}
                  disabled={approvalBusy || connection !== "active"}
                >
                  {approvalBusy ? "Verifying…" : "Add approved device to local trust"}
                </button>
              </>
            ) : null}
          </section>

          <section className="bootstrap-panel" aria-labelledby="owner-security-heading">
            <div>
              <p className="panel-kicker">Device trust · owner action</p>
              <h2 id="owner-security-heading">Restrict a device or retire this one</h2>
              <p className="capture-help">
                Cryptex derives identity and cutoff evidence from verified canonical history. Confirming writes one signed security event; it does not delete history or start sync.
              </p>
            </div>
            {!ownerOperationPrepared ? (
              <form className="bootstrap-form" onSubmit={(event) => void prepareOwnerSecurityOperation(event)}>
                <label htmlFor="owner-operation-kind">Security operation</label>
                <select
                  id="owner-operation-kind"
                  value={ownerOperationKind}
                  onChange={(event) => setOwnerOperationKind(event.target.value as "device_restriction" | "self_retirement")}
                  disabled={approvalBusy || connection !== "active"}
                >
                  <option value="device_restriction">Restrict an approved device</option>
                  <option value="self_retirement">Retire this device</option>
                </select>
                {ownerOperationKind === "device_restriction" ? (
                  <>
                    <label htmlFor="owner-target-device">Approved device ID</label>
                    <input
                      id="owner-target-device"
                      value={ownerTargetDeviceId}
                      onChange={(event) => setOwnerTargetDeviceId(event.target.value)}
                      maxLength={128}
                      required
                      disabled={approvalBusy}
                    />
                    <label htmlFor="owner-restriction-reason">Reason</label>
                    <select
                      id="owner-restriction-reason"
                      value={ownerRestrictionReason}
                      onChange={(event) => setOwnerRestrictionReason(event.target.value as "retired" | "compromised")}
                      disabled={approvalBusy}
                    >
                      <option value="compromised">Compromised</option>
                      <option value="retired">Retired</option>
                    </select>
                    <label htmlFor="owner-cutoff-seq">Exact authorized sequence cutoff</label>
                    <input
                      id="owner-cutoff-seq"
                      type="number"
                      min="0"
                      step="1"
                      value={ownerCutoffSeq}
                      onChange={(event) => setOwnerCutoffSeq(event.target.value)}
                      required
                      disabled={approvalBusy}
                    />
                    <p className="capture-help">The daemon checks that this exact boundary belongs to the approved device. The matching content hash is shown before confirmation.</p>
                  </>
                ) : (
                  <p className="capture-help">This permanently ends this device’s authority after the displayed last-authorized event. Review the exact boundary before confirming.</p>
                )}
                <button className="search-button" type="submit" disabled={approvalBusy || connection !== "active"}>
                  {approvalBusy ? "Checking…" : "Prepare owner review"}
                </button>
              </form>
            ) : (
              <div className="bootstrap-review" aria-labelledby="owner-operation-review-heading">
                <h3 id="owner-operation-review-heading">Review the exact security change</h3>
                <p className="capture-help">
                  Confirm only if these full identifiers and boundary are correct. The confirmation is tied to this live desktop connection and expires automatically.
                </p>
                <dl>
                  <dt>Operation</dt><dd>{ownerOperationPrepared.snapshot.operation}</dd>
                  <dt>Brain ID</dt><dd><code>{ownerOperationPrepared.snapshot.brainId}</code></dd>
                  <dt>Transaction</dt><dd><code>{ownerOperationPrepared.snapshot.transactionId}</code></dd>
                  <dt>Issuing device</dt><dd><code>{ownerOperationPrepared.snapshot.localIdentity.deviceId}</code></dd>
                  <dt>Issuing signing key</dt><dd><code>{ownerOperationPrepared.snapshot.localIdentity.signingKeyId}</code></dd>
                  {ownerOperationPrepared.snapshot.operation === "device_restriction" ? (
                    <>
                      <dt>Target device</dt><dd><code>{ownerOperationPrepared.snapshot.peerIdentity?.deviceId ?? "—"}</code></dd>
                      <dt>Target signing key</dt><dd><code>{ownerOperationPrepared.snapshot.peerIdentity?.signingKeyId ?? "—"}</code></dd>
                      <dt>Target key fingerprint · SHA-256</dt><dd><code>{ownerOperationPrepared.snapshot.peerIdentity?.publicKeyFingerprintSha256 ?? "—"}</code></dd>
                      <dt>Reason</dt><dd>{ownerOperationPrepared.snapshot.restrictionReason ?? "—"}</dd>
                      <dt>Authorized through sequence</dt><dd>{ownerOperationPrepared.snapshot.cutoffSeq ?? "—"}</dd>
                      <dt>Cutoff content hash</dt><dd><code>{ownerOperationPrepared.snapshot.cutoffContentHash ?? "—"}</code></dd>
                      <dt>Transport association</dt><dd><code>{ownerOperationPrepared.snapshot.transportAssociation ? `${ownerOperationPrepared.snapshot.transportAssociation.localDeviceId} ↔ ${ownerOperationPrepared.snapshot.transportAssociation.peerDeviceId}` : "—"}</code></dd>
                    </>
                  ) : (
                    <>
                      <dt>Last-authorized sequence</dt><dd>{ownerOperationPrepared.snapshot.lastAuthorizedSeq ?? "—"}</dd>
                      <dt>Last-authorized content hash</dt><dd><code>{ownerOperationPrepared.snapshot.lastAuthorizedContentHash ?? "—"}</code></dd>
                    </>
                  )}
                </dl>
                <div className="capture-confirm-actions">
                  <button className="capture-secondary" type="button" onClick={() => void cancelOwnerSecurityOperation()} disabled={approvalBusy}>Cancel</button>
                  <button className="search-button" type="button" onClick={() => void confirmOwnerSecurityOperation()} disabled={approvalBusy}>
                    {approvalBusy ? "Applying…" : "Confirm security change"}
                  </button>
                </div>
              </div>
            )}
            {ownerOperationStatus ? <p className="capture-status" role="status">{ownerOperationStatus}</p> : null}
          </section>

          <section className="capture-panel" aria-labelledby="capture-heading">
            <p className="panel-kicker">Quick capture</p>
            <h2 id="capture-heading">Inbox</h2>
            <p className="capture-help">
              Classification is required. Text is kept exactly up to{" "}
              {MAX_CAPTURE_TEXT_BYTES.toLocaleString()} UTF-8 bytes. Saved means
              canonical durability only — not search freshness. Drafts stay in this
              session; nothing is stored in localStorage.
            </p>
            <form onSubmit={submitCapture}>
              <label className="capture-label" htmlFor="capture-text">
                Note
              </label>
              <textarea
                id="capture-text"
                className="capture-textarea"
                value={draftText}
                disabled={false}
                onChange={(event) => {
                  setDraftText(event.target.value);
                  if (captureUi.kind === "committed" || captureUi.kind === "rejected") {
                    setCaptureUi({ kind: "idle" });
                  }
                }}
                spellCheck
              />
              <p className="capture-byte-count">
                {byteCount.toLocaleString()} / {MAX_CAPTURE_TEXT_BYTES.toLocaleString()} bytes
              </p>
              <div className="capture-meta-row">
                <div>
                  <label className="capture-label" htmlFor="capture-classification">
                    Classification
                  </label>
                  <select
                    id="capture-classification"
                    className="capture-select"
                    value={classification}
                    disabled={false}
                    onChange={(event) => setClassification(event.target.value)}
                    required
                  >
                    <option value="">Select classification…</option>
                    {CLASSIFICATIONS.map((value) => (
                      <option key={value} value={value}>
                        {value}
                      </option>
                    ))}
                  </select>
                </div>
                <div className="capture-actions">
                  <button className="search-button" type="submit" disabled={captureBusy || searching}>
                    {captureUi.kind === "submitting" ? "Saving…" : "Save capture"}
                  </button>
                  <button
                    className="capture-secondary"
                    type="button"
                    disabled={
                      captureBusy ||
                      !pendingSnapshot ||
                      (captureUi.kind === "uncertain" && !captureUi.canResolve)
                    }
                    onClick={() => void resolvePendingCapture()}
                  >
                    {captureUi.kind === "resolving" ? "Resolving…" : "Resolve"}
                  </button>
                  <button
                    className="capture-secondary"
                    type="button"
                    disabled={captureBusy}
                    onClick={discardDraft}
                  >
                    Discard
                  </button>
                </div>
              </div>
            </form>
            {captureUi.kind === "submitting" || captureUi.kind === "resolving" ? (
              <p className="capture-status" role="status">
                {captureUi.kind === "submitting"
                  ? `Submitting capture ${captureUi.captureId}…`
                  : `Resolving capture ${captureUi.captureId}…`}
              </p>
            ) : null}
            {captureUi.kind === "committed" ? (
              <p className="capture-status is-ok" role="status">
                {captureUi.message}
              </p>
            ) : null}
            {captureUi.kind === "uncertain" || captureUi.kind === "rejected" ? (
              <p className="capture-status" role="alert">
                {captureUi.message}
              </p>
            ) : null}
            {captureNotice ? (
              <p className="capture-status" role="status">
                {captureNotice}
              </p>
            ) : null}
                        {pendingSnapshot ? (
              <p className="capture-byte-count">
                Pending capture id {pendingSnapshot.captureId} (
                {pendingSnapshot.classification})
                {captureUi.kind === "uncertain" ||
                captureUi.kind === "submitting" ||
                captureUi.kind === "resolving"
                  ? " — Resolve before ordinary Save, or acknowledge duplication to start a new capture."
                  : ""}
              </p>
            ) : null}
            {unresolvedArchive.length > 0 ? (
              <div className="capture-archive" role="region" aria-label="Unresolved earlier captures">
                <p className="capture-byte-count">
                  Earlier unresolved capture{unresolvedArchive.length === 1 ? "" : "s"} (resolve-only; not abandoned):
                </p>
                <ul className="capture-archive-list">
                  {unresolvedArchive.map((item) => (
                    <li key={item.captureId}>
                      <span className="capture-byte-count">
                        {item.captureId} ({item.classification})
                      </span>{" "}
                      <button
                        type="button"
                        className="capture-secondary"
                        disabled={
                          captureBusy ||
                          item.canResolve === false ||
                          resolvingArchiveCaptureId === item.captureId
                        }
                        onClick={() => void resolveArchivedCapture(item.captureId)}
                      >
                        {resolvingArchiveCaptureId === item.captureId
                          ? "Resolving…"
                          : item.canResolve === false
                            ? "Receipt missing"
                            : "Resolve"}
                      </button>
                    </li>
                  ))}
                </ul>
              </div>
            ) : null}

          </section>

          <form className="search-form" onSubmit={search} role="search">
            <label className="visually-hidden" htmlFor="memory-search">Search your memory</label>
            <span className="search-glyph" aria-hidden="true">⌕</span>
            <input
              id="memory-search"
              ref={searchRef}
              value={query}
              onChange={(event) => {
                searchRequestGate.invalidate();
                setSearching(false);
                setQuery(event.target.value);
                setHits([]);
                setHoldReason(null);
                setNotice(null);
              }}
              placeholder="Search notes, people, or ideas"
              autoComplete="off"
              maxLength={1024}
            />
            <kbd className="shortcut-hint">{isMac ? "⌘" : "Ctrl"} K</kbd>
            <button className="search-button" type="submit" disabled={searching}>
              {searching ? "Searching" : "Search"}
            </button>
          </form>

          {holdCopy ? (
            <section className="safety-panel" role="alert" aria-live="assertive">
              <div className="panel-icon" aria-hidden="true">!</div>
              <div>
                <p className="panel-kicker">Retrieval paused</p>
                <p>{holdCopy}</p>
                {holdReason === "local_enrollment_required" ? (
                  <button
                    className="text-button"
                    type="button"
                    onClick={enrollThisDevice}
                    disabled={enrolling}
                  >
                    {enrolling ? "Setting up this device…" : "Set up this device"}
                  </button>
                ) : null}
              </div>
            </section>
          ) : null}

          {notice ? <p className="notice" role="status">{notice}</p> : null}
          {deviceId ? <p className="device-reference">This device · {deviceId}</p> : null}

          {timelineMode && !holdCopy && !notice ? (
            <section className="results timeline-results" aria-label="Recent notes" aria-live="polite">
              <div className="results-heading">
                <h2>Timeline</h2>
                <span>{timelineHits.length} {timelineHits.length === 1 ? "note" : "notes"}</span>
              </div>
              {detailHit ? (
                <article className="timeline-detail" aria-label="Note detail">
                  <div className="results-heading">
                    <h3>Note detail</h3>
                    <button type="button" className="text-button" onClick={() => setDetailHit(null)}>
                      Close
                    </button>
                  </div>
                  <div className="result-meta">
                    <time dateTime={detailHit.createdAt}>{displayTimelineDate(detailHit.createdAt)}</time>
                    <span>{detailHit.classification}</span>
                  </div>
                  <p>{detailHit.text}</p>
                  <dl className="timeline-detail-source">
                    <dt>Event</dt><dd>{detailHit.eventId}</dd>
                    <dt>Brain</dt><dd>{detailHit.brainId}</dd>
                    <dt>Device / sequence</dt><dd>{detailHit.deviceId} · {detailHit.deviceSeq}</dd>
                    <dt>Type</dt><dd>{detailHit.eventType}</dd>
                    <dt>Signing key</dt><dd>{detailHit.signingKeyId}</dd>
                    <dt>Content hash</dt><dd>{detailHit.contentHash}</dd>
                  </dl>
                </article>
              ) : null}
              {timelineHits.length === 0 && !timelineLoading ? (
                <p className="timeline-empty">No notes in this local timeline yet.</p>
              ) : null}
              <ul>
                {timelineHits.map((hit) => (
                  <li className="result-card" key={hit.eventId}>
                    <div className="result-meta">
                      <time dateTime={hit.createdAt}>{displayTimelineDate(hit.createdAt)}</time>
                      <span>{hit.classification}</span>
                    </div>
                    <p>{hit.text}</p>
                    <button
                      type="button"
                      className="text-button timeline-open-detail"
                      disabled={detailLoading}
                      onClick={() => void openEventDetail(hit.eventId)}
                    >
                      {detailLoading ? "Opening…" : "Open full note"}
                    </button>
                    <details className="timeline-source">
                      <summary>Source details</summary>
                      <dl>
                        <dt>Event</dt><dd>{hit.eventId}</dd>
                        <dt>Brain</dt><dd>{hit.brainId}</dd>
                        <dt>Device / sequence</dt><dd>{hit.deviceId} · {hit.deviceSeq}</dd>
                        <dt>Type</dt><dd>{hit.eventType}</dd>
                        <dt>Signing key</dt><dd>{hit.signingKeyId}</dd>
                        <dt>Content hash</dt><dd>{hit.contentHash}</dd>
                      </dl>
                    </details>
                  </li>
                ))}
              </ul>
              {timelineHasMore ? (
                <button
                  type="button"
                  className="capture-secondary timeline-more"
                  disabled={timelineLoading || !timelineCursor}
                  onClick={() => void loadTimeline(timelineCursor)}
                >
                  {timelineLoading ? "Loading…" : "Load older notes"}
                </button>
              ) : null}
            </section>
          ) : null}

          {!timelineMode && !holdCopy && !notice && hits.length === 0 ? (
            <section className="empty-state" aria-live="polite">
              <div className="empty-rule" />
              <p className="empty-title">A quiet place for what matters.</p>
              <p>Enter a phrase above to search your verified local notes.</p>
            </section>
          ) : null}

          {!timelineMode && hits.length > 0 ? (
            <section className="results" aria-label="Search results" aria-live="polite">
              <div className="results-heading">
                <h2>Notes</h2>
                <span>{hits.length} {hits.length === 1 ? "result" : "results"}</span>
              </div>
              <ul>
                {hits.map((hit) => (
                  <li className="result-card" key={hit.eventId}>
                    <div className="result-meta">
                      <time dateTime={hit.createdAt}>{displayDate(hit.createdAt)}</time>
                      <span>{hit.classification}</span>
                    </div>
                    <p>{hit.text}</p>
                  </li>
                ))}
              </ul>
            </section>
          ) : null}

          {notice?.includes("could not complete") ? (
            <p className="privacy-note">Previous results were cleared before this request.</p>
          ) : null}
        </div>

        <footer className="footer-note">
          <span>Private by design</span>
          <span className="footer-separator">·</span>
          <span>Stored and searched locally</span>
        </footer>

        {confirmDeliberateNew ? (
          <dialog
            ref={deliberateNewDialogRef}
            className="capture-confirm-dialog"
            aria-labelledby="capture-confirm-title"
            aria-describedby="capture-confirm-description"
            onCancel={(event) => {
              event.preventDefault();
              cancelDeliberateNewCapture();
            }}
          >
              <p className="panel-kicker">Unresolved capture</p>
              <h2 id="capture-confirm-title">Start another capture?</h2>
              <p id="capture-confirm-description">
                The earlier submission may still commit. Starting a new capture
                uses a new id and could create a duplicate note. The earlier
                capture will remain listed so you can resolve it later.
              </p>
              <div className="capture-confirm-actions">
                <button
                  className="capture-secondary"
                  type="button"
                  onClick={cancelDeliberateNewCapture}
                  autoFocus
                >
                  Cancel
                </button>
                <button
                  className="search-button"
                  type="button"
                  onClick={() => void confirmDeliberateNewCapture()}
                >
                  Start new capture
                </button>
              </div>
          </dialog>
        ) : null}
      </section>
    </main>
  );
}
