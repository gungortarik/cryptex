import { invoke } from "@tauri-apps/api/core";
import { FormEvent, useEffect, useMemo, useRef, useState } from "react";
import {
  CLASSIFICATIONS,
  MAX_CAPTURE_TEXT_BYTES,
  createCaptureSubmissionGate,
  interpretCaptureWire,
  newCaptureId,
  shouldClearDraftAfterCommit,
  utf8ByteLength,
  validateCaptureDraft,
} from "./capture-policy.mjs";
import { createSearchRequestGate, normalizeSearchText } from "./search-policy.mjs";

type SearchHit = {
  eventId: string;
  createdAt: string;
  classification: string;
  text: string;
};

type GuardedSearchResponse = {
  hits: SearchHit[] | null;
  holdReason: string | null;
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

export function App() {
  const isMac = navigator.platform.toLowerCase().includes("mac");
  const [query, setQuery] = useState("");
  const [hits, setHits] = useState<SearchHit[]>([]);
  const [holdReason, setHoldReason] = useState<string | null>(null);
  const [connection, setConnection] = useState<ConnectionState>("checking");
  const [searching, setSearching] = useState(false);
  const [enrolling, setEnrolling] = useState(false);
  const [deviceId, setDeviceId] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const searchRef = useRef<HTMLInputElement>(null);
  const searchRequestGate = useRef(createSearchRequestGate()).current;
  const captureRequestGate = useRef(createCaptureSubmissionGate()).current;
  const [draftText, setDraftText] = useState("");
  const [classification, setClassification] = useState("");
  const [captureUi, setCaptureUi] = useState<CaptureUiState>({ kind: "idle" });
  const [pendingSnapshot, setPendingSnapshot] = useState<CaptureSnapshot | null>(null);
  const [captureNotice, setCaptureNotice] = useState<string | null>(null);
  const draftDirtyRef = useRef(false);
  const liveDraftRef = useRef({ text: "", classification: "" });
  const byteCount = useMemo(() => utf8ByteLength(draftText), [draftText]);
  const captureBusy =
    captureUi.kind === "submitting" || captureUi.kind === "resolving";

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
  }, [draftText, classification]);

  useEffect(() => {
    draftDirtyRef.current =
      draftText.trim().length > 0 ||
      pendingSnapshot !== null ||
      captureBusy ||
      captureUi.kind === "uncertain";
  }, [draftText, pendingSnapshot, captureBusy, captureUi]);

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

  function applyCaptureResponse(response: CaptureResponse, snapshot: CaptureSnapshot, generation: number) {
    if (!captureRequestGate.isCurrent(generation)) return;
    const interpreted = interpretCaptureWire(response);
    const message = interpreted.message;

    if (interpreted.isCommitted) {
      const live = liveDraftRef.current;
      if (
        shouldClearDraftAfterCommit({
          pendingSnapshot: snapshot,
          liveText: live.text,
          liveClassification: live.classification,
          responseCaptureId: response.captureId ?? snapshot.captureId,
        })
      ) {
        setDraftText("");
        setClassification("");
      }
      setPendingSnapshot(null);
      setCaptureUi({ kind: "committed", message });
      setCaptureNotice(null);
      return;
    }

    if (interpreted.isReceiptMissing) {
      setCaptureUi({
        kind: "uncertain",
        captureId: snapshot.captureId,
        message,
        canResolve: false,
      });
      return;
    }

    if (interpreted.isCaptureIdConflict || interpreted.isRejected) {
      setCaptureUi({
        kind: "rejected",
        message: interpreted.isCaptureIdConflict
          ? `${message} A deliberate new submission will use a new capture id.`
          : message,
        canRetryAsNew: true,
      });
      return;
    }

    setCaptureUi({
      kind: "uncertain",
      captureId: snapshot.captureId,
      message,
      canResolve: true,
    });
  }

  async function submitCapture(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (captureBusy) return;

    const validationError = validateCaptureDraft({
      text: draftText,
      classification,
    });
    if (validationError) {
      setCaptureUi({ kind: "rejected", message: validationError, canRetryAsNew: false });
      return;
    }

    const captureId = newCaptureId();
    const snapshot: CaptureSnapshot = {
      captureId,
      text: draftText,
      classification,
    };
    const generation = captureRequestGate.begin();
    setPendingSnapshot(snapshot);
    setCaptureNotice(null);
    setCaptureUi({ kind: "submitting", captureId });

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
      const detail = error instanceof Error ? error.message : String(error);
      setCaptureUi({
        kind: "uncertain",
        captureId,
        message: `${detail} Outcome is uncertain after transport failure. The draft was kept. Use Resolve.`,
        canResolve: true,
      });
    }
  }

  async function resolvePendingCapture() {
    if (!pendingSnapshot || captureBusy) return;
    if (captureUi.kind === "uncertain" && !captureUi.canResolve) {
      setCaptureNotice(
        "ReceiptMissing must not auto-resubmit. Start a deliberate new submission with a new capture id if you accept possible duplication.",
      );
      return;
    }
    const snapshot = pendingSnapshot;
    const generation = captureRequestGate.begin();
    setCaptureUi({ kind: "resolving", captureId: snapshot.captureId });
    try {
      const response = await invoke<CaptureResponse>("p0_5_resolve_capture", {
        body: { captureId: snapshot.captureId },
      });
      applyCaptureResponse(response, snapshot, generation);
    } catch (error) {
      if (!captureRequestGate.isCurrent(generation)) return;
      const detail = error instanceof Error ? error.message : String(error);
      setCaptureUi({
        kind: "uncertain",
        captureId: snapshot.captureId,
        message: `${detail} Resolve failed; draft kept. Outcome remains uncertain.`,
        canResolve: true,
      });
    }
  }

  function discardDraft() {
    if (captureBusy) return;
    if (
      draftDirtyRef.current &&
      !window.confirm(
        "Discard unsaved or unresolved capture input? Drafts are not written to disk in this slice.",
      )
    ) {
      return;
    }
    captureRequestGate.invalidate();
    setDraftText("");
    setClassification("");
    setPendingSnapshot(null);
    setCaptureUi({ kind: "idle" });
    setCaptureNotice(null);
  }

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
        <button className="rail-action is-selected" type="button" aria-label="Search memory">
          <span aria-hidden="true">⌕</span>
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
              </p>
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

          {!holdCopy && !notice && hits.length === 0 ? (
            <section className="empty-state" aria-live="polite">
              <div className="empty-rule" />
              <p className="empty-title">A quiet place for what matters.</p>
              <p>Enter a phrase above to search your verified local notes.</p>
            </section>
          ) : null}

          {hits.length > 0 ? (
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
      </section>
    </main>
  );
}
