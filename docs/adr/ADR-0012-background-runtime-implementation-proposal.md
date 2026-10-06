# ADR-0012 implementation proposal — Desktop-owned runtime

**Status:** ACCEPTED for the scoped M0 implementation direction; mandatory production gates remain open
**Date:** 2026-10-03
**Scope:** P0-2 background runtime and atomic update lifecycle

## Source-record note

### Owner-authorized acceptance boundary revision — 2026-10-05

The owner answered "kurallar dahilinde onayliyorum" to the concrete proposal in
`evidence/m0/m0-acceptance-scope-decision-proposal-20261005.md`. This authorizes
preparing scoped ADR revisions only; it does not accept this ADR, complete M0,
authorize M1 or permit production distribution.

This revision separates M0 implementation-direction acceptance from mandatory
production acceptance. Earlier paragraphs and the original proof list below
retain historical context; the current acceptance sections at the end govern
this revised proposal. No accepted Master Spec or milestone criterion is removed.
M0 still requires its lifecycle/update spike and immutable synthetic fixture
safety, while production identity/event semantics remain M0 non-goals.

The original ADR-0012 source is absent from this repository. This scoped
proposal records the implementation direction selected from the P0-2 evidence;
it does not reconstruct the missing original ADR or mark the P0-2 gate complete.

## Context

P0-2 compares native per-user daemon supervision, a Desktop-owned supervisor,
and a justified hybrid. Test-only lifecycle, renderer reconnect, window-close,
login and synthetic update proofs exist on Windows and macOS. A Mac release
Desktop run now physically verifies Desktop-owned daemon startup/restart, login
startup, renderer IPC, Stop without restart, registration removal and cleanup.
Physical Mac tray Open/Quit also passed in `7b32222`; Windows release-mode physical verification,
signed packaging, compatible component updates, rollback and canonical-data
preservation also remain open. See the run evidence for the exact scope.

## Proposed decision

### Evidence status update — 2026-10-05

The original paragraphs/bullets retain their proposal-time historical scope.
Later Windows title-bar/tray Open/Quit, real hidden login/startup removal and
observed foreground console-free launch/query proofs are recorded in
`windows-physical-release-tray-20261003.md`,
`windows-desktop-login-result-20261003.md` and
`windows-console-visual-result-20261003.md` under `evidence/m0/p0-2/`.
The console-free foreground proof is not a repeated physical login proof.
Windows unsigned installed operator replacement/rollback also exists separately.

Changed native test-package selection and actual protocol-rejected rollback now
PASS on both platforms, preserving synthetic sealed fixtures after each scenario:
`windows-changed-native-package-evidence-20261005.md` and
`macos-changed-native-package-evidence-20261005.md`. Windows changed both role
hashes; Mac changed Desktop only. These results do not establish production
canonical-schema acceptance, signed installed updater safety or power-loss
durability. Mac native interruption/foundation parity was subsequently reported
PASS at `502abf4`; see
`evidence/m0/p0-2/macos-native-interruption-evidence-20261005.md` for exact source,
artifact hash, restored baseline health and cleanup. This scoped proof does not
accept this ADR or establish installed/signed or production canonical acceptance.
At that proposal-time evidence update the proposal remained PROPOSED; the scoped
owner acceptance is recorded in the current acceptance section below.

### Selected implementation direction

Use **Mode B with per-user login startup**:

- Cryptex Desktop is the per-user login-start application and the runtime
  supervisor while it is running.
- Desktop owns the `cryptexd` sidecar process. The daemon remains the only
  canonical writer; Desktop only supervises and communicates over the approved
  local IPC boundary.
- Closing the main window hides Desktop and leaves the supervisor/daemon alive.
  An explicit Quit action stops the daemon cleanly and exits Desktop.
- The Windows and macOS login-start adapters launch Desktop, never a separately
  registered daemon process.
- OS-specific startup registration and native tray/menu behavior stay behind
  platform adapters.

This is an implementation direction, not acceptance of ADR-0012 or authorization
to leave M0. Implementation must preserve the Master Spec and P0-2 contract.

## Alternatives and evidence

- **Native daemon supervision:** macOS LaunchAgent restart evidence passed.
  Windows per-user Run starts at login and a later ten-minute survival proof
  passed, but no-elevation crash restart is not established; `schtasks /Create`
  was denied in the tested user context.
- **Desktop-owned supervision:** process-level crash restart, single instance,
  window lifecycle and renderer reconnect have test-harness evidence on both
  systems. The Mac release Desktop physically owned a synthetic daemon through
  startup and crash/restart; physical tray Open and active-daemon Quit also
  passed. The equivalent Windows release-mode physical flow remains open.
  This is release-executable evidence, not
  production acceptance.
- **Hybrid:** no additional native daemon supervisor is proposed. Per-user OS
  startup launches Desktop; the Desktop supervisor owns the daemon lifecycle.

The detailed comparison is in
`evidence/m0/p0-2/supervision-candidate-comparison-20261003.md`.

## Original implementation/proof list — historical proposal context

1. Bundle a target-matched `cryptexd` sidecar and resolve it without a developer
   environment variable. Release package-smoke builds stage and resolve the
   sidecar on Windows and macOS; signed installer packaging remains open.
2. Start or safely attach to one daemon when Desktop starts; detect unexpected
   child exit, wait for lock release, restart with bounded retry, and reconnect
   clients. Do not restart after an explicit clean Quit. The Mac release run
   verified this lifecycle with a synthetic daemon; Windows physical release
   verification remains open.
3. Add native tray/menu actions for Open and Quit and verify close/hide behavior.
   Both Mac tray actions are physically verified, including Quit exit 0 with
   an active daemon. Windows physical tray Open and active-daemon Quit exit 0
   also passed in `windows-physical-release-tray-20261003.md`.
