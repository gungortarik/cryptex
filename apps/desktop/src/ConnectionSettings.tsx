import { BackupRemovalReview } from "./BackupRemovalReview";
import { PublicReadReview } from "./PublicReadReview";
import { invoke } from "@tauri-apps/api/core";
import { useEffect, useRef, useState } from "react";
import { connectionMessage, connectionUpdate, parseConnectionView, type ConnectionView } from "./connection-settings.mjs";

export function ConnectionSettings({ active, onApply }: { active: boolean; onApply: () => void }) {
  const [open, setOpen] = useState(false);
  const [view, setView] = useState<ConnectionView | null>(null);
  const [paused, setPaused] = useState(true);
  const [listen, setListen] = useState("");
  const [addresses, setAddresses] = useState<Record<string, string>>({});
  const [busy, setBusy] = useState(false);
  const [uncertain, setUncertain] = useState(false);
  const [dirty, setDirty] = useState(false);
  const [message, setMessage] = useState<string | null>(null);
  const generation = useRef(0);
  const inFlight = useRef(false);
  const authorityUnavailable = view?.error === "local_restricted" || view?.error === "authority" || view?.error === "held";
  const readOnly = busy || !active || !view || view.state === "not_enrolled" || authorityUnavailable;
  const edit = () => { generation.current += 1; setDirty(true); if (!uncertain) setMessage(null); };

  function adopt(next: ConnectionView) {
    setView(next); setPaused(next.paused); setListen(next.listenAddress ?? "");
    setAddresses(Object.fromEntries(next.devices.map(device => [device.deviceId, device.endpoint ?? ""])));
    setDirty(false); setUncertain(false);
  }
  async function load() {
    if (!active || inFlight.current) return;
    const token = ++generation.current;
    inFlight.current = true; setBusy(true);
    try {
      const next = parseConnectionView(await invoke("m8_connection_settings", { update: null }));
      if (token === generation.current) { adopt(next); setMessage(null); }
    } catch {
      if (token === generation.current) { setView(null); setMessage("Connection settings are unavailable. Retry after the local service is ready."); }
    } finally { inFlight.current = false; setBusy(false); }
  }
  async function save() {
    if (!view || readOnly || uncertain || inFlight.current) return;
    let update;
    try { update = connectionUpdate(view, paused, listen, addresses); }
    catch (error) { setMessage(error instanceof Error ? error.message : "Check the LAN addresses."); return; }
    const token = ++generation.current;
    inFlight.current = true; setBusy(true); setMessage("Applying settings and checking this device’s connection…");
    onApply();
    try {
      const next = parseConnectionView(await invoke("m8_connection_settings", { update }));
      if (token === generation.current) { adopt(next); setMessage(next.error ? null : "Settings saved. " + connectionMessage(next)); }
    } catch {
      if (token === generation.current) { setUncertain(true); setMessage("The result is uncertain. Reload current settings before making another change; your draft is still shown."); }
    } finally { inFlight.current = false; setBusy(false); }
  }
  useEffect(() => {
    generation.current += 1;
    if (!active) { setView(null); setMessage(null); }
    if (open && active) void load();
    return () => { generation.current += 1; };
  }, [active, open]);

  useEffect(() => {
    if (!open || !active || dirty || uncertain) return;
    const timer = window.setInterval(() => { if (!inFlight.current) void load(); }, 5000);
    return () => window.clearInterval(timer);
  }, [open, active, dirty, uncertain]);

  return <section className="bootstrap-panel connections-panel" aria-labelledby="connections-heading">
    <div className="connections-heading">
      <div><p className="panel-kicker">Settings · Devices</p><h2 id="connections-heading">Local connections</h2></div>
      <button type="button" className="quiet-button" aria-expanded={open} onClick={() => setOpen(!open)}>{open ? "Close settings" : "Connection settings"}</button>
    </div>
    <p className="capture-help">Connect trusted devices on your private network. Local memory stays available while sync is paused.</p>
    {open && <div className="bootstrap-form connections-form">
      {view && <>
        {!message && <p role="status" className="capture-help">{dirty ? "Unsaved connection settings." : connectionMessage(view)}</p>}
        <label className="connections-pause"><input type="checkbox" checked={paused} disabled={readOnly} onChange={e => { edit(); setPaused(e.target.checked); }} /> Pause sync</label>
        <label htmlFor="connection-local-ip">This device’s LAN IPv4 address</label>
        <div className="connections-local-address"><input id="connection-local-ip" placeholder="192.168.1.20" maxLength={15} value={listen} disabled={readOnly} onChange={e => { edit(); setListen(e.target.value); }} />
          <span>Transport port <strong>{view.localTransportPort ?? "Unavailable"}</strong></span></div>
        <p className="capture-help">The transport port belongs to this device’s enrolled profile and cannot be changed here.</p>
        <div className="connections-devices">
          <h3>Trusted devices</h3>
          {view.devices.length === 0 && <p className="capture-help">{view.state === "attention" ? "No verified device list is available." : "No eligible devices. Complete a signed pairing to add one."}</p>}
          {view.devices.map(device => <div className="connections-device" key={device.deviceId}>
            <label htmlFor={`endpoint-${device.deviceId}`}>Device <code>{device.deviceId}</code></label>
            <p className="capture-help connections-identity">Transport identity <code>{device.transportId}</code></p>
            <input id={`endpoint-${device.deviceId}`} placeholder="192.168.1.21:22000" maxLength={21} value={addresses[device.deviceId] ?? ""} disabled={readOnly} onChange={e => { edit(); setAddresses(previous => ({ ...previous, [device.deviceId]: e.target.value })); }} />
          </div>)}
        </div>
      </>}
      <div className="connections-actions">
        <button type="button" className="quiet-button" disabled={busy || !active} onClick={() => void load()}>{busy ? "Working…" : "Reload current settings"}</button>
        {view && <button type="button" className="search-button" disabled={readOnly || !dirty || uncertain} onClick={() => void save()}>Save connection settings</button>}
      </div>
      {active && <BackupRemovalReview />}
      {active && <PublicReadReview />}
      {message && <p role="status" className="capture-status">{message}</p>}
      {!active && <p role="status" className="capture-status">Connection settings are unavailable until the local service responds.</p>}
    </div>}
  </section>;
}
