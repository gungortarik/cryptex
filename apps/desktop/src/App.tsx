import { invoke } from "@tauri-apps/api/core";
import { useEffect, useState } from "react";

/**
 * Temporary P0-2 Mode B developer controls.
 * Not product UI and not an ADR-0012 decision.
 */
export function App() {
  const [log, setLog] = useState("Desktop shell ready for foundation / P0-2 Mode B probes.");
  const [cryptexd, setCryptexd] = useState("");
  const [runtimeRoot, setRuntimeRoot] = useState("");

  useEffect(() => {
    void (async () => {
      try {
        const root = await invoke<string>("p0_2_runtime_root");
        setRuntimeRoot(root);
      } catch (error) {
        setLog(`runtime root failed: ${String(error)}`);
      }
      try {
        const path = await invoke<string>("p0_2_default_cryptexd");
        setCryptexd(path);
      } catch {
        // Operator must paste cryptexd path when CRYPTEXD_BIN is unset.
      }
    })();
  }, []);

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

  async function showWindow() {
    try {
      await invoke("p0_2_show_main_window");
      setLog("main window show requested");
    } catch (error) {
      setLog(`show failed: ${String(error)}`);
    }
  }

  async function snapshot() {
    try {
      const path = await invoke<string>("p0_2_write_snapshot", {
        note: "manual_dev_snapshot",
      });
      setLog(`snapshot written: ${path}`);
    } catch (error) {
      setLog(`snapshot failed: ${String(error)}`);
    }
  }

  async function quitClean() {
    try {
      await invoke("p0_2_quit_clean");
    } catch (error) {
      setLog(`quit failed: ${String(error)}`);
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
        {runtimeRoot ? <p className="status">runtime: {runtimeRoot}</p> : null}
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
          <button type="button" onClick={showWindow}>
            Show main window
          </button>
          <button type="button" onClick={snapshot}>
            Write snapshot
          </button>
          <button type="button" onClick={quitClean}>
            Quit clean
          </button>
        </div>
        <p className="status">
          Close the main window to exercise Mode B hide-on-close (process stays alive). Restore via
          Show, Dock reopen, or `show.request` under the runtime root.
        </p>
      </section>
    </main>
  );
}
