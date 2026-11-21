import { invoke } from "@tauri-apps/api/core";
import { useEffect, useRef, useState } from "react";

type EngineState = "running" | "stopped" | "unavailable";

export function EngineControls({ onChange }: { onChange: () => void }) {
  const [state, setState] = useState<EngineState>("unavailable");
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState("");
  const [confirmStop, setConfirmStop] = useState(false);
  const dialog = useRef<HTMLDialogElement>(null);
  const cancel = useRef<HTMLButtonElement>(null);
  const pending = useRef(false);
  useEffect(() => {
    let mounted = true;
    void invoke<EngineState>("m13_engine_control", { action: "inspect" })
      .then(value => { if (mounted) setState(value); })
      .catch(() => { if (mounted) setState("unavailable"); });
    return () => { mounted = false; };
  }, []);
  useEffect(() => {
    if (confirmStop) { dialog.current?.showModal(); cancel.current?.focus(); }
    else dialog.current?.close();
  }, [confirmStop]);
  async function control(action: "restart" | "stop" | "inspect") {
    if (pending.current) return;
    pending.current = true;
    setBusy(true);
    setConfirmStop(false);
    try {
      const result = await invoke<EngineState>("m13_engine_control", { action });
      setState(result);
      setMessage(result === "running" ? "Local engine is running." : result === "stopped" ? "Local engine stopped." : "Engine controls are unavailable.");
    } catch {
      setMessage("The engine state could not be confirmed. Check again.");
    } finally {
      onChange();
      pending.current = false;
      setBusy(false);
    }
  }
  if (state === "unavailable" && !message) return null;
  return <div className="engine-controls">
    <button type="button" className="capture-secondary" disabled={busy} onClick={() => void control("restart")}>Restart engine</button>
    <details><summary>Advanced engine controls</summary>
      <button type="button" disabled={busy || state !== "running"} onClick={() => setConfirmStop(true)}>Stop engine…</button>
    </details>
    {message && <p role="status">{message} <button type="button" className="text-button" disabled={busy} onClick={() => void control("inspect")}>Check engine state</button></p>}
    <dialog ref={dialog} className="capture-confirm-dialog" aria-labelledby="engine-stop-heading" onCancel={() => setConfirmStop(false)}>
      <h2 id="engine-stop-heading">Stop the local engine?</h2>
      <p>Saving and searching will be unavailable until you restart the engine.</p>
      <div className="capture-confirm-actions">
        <button type="button" ref={cancel} onClick={() => setConfirmStop(false)}>Cancel</button>
        <button type="button" onClick={() => void control("stop")}>Stop engine</button>
      </div>
    </dialog>
  </div>;
}
