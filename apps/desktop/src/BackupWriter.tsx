import { invoke } from "@tauri-apps/api/core";
import { useRef, useState } from "react";

type Result = { kind: "created"; folder: string } | { kind: "local_writer" } | { kind: "cancelled" };

export function BackupWriter() {
  const pending = useRef(false);
  const [busy, setBusy] = useState(false);
  const [status, setStatus] = useState("");

  async function choose(create: boolean) {
    if (pending.current) return;
    pending.current = true;
    setBusy(true);
    setStatus("");
    try {
      const result = await invoke<Result>("m11_backup_writer", { create });
      if (result.kind === "created") setStatus(`Assigned this device as writer for ${result.folder}.`);
      else if (result.kind === "local_writer") setStatus("This device is the designated writer for the selected target.");
      else setStatus("Selection cancelled.");
    } catch (error) {
      const messages: Record<string, string> = {
        authority: "This device could not prove writer authority for the selected target.",
        unsafe_target: "Choose a backup folder outside the profile. To check a target, select its Cryptex backup folder.",
        history: "Cryptex could not verify the signed writer history.",
        unavailable: "Backup writer settings are unavailable for this profile.",
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
    <button type="button" className="search-button" disabled={busy} onClick={() => void choose(true)}>Create backup target</button>
    <button type="button" disabled={busy} onClick={() => void choose(false)}>Check writer</button>
    {status && <p className="capture-status" role="status">{status}</p>}
  </section>;
}
