# M8 readiness and initial slice map

**Date:** 2026-10-19  
**Product baseline:** `74bc3b14127962579ae03838bafea9336612a033`  
**Branch:** `codex/m8-device-trust`  
**Windows worktree:** `C:/Users/Tarik/.codex/worktrees/m8-device-trust/cryptex`

## Authority and sequence

The owner accepted the M7 deferral/M8 scheduling exception recorded in
`evidence/m7/m7-temporary-deferral-and-m8-sequencing-proposal-20261019.md`.
M7 stays OPEN / DEFERRED; M6 visualization remains available. M8 scope follows
its existing contract and accepted sync/trust specifications. Actions remain
unauthorized. M8 has started; it is not complete.

## Foundation audit

| M8 requirement | Existing foundation | Remaining production work |
|---|---|---|
| Explicit identity and pairing | ADR-0015, core `PairingRecord`, daemon `pairing_store.rs`, explicit local enrollment in `local_device_identity.rs` | Cryptographically bound peer trust and pairing workflow; a persisted `Trusted` enum is not peer authentication |
| Managed transport | Accepted ADR-0004 addendum, M0 P0-4 evidence, `tools/m0-lifecycle-compare/scripts/syncthing-*.mjs`, third-party 2.1.5 notices | Shipped target-specific executable, daemon-owned profile/process/REST controller and OS SecureStore integration; test scripts are not a production controller |
| Record integrity | `crates/events/src/validate_record.rs`, `versioned_reader.rs`, storage `segment_io.rs` | Guarded incoming replication pipeline bound to explicit peer trust; file arrival is not event acceptance |
| Trusted key resolution | `TrustedDaemonKeyResolver`, `LocalDeviceKeyResolver` | Local resolver intentionally recognizes only the enrolled local DeviceId/key; do not broaden it to arbitrary transported metadata |
| Sequence cutoff | Core sequence types and daemon `sequence_runtime.rs` / `sequence_store.rs` | Revoke/retire/compromise transitions and accepted cutoff enforcement; wall-clock values remain display-only |
| Safe publication / retrieval | Canonical inventory/per-device locks and `RetrievalSafetyState` in `knowledge_materialization.rs` | Incoming acceptance must join the existing lock/safety protocol before exposing new records |
| Replication protection status | Lifecycle health exists but is not replication protection | Typed truthful replication status that does not infer protection from pairing declarations or health |

The accepted M0/M1/M2 prerequisites have recorded completion evidence in the ADR
index and handoff. This audit is source/evidence inspection, not a rerun of
their tests or a claim that M8 product behavior already exists.

## Slice 1 candidate: read-only replication status

Add `cryptexd replication status --runtime-root <absolute-app-data-path>`.
Inspect existing public Brain, local-device and pairing metadata without
creating or repairing any runtime files, identities, keys, or network
connections. The command does not read canonical memory bodies or SecureStore.

Report `protection=not_configured` and mark the transport controller, replication
peer authentication, and incoming replication pipeline as `not_implemented`.
Count stored pairing states under `declared_trusted`, not authenticated peers.
Metadata presence alone does not prove that private signing material is usable.

Corrupt, unsupported, foreign-Brain or misnamed pairing metadata returns a
redacted typed error. Bound individual reads to 64 KiB and directory inventory
to 4096 entries; these are diagnostic limits, not protocol/device limits.
Output omits Brain/device identifiers, paths, secrets, and metadata contents.

The inventory is not atomic and must never be used to authorize peers/events.
This slice does not implement pairing, transport, remote event admission, or
physical path-containment guarantees. No existing writer/authorizer changes.

## Next slices

1. Specify the authenticated peer-key binding and trust/cutoff transitions
   against the accepted event model before adding a durable trust mutation.
   Record any genuinely unspecified wire/serialization decision explicitly.
2. Implement that reviewed daemon-owned trust boundary with targeted failure,
   replay, foreign-Brain, and sequence-cutoff coverage.
3. Integrate the pinned managed transport behind OS adapters, preserving
   isolated profiles, bounded authenticated REST, secrets and process ownership.
4. Wire guarded incoming validation/publication and offline convergence.
5. Add pairing UI and truthful protection status, then verify relevant
   Windows/macOS native and failure scenarios and required completion gates.

These are work slices, not independent completion claims. No later slice may
weaken canonical immutability, the local-only resolver's current protection,
incoming validation, or explicit user-controlled trust.
