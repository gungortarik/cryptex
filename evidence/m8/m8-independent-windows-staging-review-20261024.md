# M8 independent Windows staging review

**Date:** 2026-10-24
**Review source:** Owner-supplied Cursor Security Review, `503403fb-76d0-4ae5-b467-031677cef57f`
**Scope:** Working-tree Windows staging/recovery slice and M8 protocol proposal
**Recommendation:** **NOT READY**

This records the review result supplied by the owner. It is not a second
independent review. The reviewer found the Windows staging direction sound
within the tested boundary, while identifying incomplete M8 gates and three
staging API hardening items.

## Findings

| Severity | Finding | Status |
|---|---|---|
| Critical | C1 canonical import publication path is absent | Open; later implementation gate |
| Critical | C2 semantic segment schema/signature/chain/trust validation is absent | Partial: staged snapshot integrity is verified; trust/cutoff and publication integration remain open |
| Critical | C3 prepared/committed journal and crash recovery are absent | Open; next transaction implementation gate |
| Critical | C4 owner-approval IPC is absent | Open; trust remains disabled |
| Critical | C5 v3 security events do not feed `RetrievalSafetyState` | Open; trust remains disabled |
| High | H1 managed transport folder ID/path binding is not wired | Open; transport remains disabled |
| High | H2 canonical destination file-ID alias scan is absent | Open; required before publication |
| High | H3 Windows commit-marker durability is absent | Open; required before Windows commit claims |
| High | H4 Unix/macOS handle containment is absent | Open; cross-platform gate |
| High | H5 macOS live-process revalidation is absent | Open; approval IPC gate |
| High | H6 bounded bootstrap parser is absent | Open; protocol gate |
| High | H7 importer/canonical lock integration is absent | Open; publication gate |
| Medium | M1 rollback released all directory pins before cleanup | Fixed below |
| Medium | M2 duplicate `(DeviceId, segment_index)` candidates were not explicitly rejected before I/O | Fixed below; target no-replace check remained a secondary defense |
| Medium | M3 transport path was not bound to the supplied DeviceId/index | Fixed below with canonical relative naming |
| Medium | M4 startup recovery gate is not on the synthetic `pilot-fixture` path | Open; fixture remains isolated and must not enable trust |
| Medium | M5 reducer implementation/review remains open | Open; trust remains disabled |
| Low | L1 Windows path pre-walk was redundant/misleading | Open for cleanup; handle-relative open is the security boundary |
| Low | L2 synthetic `pilot-fixture` bypasses normal recovery gate | Open; scoped to fresh marker-only test runtime |

The reviewer also confirmed the current implementation has no production
importer, the managed Syncthing profile rejects active folders/peers, the
startup recovery gate is limited to normal daemon hold entry points, and
unknown journal state fails closed. The Windows component walk and its native
junction/ancestor-rename tests support the narrow tested claims; they do not
prove arbitrary-race immunity. M8 remains incomplete and trust stays disabled.

## Changes after the review

- `stage_segment` now rejects a repeated `(DeviceId, segment_index)` before
  touching incoming data and rolls back the whole unprepared batch.
- Incoming candidates must use
  `<device-id-hex>/<20-digit-segment-index>.seg`, with both components exactly
  matching the supplied candidate identity.
- Rollback drops only the transaction-directory pin while keeping its ancestor
  handles held through cleanup.
- `ImportStagingSession::verify_candidate` reopens the staged object through
  the held-directory chain, checks object identity and digest, parses only
  complete frames, verifies supported v1/v2 signatures using a caller-supplied
  trusted-key resolver, and checks device, Brain, sequence and previous-hash
  continuity from a caller-supplied verified tip. It is not wired to
  publication and does not replace trust/cutoff or `RetrievalSafetyState`
  policy checks.
- ADR-0019 §5.1.1 now documents this incoming relative naming contract.

`cargo fmt --all -- --check` and `git diff --check` pass. The first focused test
launch was blocked by Windows Application Control (OS error 4551). A normal
Cargo retry launched successfully without policy changes and exposed one stale
test assertion that still named the pre-binding fixture path. The assertion now
checks the actual synthetic source path. Focused staging tests pass (10/10),
and the full daemon library suite passes (408 passed, 4 ignored). This confirms
the retry resolved this observed launch attempt; it does not claim the system
policy has changed or that future 4551 events are impossible. Strict Clippy,
format and diff checks pass. No policy change, Actions, commit, or push occurred.

## Remaining order

1. Connect staged-segment integrity verification to the complete trusted
   stream/cutoff validator, then implement a durable immutable
   prepared/committed journal with fail-closed crash reconciliation.
2. Add canonical file-ID alias checks and integrate the publisher with the
   inventory/device lock set and all canonical readers.
3. Establish Windows durability behavior, then implement and verify macOS parity.
4. Complete owner-approval IPC, bounded bootstrap parsing, v3/reducer integration,
   then obtain a fresh independent review before any trust activation.
