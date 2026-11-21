import { invoke } from "@tauri-apps/api/core";
import { useEffect, useRef, useState } from "react";

type Selection = { old_file: string; old_digest: string; replacement_file: string; replacement_digest: string };
type Review = { kind: "review"; ticket: string; selection: Selection };
type Outcome = Review | { kind: "empty" | "kept" | "removed" };

export function BackupRemovalReview() {
  const [review, setReview] = useState<Review | null>(null);
  const [busy, setBusy] = useState(false);
  const [status, setStatus] = useState("");
  const dialog = useRef<HTMLDialogElement>(null);
  const keep = useRef<HTMLButtonElement>(null);
  useEffect(() => {
    if (review) {
      dialog.current?.showModal();
      keep.current?.focus();
    } else {
      dialog.current?.close();
    }
  }, [review]);

  async function inspect(prepare: boolean) {
    setBusy(true);
    setStatus("");
    try {
      if (prepare) {
        const result = await invoke<{ kind: string }>("m11_backup_writer", { mode: "prepare_removal", keep: null });
        if (result.kind === "cancelled") { setStatus("Selection cancelled."); return; }
        if (result.kind !== "removal_prepared") throw Error();
      }
      const result = await invoke<Outcome>("m11_backup_removal", { action: { kind: "inspect" } });
      if (result.kind === "review") setReview(result);
      else if (result.kind === "empty") setStatus("No backup is awaiting removal.");
      else throw Error();
    } catch {
      setStatus(prepare ? "Cryptex could not verify both scheduled backups for removal. Check the selected files and their checkpoints." : "Cryptex could not load the backup review.");
    } finally { setBusy(false); }
  }

  async function decide(kind: "keep" | "remove") {
    if (!review || busy) return;
    setBusy(true);
    try {
      const result = await invoke<Outcome>("m11_backup_removal", { action: { kind, ticket: review.ticket } });
      if (result.kind !== (kind === "keep" ? "kept" : "removed")) throw Error();
      setStatus(kind === "keep" ? "Backup kept." : "The selected backup was removed.");
    } catch (error) {
      setStatus(error === "expired" ? "This review has expired. Check for a current review."
        : "Cryptex could not confirm the outcome. Check the backup target before trying again.");
    } finally {
      setReview(null);
      setBusy(false);
    }
  }

  return <div>
    <button type="button" className="quiet-button" disabled={busy} onClick={() => void inspect(true)}>Select backups to compare</button>
    <button type="button" className="quiet-button" disabled={busy} onClick={() => void inspect(false)}>Review backup removal</button>
    {status && <p role="status" className="capture-help">{status}</p>}
    <dialog ref={dialog} className="capture-confirm-dialog" aria-labelledby="backup-removal-title"
      onCancel={event => { event.preventDefault(); void decide("keep"); }}>
      <h2 id="backup-removal-title">Remove this entire encrypted backup?</h2>
      <p>This permanently removes the older file from your backup target. Cryptex verified the replacement and checked that it preserves the other records.</p>
      {review && <dl className="backup-removal-files">
        <dt>File to remove</dt><dd><bdi>{review.selection.old_file}</bdi></dd>
        <dt>SHA-256</dt><dd><code>{review.selection.old_digest}</code></dd>
        <dt>Verified replacement to keep</dt><dd><bdi>{review.selection.replacement_file}</bdi></dd>
        <dt>SHA-256</dt><dd><code>{review.selection.replacement_digest}</code></dd>
      </dl>}
      <div className="capture-confirm-actions">
        <button ref={keep} type="button" className="quiet-button" disabled={busy} onClick={() => void decide("keep")}>Keep backup</button>
        <button type="button" className="capture-secondary" disabled={busy} onClick={() => void decide("remove")}>Remove this backup</button>
      </div>
    </dialog>
  </div>;
}
