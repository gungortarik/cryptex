import { invoke } from "@tauri-apps/api/core";
import { useState } from "react";

/**
 * Temporary P0-2 Mode B developer controls.
 * Not product UI and not an ADR-0012 decision.
 */
export function App() {
  const [log, setLog] = useState("Desktop shell ready for foundation / P0-2 Mode B probes.");
  const [cryptexd, setCryptexd] = useState("");

  async function startDaemon() {
    try {
      const pid = await invoke<number>("p0_2_start_daemon", { cryptexd });
      setLog(`daemon started pid=${pid}`);
    } catch (error) {
      setLog(`start failed: ${String(error)}`);
    }
  }

  async function status() {
    try {
      const text = await invoke<string>("p0_2_daemon_status");
      setLog(text || "(empty status)");
    } catch (error) {
      setLog(`status failed: ${String(error)}`);
    }
  }

  async function stopDaemon() {
    try {
      await invoke("p0_2_stop_daemon");
      setLog("daemon stop requested");
    } catch (error) {
      setLog(`stop failed: ${String(error)}`);
    }
  }

  return (
    <main className="foundation-shell">
      <section className="foundation-card" aria-labelledby="cryptex-title">
        <p className="eyebrow">M0 · Foundation · P0-2 probe</p>
        <h1 id="cryptex-title">Cryptex</h1>
        <p className="summary">
          Local-first personal memory and knowledge system for macOS and Windows.
        </p>
        <p className="status">{log}</p>
        <label>
          cryptexd path
          <input
            value={cryptexd}
            onChange={(event) => setCryptexd(event.target.value)}
            placeholder="/path/to/cryptexd"
          />
        </label>
        <div className="probe-actions">
          <button type="button" onClick={startDaemon}>
            Start probe daemon
          </button>
          <button type="button" onClick={status}>
            Daemon status
          </button>
          <button type="button" onClick={stopDaemon}>
            Stop daemon
          </button>
        </div>
        <p className="status">
          Close the main window to exercise Mode B hide-on-close (process stays alive).
        </p>
      </section>
    </main>
  );
}
