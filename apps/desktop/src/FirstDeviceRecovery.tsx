import { invoke } from "@tauri-apps/api/core";
import { useRef, useState } from "react";

type Result = { kind: "challenge"; file: string; device: string; fingerprint: string }
  | { kind: "accepted" | "cancelled" };

export function FirstDeviceRecovery() {
  const pending = useRef(false);
  const [busy, setBusy] = useState(false);
  const [status, setStatus] = useState("");
  const [request, setRequest] = useState<Extract<Result, { kind: "challenge" }> | null>(null);
  async function exchange(importApproval: boolean) {
    if (pending.current) return;
    pending.current = true;
    setBusy(true);
    setRequest(null);
    setStatus("");
    try {
      const result = await invoke<Result>("m11_first_device", { import: importApproval });
      if (result.kind === "challenge") {
        setRequest(result);
        setStatus("Public recovery request saved.");
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
    <p>Save the request for your prepared replacement profile. In the offline Recovery Kit, compare the device ID and key fingerprint before signing.</p>
    <button type="button" disabled={busy} onClick={() => void exchange(false)}>Save public request</button>
    <button type="button" disabled={busy} onClick={() => void exchange(true)}>Import signed approval</button>
    {status && <p role="status">{status}</p>}
    {request && <dl>
      <dt>Request file</dt><dd>{request.file}</dd>
      <dt>New device ID</dt><dd>{request.device}</dd>
      <dt>New key fingerprint · SHA-256</dt><dd>{request.fingerprint}</dd>
    </dl>}
  </details>;
}
