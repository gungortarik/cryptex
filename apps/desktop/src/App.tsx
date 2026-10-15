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
