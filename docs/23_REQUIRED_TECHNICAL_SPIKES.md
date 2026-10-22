# 23 — Required Technical Spikes

These are not optional research tasks. They resolve the remaining architecture decisions capable of causing major mid-project rework.

## P0-1 — Physical Event Storage Layout

**Status:** RESOLVED — ADR-0013 ACCEPTED (sealed per-device segments; one-file-per-event rejected; hybrid not selected)

### Question
Does one immutable file per event remain practical at 10k/100k/500k events with Syncthing, antivirus/indexing and rebuild workloads on Windows/macOS?

### Compare
A. one file per event  
B. immutable sealed per-device segment/pack files  
C. hybrid thresholded segments if justified

### Measure
- event append latency
- directory enumeration
- initial Syncthing scan/sync
- incremental sync
- rebuild speed
- disk overhead
- crash behavior
- purge complexity
- corruption blast radius

### Acceptance
Select the simplest option that meets scale/recovery goals on both platforms. Record ADR-0013 as ACCEPTED before M2.

**Recorded:** ADR-0013 is ACCEPTED. P0-2, P0-3 and P0-4 are unchanged by this result.

---

## P0-2 — Background Runtime / Atomic Update Lifecycle

### Question
How does `cryptexd` remain available after window close, restart after crash/login, enforce single instance and update safely with Desktop on both OSes?

### Compare
- native per-user background registration (LaunchAgent + Windows per-user startup strategy)
- Tauri/tray supervisor spawning daemon sidecar
- justified hybrid if necessary

### Must prove
- close main window; engine remains available as designed
- reboot/login startup
- crash detection/restart
- one daemon instance
- Desktop reconnect
- update replaces compatible Desktop + daemon without canonical-data damage
- rollback/error path

### Acceptance
ADR-0012 accepted before M1/M2.

---

## P0-3 — Local IPC Transport

### Question
Which implementation cleanly supports Unix-domain socket + Windows named pipe semantics, user-only access, protocol handshake and testability?

### Must prove
- current-user-only permissions/ACL
- short safe macOS socket path
- Windows named-pipe security
- protocol version negotiation
- Desktop/CLI/MCP client roles
- daemon restart/reconnect
- malformed/oversized request handling

### Acceptance
Exact library/implementation captured in ADR-0007 addendum before production daemon APIs.

---

## P0-4 — Managed Syncthing Integration

### Question
Can Cryptex reliably supervise/configure a pinned Syncthing binary as an implementation detail without exposing unnecessary complexity?

### Must prove
- sidecar/process lifecycle on both OSes
- isolated Cryptex Syncthing config/home
- REST API authentication
- add/pair folder/device flow
- pause/resume/status
- conflict/versioning visibility
- upgrade strategy compatibility
- third-party license/notices
- no user-global Syncthing config collision

### Acceptance
ADR-0004 receives implementation addendum and chosen packaging before replication milestone.

---

## P1 spikes before corresponding feature

### Backup encryption/container
Select audited high-level format/library, test large objects, interrupted backup, restore and key recovery.

### Obsidian write-back
Prove safe event listening, `Vault.process` style changes, projection-loop suppression, and IPC from the plugin. Also prove physical containment of managed-note reads/writes on Windows and macOS, including symlink/reparse-point cases. Keep managed-note write-back disabled until containment is proven through a Vault-API-compliant design; metadata-only preflight and direct-filesystem fallback do not close this gate.

### Attachment projection
Benchmark hard-link versus copy fallback behavior and cleanup on both OSes.

### UI shell/native-feeling behavior
Before broad screen work, prove native menu/tray/titlebar/window behaviors and light/dark/system-font approach on both OSes.
