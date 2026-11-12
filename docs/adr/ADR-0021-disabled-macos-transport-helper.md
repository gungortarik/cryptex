# ADR-0021 — Disabled macOS managed transport helper

Status: **ACCEPTED FOR DISABLED DEVELOPMENT AND ISOLATED TESTS ONLY.**
Date: 2026-11-12.

The owner was shown the [concrete helper proposal](../proposals/m8-macos-transport-identity-binding.md)
and delegated the choice: “HANGISI UYGUNSA IYI ISE ONU YAP”. The implementation
selects the recommended separate transport role of the existing signed daemon.
This is not production activation, a new publisher/support-floor choice or M8
completion. ADR-0020's ordinary Mac M8 gates remain closed.

## Decision

Behind the existing default-off local feature, the canonical daemon may capture
a child running that same independently verified executable in a transport-only
mode. The helper enters before ordinary lifecycle, IPC, signing-key and canonical
initialization. A separately purpose-bound native handshake authenticates both
actual supervised instances and the release/root/epoch. Neither a helper message
nor its read-only liveness pin confers owner-operation authority.

The helper owns an isolated pinned upstream Syncthing monitor/server tree in a
private session/group. The daemon retains its helper child unreaped through
cleanup; the helper watches its retained parent generation during blocking
engine work. Never adopt a process by PID/name/UID/REST claims. Read identity only
after checking the actual established socket and retained serving instance,
matching the private profile receipt, API credential and pinned engine version.
Each identity read requires a fresh bounded observation rather than a ready cache.

Keep profile/package/OS APIs in platform adapters and reuse existing native XML,
credential binding, owned-socket HTTP, bootstrap and canonical domain services.
Renderer APIs receive no shell, paths, ports, credentials or helper controls.

Clean exit, helper loss and daemon loss require separate evidence. An unresolved
or combined supervisor loss must retain an active-profile marker and refuse a
replacement; do not claim Windows Job equivalence or repair/adopt the old engine.
The alternative OS-managed service strategy needs a separate decision if this
candidate cannot meet the required lifecycle boundaries.

## Required evidence

Public SDK/actual socket and task-generation probes; real pinned 2.1.5 startup,
foreign-server refusal before credential bytes, same-profile reopen and missing
key refusal; single/combined supervisor loss and exclusion; native Desktop read,
offer/proof and approval binding; exact-source Mac/Windows CI; independent source
review; then a one-click owner Mac run. No current-source implementation or
successful execution is inferred merely from this accepted development scope.
