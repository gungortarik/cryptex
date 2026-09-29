# iMac / Secondary macOS Soak-Node Checklist

**Status:** PROPOSED  
**Baseline:** 2026-09-28

## Purpose

This checklist defines the optional operational role of a second macOS peer used for long-running Cryptex validation.

The iMac or any other always-on device must never become a required canonical master.

## Role boundaries

- [ ] Device is treated as a peer, not canonical owner.
  - Evidence:
- [ ] Loss or unavailability of this device does not prevent other trusted desktops from operating locally.
  - Evidence:
- [ ] No implementation depends on this device's uptime for canonical correctness.
  - Evidence:

## Intended uses

- [ ] Device can be used as a second macOS replication peer when the relevant milestone is active.
  - Evidence:
- [ ] Device can be used for long-running soak tests when required by hardening/replication work.
  - Evidence:
- [ ] Device may be a candidate designated scheduled-backup writer after the backup milestone exists.
  - Evidence:
- [ ] Backup-writer role, if assigned, is explicit and operational only.
  - Evidence:

## Platform/runtime safety

- [ ] Supported/up-to-date macOS is installed.
  - Evidence:
- [ ] FileVault is enabled/recommended according to policy.
  - Evidence:
- [ ] Cryptex runtime data uses the accepted macOS app-data root.
  - Evidence:
- [ ] Development runtime, if used, uses the Cryptex-Dev root outside the source repository.
  - Evidence:
- [ ] No real personal memory is used before the release/onboarding gate authorizes it.
  - Evidence:

## Replication soak requirements

When M8/M16 replication testing is active:

- [ ] Peer can remain online for long-duration replication observation.
  - Evidence:
- [ ] Offline/reconnect behavior is exercised without treating the peer as primary.
  - Evidence:
- [ ] Sleep/wake/reconnect behavior is tested when required by hardening.
  - Evidence:
- [ ] Sync stress evidence is retained according to the active milestone.
  - Evidence:

## Backup-writer candidate requirements

When M11 backup behavior is implemented:

- [ ] Scheduled-backup-writer role can be assigned explicitly.
  - Evidence:
- [ ] Role can be transferred explicitly to another trusted device.
  - Evidence:
- [ ] Other devices may still create manual emergency backups according to the backup spec.
  - Evidence:
- [ ] This role does not grant canonical ownership.
  - Evidence:

## Completion rule

This checklist is not a prerequisite that makes an iMac mandatory for Cryptex.

It is complete only for the operational role actually being exercised.

## Source basis

- `docs/00_MASTER_SPEC.md`
- `docs/18_IMPLEMENTATION_READINESS_PLAN.md`
- `docs/07_SYNC_AND_CONFLICT_PROTOCOL.md`
- `docs/10_BACKUP_RESTORE.md`
- `docs/15_PLATFORM_MACOS_WINDOWS.md`
