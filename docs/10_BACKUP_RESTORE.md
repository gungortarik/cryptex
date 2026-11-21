# 10 — Cryptex Backup and Restore

**Status:** ACCEPTED  
**Baseline:** 2026-09-28

## 1. Purpose

This document defines the accepted V1 backup and disaster-restore boundaries.

It distinguishes backup creation from verified recovery and preserves the local-first architecture without introducing a required cloud API dependency.

Exact backup encryption/container implementation is intentionally deferred to the required feature-specific spike and M11 evidence.

## 2. Backup target model

V1 backup is an encrypted filesystem-target abstraction.

A user may choose a filesystem path such as:

- a Google Drive for Desktop synchronized folder
- a USB drive
- another local filesystem path

Cryptex V1 does not require Google OAuth or direct Google Drive API integration.

A cloud-synchronized folder is treated as a filesystem target, not as canonical application infrastructure.

## 3. Local-first boundary

Backup must not become a prerequisite for normal local operation.

Canonical local memory, local capture, local search and daemon authority remain functional without a cloud backup service being reachable.

Backup is a recovery mechanism, not the canonical writer or application database.

## 4. Encryption requirement

V1 backups are encrypted.

Cryptex does not implement custom cryptography.

The exact audited high-level format/library must be selected through the required backup encryption/container spike before M11 product implementation.

That spike must exercise:

- large objects
- interrupted backup
- restore
- key recovery

## 5. Recovery Kit relationship

Disaster recovery must restore both content and trust.

The Recovery Kit therefore carries standardized recovery secret material for:

1. decrypting encrypted backups
2. authorizing the first new trusted device after loss of all ordinary trusted devices

The offline recovery signing secret is not routinely stored on normal devices.

The owner can disconnect the replacement computer and use a separate offline
Recovery Kit tool on it. A second computer is optional. The replacement
prepares a public challenge bound to its fresh device key and the reconstructed
archive. The owner checks the Brain, backup and device fingerprint and explicitly
approves that challenge in the offline tool. Only the public signed approval
returns to the replacement. Routine Desktop and daemon operation have no
interface for producing a recovery signature.

Before starting the service on a replacement, the owner can prepare a new
profile with `cryptexd lifecycle prepare-first-device-m11 --runtime-root <absolute-new-directory>`.
The parent directory must exist. The command refuses an existing destination
and creates only the minimum-reader lock, without enrolling a device.

In that blank profile, **Check Kit and backup** verifies the selected
encrypted backup, its independently held checkpoint, the sealed Kit and its
separate unlock file. **Prepare replacement profile** repeats those checks and
stages the verified archive and a fresh device identity. An occupied profile
is preserved. The replacement uses the minimum-reader lock layout for M9
attachment history.

Choose **Save public request** and take the replacement computer offline.
Open the separate Recovery Kit utility, compare the Brain, backup digest,
device ID and key fingerprint, and enter the displayed device ID to approve.
Choose **Import signed approval** in Cryptex to activate the pending identity.
After an interruption, the same signed approval can finish activation with the
same device key. These steps are currently exercised on temporary profiles.

The exact Recovery Kit container/serialization is not selected by this document.

## 6. Scheduled backup writer

V1 uses one explicitly designated device for scheduled backup and retention.

This avoids multi-writer pruning/retention races.

Other trusted devices may:

- take manual emergency backups
- explicitly take over the scheduled-backup role through a controlled role change

If the designated device is lost, the owner may start scheduled backups on a new
target with another trusted device. Taking over the existing target requires its
current writer's signed approval. Before issuing approval, the outgoing writer
durably gives up its authority to publish and prune on that target. The recipient
verifies the target, the current writer's signature and its own pending request.
Restarting the former writer cannot reactivate its previous jobs.

The designated backup writer is not the canonical master device.

It is an operational role only.

## 7. Backup is not verification

Successful upload, file creation or target synchronization does not prove that a backup is valid.

Backup verification requires restore/reconstruction evidence.

The audit requires periodic sandbox restore behavior as part of recovery confidence.

A backup status must not be represented as Verified merely because bytes were written successfully.

## 8. Restore requirement

Restore must reconstruct the required Cryptex state from accepted backup/recovery material according to the applicable milestone.

Recovery must not depend on disposable local state such as:

