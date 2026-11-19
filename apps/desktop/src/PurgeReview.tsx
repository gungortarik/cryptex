import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

type Note = { eventId: string; contentHash: string; text: string; classification: string };
type Outcome = { kind: "prepared"; ticket: string; remove_locally: boolean } | { kind: "cancelled" } | { kind: "confirmed" | "purged"; note_id: string; note_hash: string };

export function PurgeReview({ note, onPurged }: { note: Note; onPurged: () => void }) {
  const dialog = useRef<HTMLDialogElement>(null);
  const cancel = useRef<HTMLButtonElement>(null);
  const [ticket, setTicket] = useState("");
  const [removeLocally, setRemoveLocally] = useState(false);
  const [busy, setBusy] = useState(false);
  const [status, setStatus] = useState("");

  useEffect(() => {
    if (ticket) {
      dialog.current?.showModal();
      cancel.current?.focus();
    } else {
      dialog.current?.close();
    }
  }, [ticket]);

  async function prepare() {
    setBusy(true);
    setStatus("");
    try {
      const reply = await invoke<Outcome>("m10_purge_review", {
        action: { kind: "prepare", note_id: note.eventId, note_hash: note.contentHash },
      });
      if (reply.kind !== "prepared" || !reply.ticket) throw Error();
      setRemoveLocally(reply.remove_locally);
      setTicket(reply.ticket);
    } catch {
      setStatus("This note could not be verified for deletion review.");
    } finally {
      setBusy(false);
    }
  }

  async function finish(kind: "confirm" | "cancel") {
    if (!ticket || busy) return;
    setBusy(true);
    try {
      const reply = await invoke<Outcome>("m10_purge_review", { action: { kind, ticket } });
      if (reply.kind !== (kind === "cancel" ? "cancelled" : removeLocally ? "purged" : "confirmed")) throw Error();
      if ((reply.kind === "confirmed" || reply.kind === "purged") && (reply.note_id !== note.eventId || reply.note_hash !== note.contentHash)) throw Error();
      if (reply.kind === "purged") { onPurged(); return; }
      setStatus(kind === "cancel" ? "Deletion cancelled." : "Permanent deletion confirmed for this test note. Your approval was saved.");
    } catch {
      setStatus("Cryptex could not verify the decision. Review the note again.");
    } finally {
      setTicket("");
      setBusy(false);
    }
  }

  return (
    <div className="purge-review">
      <button type="button" className="text-button" disabled={busy} onClick={prepare}>Review permanent deletion</button>
      {status && <p role="status">{status}</p>}
      <dialog ref={dialog} className="capture-confirm-dialog" aria-labelledby="purge-title"
        onCancel={event => { event.preventDefault(); void finish("cancel"); }}>
        <h2 id="purge-title">Permanently delete this note?</h2>
        <p>{note.classification} · <code>{note.eventId}</code></p>
        <blockquote className="purge-note">{note.text}</blockquote>
        <p>Permanent deletion cannot be undone. Copies on unreachable devices or external backups may remain.</p>
        <p>{removeLocally ? "Confirm to remove the note from this temporary profile and its search index." : "This temporary profile saves a signed record of your approval. The note will stay stored."}</p>
        <div className="capture-confirm-actions">
          <button type="button" ref={cancel} disabled={busy} onClick={() => void finish("cancel")}>Cancel</button>
          <button type="button" className="purge-confirm" disabled={busy} onClick={() => void finish("confirm")}>Confirm permanent deletion</button>
        </div>
      </dialog>
    </div>
  );
}
