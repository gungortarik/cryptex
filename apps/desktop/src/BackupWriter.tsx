import { invoke } from "@tauri-apps/api/core";
import { useRef, useState } from "react";

type WriterMode = "create" | "inspect" | "schedule" | "pause" | "retain" | "verify"
  | "share_writer" | "request_writer" | "release_writer" | "accept_writer";
type Result = { kind: "created"; folder: string } | { kind: "local_writer" }
  | { kind: "scheduled" | "paused" | "cancelled" | "verified" | "retention_saved" }
  | { kind: "handoff"; exchange: { kind: "saved"; file: string } | { kind: "accepted" } };

export function BackupWriter() {
  const pending = useRef(false);
  const [busy, setBusy] = useState(false);
  const [status, setStatus] = useState("");

  const [keep, setKeep] = useState("");
  const count = keep === "" ? null : Number(keep);
  const validCount = count === null || (Number.isSafeInteger(count) && count > 0);

  async function choose(mode: WriterMode) {
    if (pending.current) return;
    pending.current = true;
    setBusy(true);
    setStatus("");
    try {
      const result = await invoke<Result>("m11_backup_writer", { mode, keep: mode === "retain" ? count : null });
      if (result.kind === "created") setStatus(`Assigned this device as writer for ${result.folder}.`);
      else if (result.kind === "handoff") {
        setStatus(result.exchange.kind === "saved"
          ? `Saved ${result.exchange.file} in the selected backup target.`
          : "This device now holds the writer role for the selected target.");
      }
      else if (result.kind === "local_writer") setStatus("This device is the designated writer for the selected target.");
      else if (result.kind === "scheduled") setStatus("Daily backup enabled. The first backup runs when both folders are available.");
      else if (result.kind === "paused") setStatus("Scheduled backups paused.");
      else if (result.kind === "verified") setStatus("Scheduled backup verified by restoring it in temporary storage.");
      else if (result.kind === "retention_saved") setStatus(count === null ? "All scheduled backups will be kept." : `Retention saved. Cryptex will keep the newest ${count} verified scheduled backups.`);
      else setStatus("Selection cancelled.");
    } catch (error) {
      const messages: Record<string, string> = {
        authority: "This device could not prove writer authority for the selected target.",
        unsafe_target: "Choose a backup folder outside the profile. To check a target, select its Cryptex backup folder.",
        history: "Cryptex could not verify the signed writer history.",
        unavailable: "Backup writer settings are unavailable for this profile.",
        original_required: "Use manual export for a recovered device. Select its original encrypted backup and checkpoint to include both histories.",
        verification: "Cryptex could not complete scheduled backup verification and retention. Check the generation and recovery files.",
        recipient: "Select and confirm the Recovery Kit’s public certificate before scheduling backups.",
        busy: "Another backup operation is in progress.",
        uncertain: "Cryptex could not confirm the assignment. Select Check writer for the target to verify it.",
      };
      setStatus(messages[String(error)] ?? "Cryptex could not confirm the backup writer.");
    } finally {
      pending.current = false;
      setBusy(false);
    }
  }

  return <section className="vault-panel" aria-labelledby="backup-writer-heading">
    <div><h2 id="backup-writer-heading">Scheduled backup writer</h2>
      <p className="capture-help">Assign this device to a new filesystem target or check its authority for an existing target.</p>
    </div>
    <button type="button" className="search-button" disabled={busy} onClick={() => void choose("create")}>Create backup target</button>
    <button type="button" disabled={busy} onClick={() => void choose("inspect")}>Check writer</button>
    <details>
      <summary>Transfer the writer role</summary>
      <p>Use the same backup target on both devices. Share the assignment from the current writer, then request the role on the new device. Review and sign on the current writer, then accept the grant on the new device.</p>
      <button type="button" disabled={busy} onClick={() => void choose("share_writer")}>Share current assignment</button>
      <button type="button" disabled={busy} onClick={() => void choose("request_writer")}>Request writer role</button>
      <button type="button" disabled={busy} onClick={() => void choose("release_writer")}>Review writer transfer</button>
      <button type="button" disabled={busy} onClick={() => void choose("accept_writer")}>Accept signed grant</button>
    </details>
    <button type="button" disabled={busy} onClick={() => void choose("schedule")}>Enable daily backup</button>
    <button type="button" disabled={busy} onClick={() => void choose("pause")}>Pause scheduled backups</button>
    <label>Verified scheduled backups to keep
      <input type="number" min="1" step="1" placeholder="Keep all" value={keep} disabled={busy} onChange={e => setKeep(e.target.value)} />
    </label>
    <button type="button" disabled={busy || !validCount} onClick={() => void choose("retain")}>Save retention</button>
    <button type="button" disabled={busy} onClick={() => void choose("verify")}>Verify scheduled backup</button>
    {status && <p className="capture-status" role="status">{status}</p>}
  </section>;
}
