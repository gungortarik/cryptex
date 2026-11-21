import { invoke } from "@tauri-apps/api/core";
import { useRef, useState } from "react";

type ExportResult = { kind: "cancelled" } | { kind: "written"; backup: string; checkpoint: string };

export function ManualBackup() {
  const pending = useRef(false);
  const [busy, setBusy] = useState(false);
  const [status, setStatus] = useState("");
  const [verifying, setVerifying] = useState(false);

  async function verifyBackup() {
    if (pending.current) return;
    pending.current = true;
    setVerifying(true);
    setStatus("");
    try {
      const verified = await invoke<boolean>("m11_verify_manual_backup");
      setStatus(verified ? "Backup verified by restoring it in temporary storage." : "Verification cancelled.");
    } catch (error) {
      setStatus(String(error) === "unavailable" ? "Backup verification is unavailable for this profile." :
        "Verification failed. Check the backup, its independent checkpoint, and the Recovery Kit and unlock file.");
    } finally {
      pending.current = false;
      setVerifying(false);
    }
  }

  async function exportBackup() {
    if (pending.current) return;
    pending.current = true;
    setBusy(true);
    setStatus("");
    try {
      const result = await invoke<ExportResult>("m11_export_backup");
      setStatus(result.kind === "cancelled" ? "Export cancelled." :
        `Backup written as ${result.backup}. Keep ${result.checkpoint} for recovery. Restore verification is pending.`);
    } catch (error) {
      const messages: Record<string, string> = {
        invalid_recipient: "Cryptex could not verify the Recovery Kit certificate for this profile.",
        unsafe_destination: "Choose two separate folders outside the Cryptex profile.",
        unavailable: "Manual backup is unavailable for this profile.",
        history_held: "Cryptex could not verify the complete profile history for export.",
        busy: "Another manual export is in progress.",
        encryption_failed: "Cryptex could not encrypt this backup.",
        uncertain: "Cryptex could not confirm the export. Check both selected folders.",
      };
      setStatus(messages[String(error)] ?? "Cryptex could not complete the export.");
    } finally {
      pending.current = false;
      setBusy(false);
    }
  }

  return <section className="vault-panel" aria-labelledby="manual-backup-heading">
    <div>
      <h2 id="manual-backup-heading">Encrypted backup</h2>
      <p className="capture-help">Choose a backup folder and a separate trusted folder for the recovery checkpoint.</p>
    </div>
    <button className="search-button" type="button" disabled={busy || verifying} onClick={() => void exportBackup()}>
      {busy ? "Exporting…" : "Export encrypted backup"}
    </button>
    <button type="button" disabled={busy || verifying} onClick={() => void verifyBackup()}>
      {verifying ? "Verifying…" : "Verify manual backup"}
    </button>
    {status && <p className="capture-status" role="status">{status}</p>}
  </section>;
}
