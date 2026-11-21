import { invoke } from "@tauri-apps/api/core";
import { useRef, useState } from "react";

type Archive = { brain: string; kit: string; backup_sha256: string };
type Result = ({ kind: "challenge"; file: string; device: string; fingerprint: string } & Archive)
  | ({ kind: "archive_verified" } & Archive)
  | { kind: "accepted" | "prepared" | "cancelled" };

export function FirstDeviceRecovery() {
  const pending = useRef(false);
  const [busy, setBusy] = useState(false);
  const [status, setStatus] = useState("");
  const [request, setRequest] = useState<Extract<Result, { kind: "challenge" }> | null>(null);
  const [archive, setArchive] = useState<Archive | null>(null);
  async function exchange(action: "inspect" | "prepare" | "export" | "import") {
    if (pending.current) return;
    pending.current = true;
    setBusy(true);
    setRequest(null);
    setArchive(null);
    setStatus("");
    try {
      const result = action === "prepare" ? await invoke<Result>("m11_prepare_replacement") :
        await invoke<Result>("m11_first_device", { import: action === "import", inspect: action === "inspect" });
      if (result.kind === "archive_verified") {
        setArchive(result);
        setStatus("Recovery Kit and backup verified.");
      } else if (result.kind === "challenge") {
        setRequest(result);
        setArchive(result);
        setStatus("Public recovery request saved.");
      } else if (result.kind === "prepared") {
        setStatus("Replacement profile prepared. Save its public request to continue.");
      } else {
        setStatus(result.kind === "accepted" ? "Signed approval verified for this device." : "Selection cancelled.");
      }
    } catch (error) {
      setStatus(String(error) === "no_pending_recovery" ? "This profile has no prepared recovery request." :
        String(error) === "unavailable" ? "First-device recovery is unavailable for this profile." :
        "Cryptex could not verify the selected recovery files. Check the request and the selected folder.");
    } finally {
      pending.current = false;
      setBusy(false);
    }
  }
  return <details data-first-device-recovery>
    <summary>Recover the first device</summary>
    <p>Disconnect this computer before opening the Recovery Kit. Select your backup, checkpoint, Kit and unlock file to prepare the blank profile. Save the public request, compare the identifiers in the offline signing tool, then import its signed approval here.</p>
    <button type="button" disabled={busy} onClick={() => void exchange("inspect")}>Check Kit and backup</button>
    <button type="button" disabled={busy} onClick={() => void exchange("prepare")}>Prepare replacement profile</button>
    <button type="button" disabled={busy} onClick={() => void exchange("export")}>Save public request</button>
    <button type="button" disabled={busy} onClick={() => void exchange("import")}>Import signed approval</button>
    {status && <p role="status">{status}</p>}
    {archive && <dl>
      <dt>Brain ID</dt><dd>{archive.brain}</dd>
      <dt>Recovery Kit ID</dt><dd>{archive.kit}</dd>
      <dt>Backup SHA-256</dt><dd>{archive.backup_sha256}</dd>
    </dl>}
    {request && <dl>
      <dt>Request file</dt><dd>{request.file}</dd>
      <dt>New device ID</dt><dd>{request.device}</dd>
      <dt>New key fingerprint · SHA-256</dt><dd>{request.fingerprint}</dd>
    </dl>}
  </details>;
}
