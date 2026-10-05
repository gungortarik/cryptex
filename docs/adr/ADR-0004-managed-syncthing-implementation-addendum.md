# ADR-0004 implementation addendum — managed Syncthing

**Status:** PROPOSED — Windows isolated spike verified; acceptance pending
**Date:** 2026-10-05
**Scope:** M0 P0-4 implementation choice; no replication semantics

## Source record and authority

The original ADR-0004 source is missing. This addendum records an explicit new
implementation proposal and its evidence, without reconstructing that record.
Authority: Master Spec, accepted stack baseline **2.1.5**, M0 contract and P0-4
spike. Trusted-device semantics, canonical validation/signatures and writer
authority remain Cryptex responsibilities. Syncthing transports files only.

## Proposed decision

Distribute an unmodified, target-matched **stable Syncthing 2.1.5** native
executable with Cryptex, retaining upstream license/attributions and provenance.
Resolve from the package, rather than assuming PATH or a user installation.
Keep its private config/data below platform application support, distinct from
all user-global profiles. Disable independent automatic binary upgrades; Cryptex
package compatibility and rollback must control changes to the pinned version.

The backend should own process supervision and authenticated loopback REST;
renderer capabilities must not expose API keys, arbitrary shell/filesystem or
raw Syncthing configuration. Backend controls pairing/folder state and maps
transport status/conflict/version observations into typed diagnostics. Secrets
require OS secure storage integration, with native generated private files
protected by the user-only platform permissions. No public Internet API or
visible default home Sync folder is required.

This document does not authorize a new production supervisor or claim these
distribution/backend requirements are implemented. The current Node controller
is a test tool and is not shipped. Productionization remains separately scoped.

## Alternatives considered

- User-global installation/profile: cannot reliably pin the engine or preserve
  isolation; rejected for the proposed managed product boundary.
- Unpinned self-updating process: package compatibility/rollback can drift;
  rejected for the reproducible baseline.
- Embedding transport internals as a library or recreating synchronization:
  introduces unnecessary implementation/upgrade coupling; not proposed.
- Standalone pinned native process with backend REST control: measured here;
  proposed because it retains the upstream protocol and an explicit boundary.

## Evidence

`evidence/m0/p0-4/windows-managed-syncthing-evidence-20261005.md` records separate
native identities, isolated config, authenticated REST, pairing/transfer,
pause/resume/status, native history/conflict preservation, retained same-version
restart and clean shutdown. Four real monitor exits were 0 and their PIDs absent.
The observed user-global config digest stayed unchanged.

The staged-package follow-up runs a copied native binary with exact LICENSE/AUTHORS
from its fresh package role and rechecks membership/digests afterward. Windows
passes; this does not establish a signed/installed distribution or dependency
notice completeness. Mac executable-mode/package parity remains pending.

Early restart shutdown needed about 10.1 seconds. Exact pinned source includes
a 10-second connection close timeout and a 10-second database shutdown wait;
the controller uses 25 seconds with margin, rejects database-stop failure and
retains forced cleanup as failure. Native Syncthing behavior is not patched.

Upstream license/author inventory: `third_party/syncthing/2.1.5/`.
Sources: [release](https://github.com/syncthing/syncthing/releases/tag/v2.1.5),
[configuration API](https://docs.syncthing.net/rest/config.html),
[process/CLI options](https://docs.syncthing.net/users/syncthing.html).

## Tradeoffs, platforms and recovery

Native executables/resources/notices must be retained per supported architecture.
Loopback ports may collide; a bind/readiness failure must be explicit, never
attach to an unknown process. Keep Windows process-tree and POSIX process-group
cleanup behind platform adapters, targeting only owned test/managed children.
A transport crash must not change canonical writer/trust rules or silently
reinterpret corrupted files. Startup failure, secret failure, version mismatch
and incompatible config/database migration require structured refusal.

Pairing tests use synthetic local identities. They do not establish production
device trust, cross-host semantics, platform signing or publisher authenticity.
Native history/conflict files are diagnostic transport artifacts; they must not
be treated as canonical events. The upgrade policy needs an actual changed-version
compatibility/rollback probe before its acceptance claim.

## Required acceptance evidence

1. Equivalent Mac managed lifecycle against exact stable 2.1.5.
2. Target-matched staged/bundled package resolution and notice inclusion on both
   OSes; source/provenance/dependency-notice review before distribution.
3. Explicit upgrade compatibility strategy and tested migration/failure boundaries;
   same-version restart alone must not be relabeled version upgrade.
4. Review of proposed supervisor/secret boundary and residual M0 limitations.
5. Human acceptance and evidence-backed P0-4 checklist.

**Not accepted.** Windows scoped PASS does not complete P0-4 or M0, and does not
authorize M1 or product replication features.
