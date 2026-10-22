// Observation-only M7 P1 spike. Managed-note IO disabled by owner decision (2026-10-16).
const { Plugin, Notice, apiVersion } = require("obsidian");

// These endpoints mirror cryptexd/src/ipc.rs. This probe never creates them.
function healthEndpoint(runtime) {
  if (runtime.platform === "win32") return "\\\\.\\pipe\\cryptex-cryptexd-v1";
  if (runtime.platform === "darwin") {
    const uid = runtime.getuid?.();
    if (Number.isSafeInteger(uid) && uid >= 0) return `/tmp/cpx-cryptex-${uid}/s`;
  }
  throw new Error("unsupported observation-probe platform");
}

module.exports = class CryptexM7Spike extends Plugin {
  async onload() {
    this.scopeAccepted = false;
    const root = this.app.vault.adapter.getBasePath?.();
    // Synthetic-scope opt-in only. This is not a physical containment check.
    if (typeof root !== "string" || !root.replaceAll("\\", "/").split("/").includes("Cryptex-Dev")) {
      new Notice("M7 observation probe requires an isolated Cryptex-Dev test vault.");
      return;
    }
    this.scopeAccepted = true;
    this.stats = { modify: 0, create: 0, rename: 0, delete: 0 };
    void this.writeEvidence('event-counts', this.stats);
    this.activeIpcSockets = new Set();
    this.unloaded = false;
    this.status = this.addStatusBarItem();
    this.status.setText("M7 spike: waiting for layout");
    this.startupRunId = Date.now();
    this.layoutReadyObserved = false;
    this.startupCounts = { create: 0, modify: 0, rename: 0, delete: 0 };
    this.startupEvents = [];
    for (const eventName of Object.keys(this.startupCounts)) {
      this.registerEvent(this.app.vault.on(eventName, (file) => {
        if (!this.unloaded && !this.layoutReadyObserved && this.isManaged(file)) {
          this.startupCounts[eventName] += 1;
          if (this.startupEvents.length < 32) this.startupEvents.push({ event: eventName, path: file.path, kind: file.constructor?.name || "unknown" });
        }
      }));
    }
    this.app.workspace.onLayoutReady(() => {
      if (this.unloaded) return;
      this.layoutReadyObserved = true;
      void this.writeEvidence("startup-events-" + this.startupRunId, { scope: "synthetic-only", counts: { ...this.startupCounts }, events: this.startupEvents.slice() });
      this.attachEvents();
    });

    this.addCommand({
      id: "m7-spike-ipc-health",
      name: "M7 Spike: local Cryptex health IPC",
      callback: () => this.healthProbe(),
    });
    this.addCommand({
      id: "m7-spike-ipc-unload",
      name: "M7 Spike: IPC unload cleanup probe",
      callback: () => this.ipcUnloadProbe(),
    });
    this.addCommand({
      id: "m7-spike-counts",
      name: "M7 Spike: show event counts",
      callback: () => new Notice(JSON.stringify(this.stats)),
    });
    await this.recordRuntimeInventory();
  }

  async recordRuntimeInventory() {
    const runtime = window.require("process");
    const bounded = (value) => typeof value === "string" ? value.slice(0, 64) : null;
    await this.writeEvidence("runtime-inventory", {
      schema_version: 1,
      scope: "synthetic-only-observation",
      obsidian_api_version: bounded(apiVersion),
      platform: bounded(runtime.platform),
      arch: bounded(runtime.arch),
      electron_version: bounded(runtime.versions?.electron),
      node_version: bounded(runtime.versions?.node),
      api_types_package: "none: plain CommonJS runtime probe",
      temporary_manifest_min_app: bounded(this.manifest?.minAppVersion),
      product_baseline_selected: false,
    });
  }

  async writeEvidence(name, value) {
    if (!this.scopeAccepted) return;
    const target = `.obsidian/plugins/cryptex-m7-spike/${name}.json`;
    await this.app.vault.adapter.write(target, JSON.stringify(value) + "\n");
  }
  attachEvents() {
    if (!this.scopeAccepted || this.unloaded || this.eventsAttached) return;
    this.eventsAttached = true;
    this.registerEvent(this.app.vault.on("modify", (file) => this.onModify(file)));
    this.registerEvent(this.app.vault.on("create", (file) => this.onOther("create", file)));
    this.registerEvent(this.app.vault.on("rename", (file) => this.onOther("rename", file)));
    this.registerEvent(this.app.vault.on("delete", (file) => this.onOther("delete", file)));
    this.status.setText("M7 spike: observation only; note IO disabled");
    console.info("M7_SPIKE listeners_ready");
  }

  isManaged(file) {
    return this.scopeAccepted && typeof file?.path === "string" && file.path.startsWith("Cryptex/");
  }

  async onModify(file) {
    if (this.unloaded || !this.isManaged(file)) return;
    this.stats.modify += 1;
    this.updateStatus();
    console.info("M7_SPIKE managed_modify_observed; origin not classified");
  }

  onOther(kind, file) {
    if (this.unloaded || !this.isManaged(file)) return;
    this.stats[kind] += 1;
    this.updateStatus();
    console.info("M7_SPIKE managed_" + kind + "_observed");
  }

  updateStatus() {
    this.status.setText(`M7 observe m:${this.stats.modify}; note IO disabled`);
    void this.writeEvidence("event-counts", this.stats);
  }

  async processProbe() {
    new Notice("Managed-note reads and writes are disabled pending containment proof.");
  }

  async manualConflictProbe() {
    new Notice("Managed-note reads and writes are disabled pending containment proof.");
  }

  async cleanupManualMarker() {
    new Notice("Managed-note reads and writes are disabled pending containment proof.");
  }

  async junctionMetadataProbe() {
    new Notice("Managed-note reads and writes are disabled pending containment proof.");
  }

  async junctionBoundaryProbe() {
    new Notice("Managed-note reads and writes are disabled pending containment proof.");
  }
  async lifecycleProbe() {
    new Notice("Managed-note reads and writes are disabled pending containment proof.");
  }

  async concurrentProcessProbe() {
    new Notice("Managed-note reads and writes are disabled pending containment proof.");
  }
  async healthProbe() {
    if (!this.scopeAccepted || this.unloaded) return;
    try {
      const net = window.require("net");
      const endpoint = healthEndpoint(window.require("process"));
      const requestId = `obsidian-m7-${Date.now()}`;
      const request = { protocol_version: 1, request_id: requestId, client_role: "mcp", capabilities: ["status.read"], operation: "health.ping" };
      const result = await new Promise((resolve, reject) => {
        const socket = net.createConnection({ path: endpoint });
        this.activeIpcSockets.add(socket);
        let buffer = Buffer.alloc(0);
        let pending = true;
        const timer = setTimeout(() => socket.destroy(new Error("timeout")), 2000);
        socket.once("connect", () => {
          if (this.unloaded) socket.destroy();
          else socket.write(JSON.stringify(request) + "\n");
        });
        socket.on("data", (chunk) => {
          if (!pending) return;
          if (buffer.length + chunk.length > 65536) {
            socket.destroy(new Error("oversized frame"));
            return;
          }
          buffer = Buffer.concat([buffer, chunk]);
          const end = buffer.indexOf(10);
          if (end < 0) return;
          try {
            const value = JSON.parse(buffer.subarray(0, end).toString("utf8"));
            pending = false;
            clearTimeout(timer);
            // We have a complete newline-framed status response. Destroy the
            // transport rather than half-closing only its write side: some
            // peers keep their write side open, which otherwise retains this
            // client socket until the timeout (observed by the macOS fixture).
            socket.destroy();
            resolve(value);
          } catch (error) {
            socket.destroy(error);
          }
        });
        socket.once("error", (error) => { pending = false; clearTimeout(timer); reject(error); });
        socket.once("close", () => {
          clearTimeout(timer);
          this.activeIpcSockets.delete(socket);
          if (pending) {
            pending = false;
            reject(new Error(this.unloaded ? "unloaded" : "closed before response"));
          }
        });
      });
      if (result?.protocol_version !== 1 || result?.request_id !== requestId || result?.operation !== "health.pong" || result?.accepted !== true) {
        throw new Error("response mismatch");
      }
      await this.writeEvidence("ipc-result", { ok: true, operation: result.operation, daemon_pid: result.daemon_pid });
      if (!this.unloaded) new Notice("Cryptex health IPC PASS (status only).");
    } catch (_) {
      await this.writeEvidence("ipc-result", { ok: false });
      if (!this.unloaded) new Notice("Cryptex health IPC unavailable; no note data sent.");
    }
  }

  async ipcUnloadProbe() {
    if (!this.scopeAccepted || this.unloaded) return;
    const net = window.require("net");
    const endpoint = healthEndpoint(window.require("process"));
    const requestId = "obsidian-m7-ipc-unload-" + Date.now();
    const request = { protocol_version: 1, request_id: requestId, client_role: "mcp", capabilities: ["status.read"], operation: "health.ping" };
    const result = await new Promise((resolve) => {
      const socket = net.createConnection({ path: endpoint });
      const state = { scope: "synthetic-transport-fixture", request_id: requestId, connected: false, request_sent: false, response_received: false, closed: false, closed_on_unload: false };
      this.activeIpcSockets.add(socket);
      const timer = setTimeout(() => { state.timed_out = true; socket.destroy(); }, 60000);
      socket.once("connect", () => { state.connected = true; socket.write(JSON.stringify(request) + "\n"); state.request_sent = true; });
      socket.on("data", () => { state.response_received = true; socket.destroy(); });
      socket.once("error", (error) => { state.error_code = error.code || "error"; });
      socket.once("close", () => {
        clearTimeout(timer);
        state.closed = true;
        state.closed_on_unload = this.unloaded === true && !state.response_received;
        this.activeIpcSockets.delete(socket);
        void this.writeEvidence("ipc-unload-" + requestId, state);
        resolve(state);
      });
    });
    if (!this.unloaded) new Notice(result.closed_on_unload ? "Pending IPC socket closed on plugin unload." : "IPC unload probe completed; inspect bounded result.");
    console.info("M7_SPIKE_ipc_unload", JSON.stringify(result));
  }

  onunload() {
    this.unloaded = true;
    for (const socket of this.activeIpcSockets || []) socket.destroy();
    this.status?.setText("M7 spike: unloaded");
    console.info("M7_SPIKE unloaded");
  }
};


