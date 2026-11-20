import { invoke } from "@tauri-apps/api/core";
import { useEffect, useRef, useState } from "react";

type Selection = { note_id: string; note_hash: string; state_hash: string };
type Choice = { target: Selection; preview: string; created_at: string };
type Reply =
  | { kind: "ready"; target: Selection | null }
  | { kind: "forgotten"; choices: Choice[]; next: string | null }
  | { kind: "changed" };

const request = (action: object) => invoke<Reply>("m10_note_lifecycle", { action });

export function NoteLifecycle({ note, onChanged }: {
  note?: { eventId: string; contentHash: string };
  onChanged: (action: "forget" | "restore") => void;
}) {
  const [ready, setReady] = useState(false);
  const [target, setTarget] = useState<Selection | null>(null);
  const [page, setPage] = useState<Extract<Reply, { kind: "forgotten" }> | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const mounted = useRef(false);
  const id = note?.eventId;
  const hash = note?.contentHash;
  useEffect(() => {
    let current = true;
    mounted.current = true;
    request({ kind: "inspect", note_id: id ?? null }).then(reply => {
      if (!current || reply.kind !== "ready") return;
      setReady(true);
      if (reply.target && reply.target.note_id === id && reply.target.note_hash === hash) setTarget(reply.target);
    }).catch(reason => {
      if (current && String(reason) !== "disabled") setError("Note visibility controls are unavailable.");
    });
    return () => { current = false; mounted.current = false; };
  }, [id, hash]);

  async function list(after: string | null) {
    setBusy(true);
    setError("");
    try {
      const reply = await request({ kind: "list", after });
      if (reply.kind !== "forgotten") throw Error();
      if (mounted.current) setPage(reply);
    } catch {
      if (mounted.current) { setPage(null); setError("Forgotten notes could not be loaded."); }
    } finally { if (mounted.current) setBusy(false); }
  }

  async function change(kind: "forget" | "restore", selection: Selection) {
    setBusy(true);
    setError("");
    try {
      const reply = await request({ kind, target: selection });
      if (reply.kind !== "changed") throw Error();
      if (mounted.current) { setPage(null); onChanged(kind); }
    } catch {
      if (mounted.current) {
        setPage(null);
        setTarget(null);
        setError("The change could not be verified. Reopen the timeline to check the note.");
      }
    } finally { if (mounted.current) setBusy(false); }
  }

  if (!ready && !error) return null;
  return <div className="note-lifecycle">
    {error && <p role="status">{error}</p>}
    {note ? target && <button type="button" className="text-button" disabled={busy} onClick={() => void change("forget", target)}>Forget note</button>
      : ready && <button type="button" className="text-button" aria-expanded={page !== null} disabled={busy}
        onClick={() => page ? setPage(null) : void list(null)}>Forgotten notes</button>}
    {page && <section className="forgotten-picker" aria-label="Forgotten notes">
      {page.choices.length === 0 && <p>No forgotten notes.</p>}
      {page.choices.map(choice => <article key={choice.target.note_id} className="result-card">
        <div className="result-meta"><time dateTime={choice.created_at}>{new Date(choice.created_at).toLocaleString()}</time></div>
        <p>{choice.preview}</p>
        <button type="button" className="text-button" disabled={busy} onClick={() => void change("restore", choice.target)}>Restore note</button>
      </article>)}
      {page.next && <button type="button" className="text-button" disabled={busy} onClick={() => void list(page.next)}>Next page</button>}
    </section>}
  </div>;
}
