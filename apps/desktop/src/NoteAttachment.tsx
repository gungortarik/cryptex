import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";

export function NoteAttachment({ noteId, noteHash }: { noteId: string; noteHash: string }) {
  const [busy, setBusy] = useState(false);
  const [status, setStatus] = useState("");

  async function attach(file: File) {
    if (file.size > 512 * 1024) {
      setStatus("Choose a file of 512 KiB or less.");
      return;
    }
    setBusy(true);
    setStatus("");
    try {
      const bytes = Array.from(new Uint8Array(await file.arrayBuffer()));
      await invoke("m9_attach_file", { file: { noteId, noteHash, bytes } });
      setStatus("File attached. Reopen the note to see its verified link count.");
    } catch (error) {
      setStatus(error === "uncertain"
        ? "The save could not be confirmed. Reopen the note to check."
        : "Cryptex could not attach this file.");
    } finally {
      setBusy(false);
    }
  }

  return (
    <div>
      <label>Attach a file <input type="file" disabled={busy} onChange={(event) => {
        const file = event.currentTarget.files?.[0];
        event.currentTarget.value = "";
        if (file) void attach(file);
      }} /></label>
      <p role="status" aria-live="polite">{busy ? "Attaching file…" : status}</p>
    </div>
  );
}