4. Register per-user login startup for Desktop with least privilege and provide
   a reversible user-facing setting or documented control. The Mac setting was
   physically enabled, exercised through real sign-out/sign-in, and removed;
   Windows login enable, real hidden sign-in and registration removal also
   passed; console-free visual behavior on the rebuilt Windows release remains
   open. See `windows-desktop-login-result-20261003.md` for the scope.
5. Prove actual login startup, crash/restart, duplicate rejection and shutdown
   on Windows and macOS using packaged builds. The Mac release-mode run passed
   these checks including tray Open/Quit; the Windows release-mode physical run
   remains open.
6. Prove a compatible Desktop+daemon package update and rollback with canonical
   data unchanged on both platforms.

This original list predates the owner-approved boundary revision. Its production
requirements remain mandatory in the separate production gates below. This
The proposal was PROPOSED at that point in its historical evidence record; the
current scoped M0 status is ACCEPTED as recorded in the acceptance section.

## Security and recovery constraints

- Runtime data remains in the OS application-support location, outside the Git
  checkout and away from visible home/Desktop/Documents folders by default.
- Desktop receives no canonical-write path or capability; all canonical writes
  continue through `cryptexd` and the approved IPC authorization boundary.
- Update failure must restore the previous compatible package and must not
  mutate or remove canonical events.
- Startup registration is per-user, least-privilege and removable; no elevated
  service or public endpoint is introduced.

## Acceptance state

### Owner acceptance — 2026-10-05

The owner confirmed the intended window/quit behavior: “Quit ile temiz
kapatalım. Çarpıya basınca tamamen kapanmasın app.” This selects the proposed
Mode B behavior: closing the main window hides Desktop and leaves its owned
supervisor running; explicit Quit cleanly stops the daemon and exits Desktop.
The owner had previously approved Mode B with per-user Desktop login startup
for implementation. This proposal is therefore **ACCEPTED for its scoped M0
implementation direction**, subject to the evidence and limits below.

This acceptance does not accept signed/installed production updates, canonical
schema/signature safety, power-loss durability, production secret storage,
distribution/notarization or the separate production residuals. Those remain
mandatory gates in their assigned milestones.

### Current M0 criteria and evidence

Evidence paths in this table are under `evidence/m0/p0-2/`.

| Criterion | Retained proof and exact limit |
| --- | --- |
| Target-matched sidecar and backend launch authority | Both-OS package-smoke/changed-package runs; `macos-sidecar-boundary-evidence-20261005.md` plus `../m0-security-notice-acceptance-review-20261005.md`: Windows 20/20, Mac 13/13 validator tests; no physical renderer invocation claim |
| Process ownership, crash restart, duplicate refusal, Stop and reconnect | `windows-release-desktop-owned-supervisor-20261003.md`, `macos-tauri-owned-supervisor-restart-evidence-20261003.md`, both-OS renderer reconnect evidence; combined dated slices, not one final installed build |
| Physical window/tray Open and active-daemon Quit | `windows-physical-release-tray-20261003.md`, Mac owned-supervisor and physical traffic-light evidence; retained PID through restore, clean exit/lock release |
| Real login and startup removal | `windows-desktop-login-result-20261003.md` and Mac owned-supervisor evidence; Windows login console defect retained separately from later correction |
| Console correction | `windows-console-visual-result-20261003.md`: foreground launch/query observed without new terminal; final package console-free login remains a production gate |
| Changed native package selection and protocol rejection/rollback | Both-OS `*-changed-native-package-evidence-20261005.md`; Desktop changed on both, daemon changed on Windows only; rejected candidate restored baseline |
| Interrupted updater recovery | Both-OS `*-native-interruption-evidence-20261005.md`; actual kill after selection before successor launch, baseline health and owned-process/lock/IPC cleanup |
| Immutable fixture safety | Changed-package/interruption reports preserve accepted-codec synthetic sealed packs/manifest; no real production event schema or personal data |

Before M0 ADR acceptance: review this table against retained evidence, complete
the scoped security/failure and notice-boundary review, obtain required final
cross-platform CI and human diff/ADR approval. The M0 contract and gate checklist
must independently pass before M1 begins. No unchecked production gate is PASS.

### Mandatory production gates retained

1. M1 OS secure storage/identity and M2 real event schema must preserve daemon-only
   writing, signatures/validation and immutable canonical data. Synthetic packs
   do not establish production canonical-data update/recovery safety.
2. M14 installed updater coordination must verify final packaged lifecycle,
   console-free login, compatible pair handover, explicit rollback and required
   interruption/durability boundaries. Canonical storage remains outside package
   rollback state. One SIGKILL boundary does not establish power-loss durability.
3. Signed/notarized release, publisher verification and final distribution notices
   remain mandatory before distribution under M17 release acceptance. M14
   packaging work is not signed release acceptance.
4. Before hardened production IPC APIs: resolve the synchronous whole-read
   deadline/stalled-peer limitation in the security review. Framing bounds bytes,
   not read latency. Accepted ADR-0007 retains its narrow health-only scope.
5. Preserve private app-data roots, least-privilege removable startup, no public
   endpoint and no arbitrary renderer shell/filesystem capability. Recovery/purge
   never become autonomous AI tools. Later work requires milestone authorization.
6. Independent security review at `3de9d00` also identifies the non-pilot
   filesystem-authorized developer `start.request` executable override. Remove/
   gate or constrain this test control before shipping product supervision.
   Backend validation of renderer requests does not remove this separate local
   test-control path. See `evidence/m0/m0-independent-review-results-20261005.md`.

M0 implementation direction accepted by the owner as recorded above. M0 overall
completion is governed by the foundation checklist; production gates above remain
mandatory and unverified until their assigned milestones.
