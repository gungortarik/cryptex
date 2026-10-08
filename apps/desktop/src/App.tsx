import { invoke } from "@tauri-apps/api/core";
import { FormEvent, useEffect, useRef, useState } from "react";

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

  async function search(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setSearching(true);
    setNotice(null);
    setHoldReason(null);
    // Never leave older plaintext visible while a new retrieval decision runs.
    setHits([]);
    try {
      const text = query.trim();
      const response = await invoke<GuardedSearchResponse>("p0_4_guarded_search", {
        query: { text: text || null, limit: 20 },
      });
      if (response.holdReason) {
        setConnection("attention");
        setHoldReason(response.holdReason);
        return;
      }
      setConnection("active");
      setHits(response.hits ?? []);
    } catch {
      setConnection("attention");
      setNotice("Cryptex could not complete a guarded search. No results were kept on screen.");
    } finally {
      setSearching(false);
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
        <div className="rail-bottom" aria-hidden="true">M4</div>
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

          <form className="search-form" onSubmit={search} role="search">
            <label className="visually-hidden" htmlFor="memory-search">Search your memory</label>
            <span className="search-glyph" aria-hidden="true">⌕</span>
            <input
              id="memory-search"
              ref={searchRef}
              value={query}
              onChange={(event) => {
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
