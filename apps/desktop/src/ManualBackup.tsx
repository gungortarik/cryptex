import { invoke } from "@tauri-apps/api/core";
import { useRef, useState } from "react";

type ExportResult = { kind: "cancelled" } | { kind: "written"; backup: string; checkpoint: string };

export function ManualBackup() {
  const pending = useRef(false);
  const [busy, setBusy] = useState(false);
  const [status, setStatus] = useState("");

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
    <button className="search-button" type="button" disabled={busy} onClick={() => void exportBackup()}>
      {busy ? "Exporting…" : "Export encrypted backup"}
    </button>
    {status && <p className="capture-status" role="status">{status}</p>}
  </section>;
}
