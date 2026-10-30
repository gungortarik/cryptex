# M8 canonical writer lock coverage audit — 2026-10-25

## Scope and method

Read-only source audit at `ae61603b517f68b63d559ccb5ee77d744176e6a6` plus
the current uncommitted M8 documentation changes. Searched Rust sources for
`append_frame`, `seal_active_segment`, `recover_active_open`, and canonical
event path helpers, then inspected the production call sites and daemon entry
points. No source or runtime data was changed. No tests or Actions were run.

## Findings

- Production calls to the segment mutation primitives
  (`append_frame`, `seal_active_segment`, `recover_active_open`) are in
  `services/cryptexd/src/canonical_writer.rs`.
- The public canonical append function obtains
  `acquire_device_canonical_lock`; capture's receipt-backed path obtains that
  same device lock before calling `append_memory_note_with_lock`.
- The lock helper acquires `canonical/events/inventory.lock` before the
  per-device `writer.lock`; multiple-device acquisition sorts DeviceIds by
  their byte representation and rejects duplicates.
- Projection, knowledge materialization, and verified-read paths inspect
  canonical segments and acquire the corresponding inventory/device locks;
  their production code does not call the segment mutation primitives.
- Other direct segment mutation matches found by the source search are in
  tests. Storage crate mutation APIs remain public, so this source audit does
  not prevent a future caller from bypassing the daemon lock convention.
- Import staging still has no canonical publication function. Its prepared
  recovery takes inventory/device locks only to validate and abort a
  pre-publication transaction. Therefore importer participation in the shared
  writer lock set is not yet demonstrated, and no claim is made about
  publication serialization, policy validation, or replay.

## Disposition

Existing daemon production segment mutation paths are covered by the shared
lock convention at this source revision. The audit narrows the open lock
coverage gate to a future importer publisher and to guarding the storage API
against new out-of-convention callers. It does not close any critical M8
publication, trust/cutoff, approval/bootstrap, macOS parity, or independent
review gate. M8 remains incomplete; transport and trust activation remain
disabled.
