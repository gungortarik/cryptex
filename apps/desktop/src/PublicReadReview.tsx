import { invoke } from "@tauri-apps/api/core";
import { useEffect, useRef, useState } from "react";

type Request = { kind: "review"; ticket: string; peer_pid: number };
type Outcome = Request | { kind: "empty" | "allowed" | "denied" };
type Access = { kind: "requests"; paused: boolean; maintenance?: boolean };

export function PublicReadReview({ connectionMode }: { connectionMode?: string }) {
  const [request, setRequest] = useState<Request | null>(null);
  const [message, setMessage] = useState("");
  const [busy, setBusy] = useState(false);
  const [paused, setPaused] = useState<boolean | null>(null);
  const [maintenance, setMaintenance] = useState(false);
  const accessGeneration = useRef(0);
  const changingAccess = useRef(false);
  const dialog = useRef<HTMLDialogElement>(null);
  const deny = useRef<HTMLButtonElement>(null);
  useEffect(() => {
    let disposed = false;
    async function refresh() {
      if (changingAccess.current || document.visibilityState === "hidden") return;
      const seq = ++accessGeneration.current;
      try {
        const view = await invoke<Access>("m12_public_read_review", { action: { kind: "status" } });
        if (!disposed && seq === accessGeneration.current) {
          setPaused(view?.kind === "requests" && typeof view.paused === "boolean" ? view.paused : null);
          setMaintenance(view?.maintenance === true);
        }
      } catch {
        if (!disposed && seq === accessGeneration.current) setPaused(null);
      }
    }
    void refresh();
    const timer = window.setInterval(refresh, 15000);
    window.addEventListener("focus", refresh);
    return () => { disposed = true; window.clearInterval(timer); window.removeEventListener("focus", refresh); };
  }, [connectionMode]);
  async function changeAccess() {
    if (busy || changingAccess.current || paused === null) return;
    changingAccess.current = true;
    ++accessGeneration.current;
    setBusy(true);
    try {
      const view = await invoke<Access>("m12_public_read_review", { action: { kind: paused ? "resume" : "pause" } });
      if (view.kind !== "requests" || typeof view.paused !== "boolean") throw Error();
      setPaused(view.paused);
      setRequest(null);
      setMessage("");
    } catch {
      setPaused(null);
      setMessage("The service could not confirm the AI access setting.");
    } finally { changingAccess.current = false; setBusy(false); }
  }
  useEffect(() => {
    if (request) { dialog.current?.showModal(); deny.current?.focus(); }
    else dialog.current?.close();
  }, [request]);
  async function act(kind: "inspect" | "deny" | "allow") {
    if (busy || (kind !== "inspect" && !request)) return;
    setBusy(true);
    try {
      const outcome = await invoke<Outcome>("m12_public_read_review", {
        action: kind === "inspect" ? { kind } : { kind, ticket: request!.ticket },
      });
      setRequest(outcome.kind === "review" ? outcome : null);
      setMessage({ empty: "No client is waiting for approval.", review: "", allowed: "One public-note search approved.",
        denied: "Public-note access denied." }[outcome.kind]);
    } catch {
      setRequest(null);
      setMessage("This request could not be confirmed. Check for a current request.");
    } finally { setBusy(false); }
  }
  return <div>
    <button type="button" className="quiet-button" disabled={busy || paused === true} onClick={() => void act("inspect")}>Review AI access</button>
    {paused !== null && !maintenance && <button type="button" className="capture-secondary" disabled={busy} onClick={() => void changeAccess()}>
      {paused ? "Resume AI access" : "Pause AI access for this session"}
    </button>}
    {paused && <p role="status">{maintenance ? "AI access is paused by Maintenance Mode." : "AI access is paused until you resume it or restart the local service."}</p>}
    <dialog ref={dialog} className="capture-confirm-dialog" aria-labelledby="public-read-title" aria-describedby="public-read-scope"
      onCancel={event => { event.preventDefault(); void act("deny"); }}>
      <h2 id="public-read-title">Allow one public-note search?</h2>
      <p id="public-read-scope">Compare this code with the one in your MCP client. Approval lets that connection read up to ten public note excerpts for one search.</p>
      <p><code className="public-read-ticket">{request?.ticket}</code></p>
      <p>Local bridge process {request?.peer_pid}</p>
      <div className="capture-confirm-actions">
        <button ref={deny} className="capture-secondary" type="button" disabled={busy} onClick={() => void act("deny")}>Deny</button>
        <button className="capture-secondary" type="button" disabled={busy} onClick={() => void act("allow")}>Allow one search</button>
      </div>
    </dialog>
    <p role="status">{message}</p>
  </div>;
}
