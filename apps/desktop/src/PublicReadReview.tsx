import { invoke } from "@tauri-apps/api/core";
import { useEffect, useRef, useState } from "react";

type Request = { kind: "review"; ticket: string; peer_pid: number };
type Outcome = Request | { kind: "empty" | "allowed" | "denied" };

export function PublicReadReview() {
  const [request, setRequest] = useState<Request | null>(null);
  const [message, setMessage] = useState("");
  const [busy, setBusy] = useState(false);
  const dialog = useRef<HTMLDialogElement>(null);
  const deny = useRef<HTMLButtonElement>(null);
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
    <button type="button" className="quiet-button" disabled={busy} onClick={() => void act("inspect")}>Review AI access</button>
    <dialog ref={dialog} className="capture-confirm-dialog" aria-labelledby="public-read-title" aria-describedby="public-read-scope"
      onCancel={event => { event.preventDefault(); void act("deny"); }}>
      <h3 id="public-read-title">Allow one public-note search?</h3>
      <p id="public-read-scope">Compare this code with the one in your MCP client. Approval lets that connection read up to ten public note excerpts for one search.</p>
      <p><code>{request?.ticket}</code></p>
      <p>Local bridge process {request?.peer_pid}</p>
      <button ref={deny} type="button" disabled={busy} onClick={() => void act("deny")}>Deny</button>
      <button type="button" disabled={busy} onClick={() => void act("allow")}>Allow one search</button>
    </dialog>
    <p role="status">{message}</p>
  </div>;
}
