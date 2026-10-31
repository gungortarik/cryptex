# ADR-0018 — Local Device Identity for M4 Retrieval

**Status:** ACCEPTED — owner approved 2026-10-08
**Date:** 2026-10-08
**Milestone:** M4 — Knowledge Engine

## 1. Context

The Master Spec §9 and accepted M4 contract require Desktop search results to
obey the daemon-owned `RetrievalSafetyState`. The daemon safety evaluator and
guarded M3 search API exist, but Desktop has no content IPC operation and the
production daemon cannot currently construct the trusted-device inventory and
verification-key resolver required to evaluate canonical history.

ADR-0015 distinguishes `install_id` from the stable trusted `device_id`, but
does not define local device enrollment. Pairing, remote trust, retirement and
incoming-device validation remain M8. Trust must not be inferred from a
directory name, event-provided key, OS machine GUID, Syncthing ID, hardware
serial, or a pairing display state.

This decision defines only the local identity boundary needed for M4 Desktop
retrieval. It does not revise ADR-0015's device-trust model or implement M8.

## 2. Decision

1. Keep the M4 Desktop-retrieval acceptance gate. Do not defer it to M8.
2. During explicit Brain setup, `cryptexd` creates one durable local `DeviceId`
   for this installation and binds it to the existing ADR-0016 Ed25519 signing
   identity through the SecureStore seed and matching public metadata. It must
   not use an OS- or transport-provided identifier.
3. Enrollment is exclusive and durable. Repeating setup succeeds only when the
   existing local identity and key binding match. Missing, corrupt, partial, or
   conflicting state fails closed; it never silently mints a replacement.
4. The M4 production trusted set contains only this locally enrolled device.
   Any other discovered device stream without an M8-authorized key/trust record
   causes a fail-closed Brain-wide retrieval hold. No remote trust is inferred.
5. Add a bounded, read-only Desktop search IPC operation. It accepts a typed
   search request, invokes the daemon guard, and returns typed results or a
   typed hold/error. Desktop never opens canonical files or SQLite directly.
   A Brain-wide hold returns no content.
6. Production v2 writing remains disabled. This decision does not implement
   pairing, replication, retirement, downgrade prevention, Obsidian export, or
   M8/M10/M14 behavior.

## 3. Security and compatibility invariants

- `cryptexd` remains the only component that reads canonical event packs and
  returns guarded content. Desktop uses only the bounded IPC operation.
- Same-user IPC authorization is necessary but does not replace the canonical
  signature/key checks or `RetrievalSafetyState` evaluation.
- The guard is evaluated against a complete, locked canonical inventory; any
  newly discovered untrusted stream or stale/unknown safety state withholds the
  whole Brain.
- Event keys are resolved only from daemon-owned identity metadata bound to
  SecureStore-held key material. A public key carried by an event is never a
  trust root.
- Existing v1 bytes, validation, integrity semantics, M3 tables, and M4
  derived-state rebuild behavior remain unchanged. No production v2 writer is
  enabled.
- The user's Windows and macOS machines are separate platform test targets;
  their ownership alone does not create a cryptographic trust relationship or
  imply that cross-device replication is implemented.

## 4. Required implementation and verification if accepted

- First local enrollment, restart/reopen, concurrent enrollment, Brain
  mismatch, key mismatch, partial/corrupt identity, and no-silent-replacement
  tests.
- An additional unknown device stream causes Brain-wide hold; no directory or
  event key is auto-trusted.
- Bounded IPC framing, query validation, response limits, read-only capability
  policy, unauthorized-client refusal, and no content leakage on errors.
- RetrievalSafetyState allow, hold, missing, corrupt, stale, inventory-change,
  and Brain-wide hold cases through the actual Desktop-facing daemon operation.
- Windows/macOS semantic equivalence; v1 golden vectors and all existing M2/M3
  regressions unchanged; production v2 writing remains disabled.

## 5. Scope boundaries

This draft does not define cross-device enrollment or prove that two physical
machines are trusted peers. It does not create a pairing UX, transport
controller, retirement/revocation workflow, trusted-device capability roster,
or downgrade protection. Those remain under the applicable M8 and ADR-0017
production activation gates.

## 6. Alternatives

| Alternative | Disposition |
|---|---|
| Defer Desktop retrieval to M8 | Rejected by this draft because it changes the accepted M4 completion gate; would require a separate owner-approved milestone revision. |
| Trust every discovered device stream | Rejected; directory presence and self-asserted keys are not trust proof. |
| Let Desktop query SQLite/canonical files directly | Rejected; bypasses daemon authority and the retrieval guard. |
| Reuse `install_id` as `device_id` | Rejected by ADR-0015; their lifetimes and roles differ. |

## 7. Acceptance and limits

The owner accepted this local-only M4 boundary on 2026-10-08. This authorizes
the narrow M4 work above only; it does not enable production v2 writing or
authorize M8/M10/M14 implementation.

## 8. Cross-checks

- `docs/00_MASTER_SPEC.md` §9: RetrievalSafetyState guards M3 and Desktop
  search.
- `docs/milestones/M4_KNOWLEDGE_ENGINE.md` scope, implementation step 5, and
  verification requirements: Desktop results are an M4 acceptance item.
- ADR-0015 §§2.2, 2.4: install/device IDs differ; device identity is stable
  once enrolled; pairing/trust workflows are M8.
- ADR-0016 §§6.2–6.3: local event-signing seed and public key metadata are
  bound and fail closed; they are not a remote-device trust registry.
- ADR-0017 §§5.4 and 8: retrieval safety is daemon-owned; M4 completion is
  separate from v2 activation and does not authorize M8 implementation.
- `evidence/m4/m4-desktop-retrieval-integration-blocker-20261008.md`: current
  code and IPC gaps.