- SQLite databases
- search indexes
- generated Obsidian vaults
- caches
- UI state

Derived state is rebuilt after canonical recovery.

## 9. Sandbox restore

M11 includes sandbox restore rehearsal.

Restore testing must avoid overwriting a live personal Brain merely to verify recoverability.

The exact sandbox layout and automation mechanism are implementation details for M11.

The result must provide real reconstruction evidence rather than configuration-only proof.

## 10. Retention

M11 owns backup retention behavior.

Retention is performed by the designated scheduled backup writer in V1.

Retention behavior must not silently introduce multi-writer pruning races.

The exact retention schedule/policy is not selected by this document unless later accepted product requirements define it.

## 11. Manual export

M11 includes a manual export bundle.

The exact bundle format is not selected here.

Manual export must not weaken the security/recovery model or become a hidden unencrypted copy unless explicitly designed and accepted as such.

After first-device recovery, keep the original encrypted backup and the new
device's encrypted history in separate files. Preserve the original ciphertext
byte for byte. A complete recovery check must open both files and verify the
new device's relationship to the original history. Keep their public recovery
checkpoint independently of the backup target.

## 12. Failure behavior

Relevant backup/recovery testing includes:

- interrupted backup
- partial target availability
- damaged backup
- large-object handling
- incorrect/missing recovery material
- restore into a sandbox
- derived-state rebuild after restore
- target unavailable/offline

Failures must be explicit and diagnosable.

Backup tooling must not report Verified when required reconstruction evidence is absent.

## 13. Privacy and logs

Backup/restore logs and diagnostics must not contain by default:

- memory bodies
- backup decryption secrets
- Recovery Kit signing secrets
- credentials
- private keys

Recovery-sensitive diagnostics remain local/redacted according to the operations specification.

## 14. Purge interaction

Physical purge is best-effort across reachable Cryptex-controlled backups/snapshots.

Cryptex cannot promise deletion from provider-internal historical versions that it does not control.

When an affected encrypted generation contains other notes, retain it until the
offline Kit is available and a sandbox restore establishes that a replacement
preserves every unaffected record and attachment. The owner must then approve
removal of that particular old generation separately. Verification, a note purge
and a saved retention count do not grant this approval. Cancellation or loss of
either target leaves the old generation pending.

## 15. Cross-platform behavior

Backup and restore are product requirements on macOS and Windows.

Filesystem-target behavior, large-object handling and recovery must be verified where platform differences matter.

A backup solution that only works correctly on one supported platform is insufficient for the applicable product gate.

## 16. No cloud lock-in

V1 does not require a vendor-specific cloud API.

A user may choose any compatible filesystem target according to the product implementation.

Adding a required cloud service, OAuth dependency or server-side canonical component requires explicit architectural approval.

## 17. Milestone boundaries

### Before M11

The backup encryption/container P1 spike must select an audited high-level format/library and verify the required failure/recovery scenarios.

### M11 — Backup + Disaster Restore

M11 includes:

- filesystem target
- encryption
- designated backup writer
- retention
- verification
- sandbox restore rehearsal
- Recovery Kit first-device bootstrap
- manual export bundle

### M16 — Hardening

M16 includes broader backup/restore drills as part of product hardening.

### M17 — Private Beta / Release Candidate

Real personal Brain onboarding occurs only after backup/recovery confidence.

## 18. Explicitly unresolved

This document does not select:

- exact encryption/container library
- exact backup archive/chunk format
- exact retention schedule
- exact sandbox directory layout
- exact manual export bundle format
- exact filesystem-target UI
- exact backup scheduler implementation

These require the feature-specific spike, M11 implementation evidence and applicable accepted ADR/specification updates.

## 19. Source basis

This document consolidates existing backup/recovery requirements from:

- `docs/00_MASTER_SPEC.md`
- `docs/08_SECURITY_DEVICE_TRUST_AND_RECOVERY_ROOT.md`
- `docs/09_FORGET_PURGE_AND_DATA_LIFECYCLE.md`
- `docs/20_MILESTONE_ROADMAP.md`
- `docs/23_REQUIRED_TECHNICAL_SPIKES.md`
- `docs/32_PRE_IMPLEMENTATION_AUDIT_REPORT.md`

It does not independently introduce a cloud dependency or custom cryptographic design.
