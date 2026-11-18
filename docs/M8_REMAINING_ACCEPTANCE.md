# M8 remaining acceptance — current checkpoint November 17

## Current continuation (supersedes the historical November 14 gap list below)

Both normal product paths now have incoming integration. The Windows missing
receive pump was fixed and passed native CI including genuine two-process
bidirectional exchange/restart. Owner confirms the existing Mac-origin test note
is now visible in the updated Windows VM; the reverse direction was already
observed on Mac. See [automated evidence](../evidence/m8/m8-windows-normal-incoming-20261117.json)
and [owner observation](../evidence/m8/m8-owner-bidirectional-delivery-20261117.json).

VM paused-restart visibility of the existing received note is now owner-confirmed,
alongside Mac's earlier paused reopen. Do not repeat pairing or these observations.
Extended normal-Owner native coverage now passes pause persisted across restart,
separate offline tails, no delivery during pause and convergence after resume.
This exposed and corrected a Windows native lock-contention classification bug;
CI44166454534 passes Windows104/Mac30 checks. See the
[failure, fix and bounded result](../evidence/m8/m8-normal-offline-backlog-20261117.json).
It remains separate from actual-device evidence; the owner now confirms applying
the additional VM update and saving Pause sync=false. No fresh binary attestation
was collected. Do not request another successful-update receipt.
Remaining closure work must distinguish actual-device offline/reconnect and
interruption evidence, integrated cutoff/conflict/protection status, and final
candidate security/human review. Successful UI search is not canonical hash
equality or full milestone acceptance. Do not expand this into M16 stress work.
Search usability improvement was requested for later; it does not change M8 scope.

The run-bound local canonical verifier and shared Mac/VM packet now pass
CI44224805589 at source863bfcd. Both existing applications must be normally
stopped; no reset, enrollment, canonical repair or note-copy loop is required.
The packet compares daemon-verified aggregate record sets without exporting
memory bodies. The Windows owner receipt verifies ten records from two devices;
the Mac report and set comparison are pending. Packet creation is not
physical convergence/interruption acceptance. See
[the packet evidence and limits](../evidence/m8/m8-local-canonical-verification-20261117.json).

## Finite closure map (November 17; historical gaps below are not a new backlog)

| Contract area | Current accepted observation | Still required for closeout |
| --- | --- | --- |
| Pairing / owner actions | Actual Mac and VM bilateral approval/recording; existing session, replay and cancellation receipts | Final candidate UI/diff review, not another pairing run |
| Transport / incoming validation | Both normal paths connected; actual notes visible in both directions; native signed publication/replay tests | Include these sources in final security review |
| Offline / reconnect | Both actual received notes survive paused reopening; genuine two-Owner offline tails converge with identical immutable bytes in CI | Run-bound daemon-verified digest comparison on the actual Mac/VM after reconnect/interruption |
| Restriction / cutoff | Existing signed nonzero cutoff/beyond-boundary refusal; live local compromise/retirement passed CI44172483591; three real native Windows Owners now prove restriction delivery to the remaining eligible peer, fresh-revision old-roster refusal, continued authorized traffic and paused restart in CI44220199842 (213.91s) | Physical cross-platform restriction/refusal acceptance on disposable identities and reviewed terminal retirement delivery; never retire the owner's paired test identity just to satisfy this gate |
| Protection status | Settings explicitly say local readiness only; Mac native status never upgrades publication to remote durability; ten shared renderer policy checks pass | Final integrated runtime/error-state review; legacy metadata CLI is not runtime protection evidence |
| Cross-platform correctness | Prior exact-source Mac native receipts plus latest Windows normal receive and shared writer CI | Relevant final candidate CI; do not claim a Windows-only native test exercises Mac |
| Security / human acceptance | Earlier bounded source reviews retained | Final candidate security and human diff/flow review; M8 remains OPEN |

No new remote receipt protocol, general sync-stress campaign, Search redesign or
profile reset is implied by this map. The controlled activation proposal requires
actual-device daemon-verified canonical digests; raw file counts and successful
Search alone cannot close that evidence requirement.

The remaining-peer propagation scenario uses daemon-written bilateral anchors,
normal native export/incoming publication and no inter-profile canonical/inbox
file copies. It also retains the restricted peer's accepted notes and verifies
equal sealed canonical bytes on the remaining pair after restart. It is one
Windows CI host, not a new Mac/VM receipt. All selected checks passed at source
a1af58a: Windows105 and Mac30; see
[the bounded result and initial fixture correction](../evidence/m8/m8-remaining-peer-restriction-20261117.json).

Concrete propagation gap from the November 17 source review: self-retirement
can remain in the canonical active pack (`append_confirmed_owner_security_operation`
uses the ordinary threshold), and `exchange_policy::eligible_peers` refuses a
retired local identity before the normal exporter can send its terminal record.
Even a sealed retirement record would not bypass that eligibility refusal.
Do not describe local retirement/transport shutdown as remote propagation. A
reviewed terminal-record handoff/reconciliation path is still needed; do not keep
retired transport alive, bypass trust or change the approved terminal policy to
make a test green. Restriction propagation to remaining eligible peers is a
separate case from delivering retirement from the retired installation itself.
This limit is already explicit in
[the sealed export checkpoint](milestones/M8_SEALED_EXPORT_CHECKPOINT.md);
it is not a newly introduced regression or a reason to repeat owner setup.

## Historical November 14 snapshot

The source-gap and activation statements below describe that date, not current
implementation. Later corrections and owner-approved integration are recorded
in the [current handoff](34_CURRENT_IMPLEMENTATION_HANDOFF.md) and completion
ledger. Retain these older evidence boundaries without treating resolved gaps
as new work or repeating their tests.

This is an operational checklist, not an amendment or percentage estimate.
Authority: [M8 contract](milestones/M8_DEVICE_TRUST_REPLICATION.md),
[test matrix](17_TEST_AND_FAILURE_MATRIX.md), [Definition of Done](21_DEFINITION_OF_DONE.md),
accepted [ADR-0019](adr/ADR-0019-m8-peer-trust-design-proposal.md),
[ADR-0020](adr/ADR-0020-macos-m8-owner-channel-candidate.md) and
[ADR-0021](adr/ADR-0021-disabled-macos-transport-helper.md).

## Three closure areas: bounded slice complete, two broader areas open

**Selected A/B correction COMPLETE at canonical-history scope:** the owner chose
cutoff-bound local admissions and strict terminal-tail holds. Source2276652
passed exact [Mac/Windows CI43646684219](https://github.com/gungortarik/cryptex/actions/runs/43646684219),
nine new tests per platform and independent source review. See the
[correction](../evidence/m8/m8-admission-terminal-review-20261114.md). Do not ask
the same policy choice again or treat earlier NOT READY snapshots as its status.

**Current activation blocker R4:** authenticated staged policy conflicts still
become clean rejection and can reopen old canonical content. Define/review a
bounded durable verified-evidence/restart-hold contract before connecting
publication; neither a volatile latch nor publication of a held union is safe.
Malformed/unknown-key candidates must not acquire policy authority. Mac adapters,
product identity/OS policy and final integrated acceptance remain separate.
See the [full review/disposition](../evidence/m8/m8-final-protocol-review-20261114.md).

1. **Bounded native safety checks — COMPLETE for this slice.** Combined
   supervisor-loss native CI and owner-Mac acceptance are complete for their
   recorded source. The two endpoint/exec P2 findings and fixed
   [eight-case SDK inventory](proposals/m8-macos-endpoint-lifetime-check.md)
   passed exact-source [Mac/Windows CI43438187261](https://github.com/gungortarik/cryptex/actions/runs/43438187261)
   on `3a12bcb`, with five HTTP boundary tests on each platform, independent
   source review and original real Syncthing/Desktop regressions. This is not
   atomic FD-handoff attribution or product activation. New SDK negatives are
   native-CI evidence; no unchanged owner rerun is requested.
2. **Protocol and Mac product integration decisions.** ADR-0019's complete
   protocol/schema activation gates remain separate from its accepted Windows
   implementation basis. The reviewed native development/test paths remain
   default-off. Resolve the remaining protocol acceptance, configured product
   image policy, supported OS floor, product-path integration and explicit
   activation acceptance. Development/test certificates do not establish those
   decisions. Do not silently activate ordinary routes from a fixture pass.

   November 14 protocol review makes the remaining items explicit:

   - **Terminal-retention/recovery slice COMPLETE through40da5f7.** The
     [selected compact receipts](proposals/m8-terminal-journal-retention.md)
     preserve cancellation refusal, uncertain evidence and canonical retry
     binding under the existing one-MiB cap. Fourteen terminal and four real
     bilateral recovery tests passed each platform; seventeen genuine local
     anchors reopen with one scan/no journal rewrite and unchanged canonical
     bytes. Full [CI43518226858](https://github.com/gungortarik/cryptex/actions/runs/43518226858)
     and same-source default-off [native CI43518652758](https://github.com/gungortarik/cryptex/actions/runs/43518652758)
     passed. This is not final product integration acceptance.
   - Independent bootstrap/v3/owner-digest wire fixtures passed full native
     [CI43505609309](https://github.com/gungortarik/cryptex/actions/runs/43505609309)
     on `e49e318`; that slice is complete. **Candidate schema/compatibility
     slice COMPLETE through1f5e6dc:** bootstrap envelope/transcript and v3
     JSON Schema/TypeScript generation, frozen v1/v2 preservation and 495
     independent structural/re-signed cases passed exact-source
     [Mac/Windows CI43563383271](https://github.com/gungortarik/cryptex/actions/runs/43563383271).
     All ten generated files match across platforms. Schema acceptance alone
     never establishes signatures, history or owner authority.
   - Actual historical pre-v3 services now demonstrate typed whole-Brain hold
     on valid signed v3 and preserve canonical bytes, database state and vault
     contents, following a successful v1-only baseline. Signed target-history
     cutoff `0`/`1`/`u64::MAX`, wrong hash, missing position and overflow tests
     cover reducer/content selection. See the
     [review/limits](../evidence/m8/m8-protocol-schemas-compat-review-20261114.md).
     These historical service-guard results do not claim an installed old
     Desktop or OS-store reinstall campaign. Supported product-path integration
     remains separate from the completed domain compatibility proof.
   - The [§5 recovery coverage map](../evidence/m8/m8-recovery-coverage-map-20261113.md)
     now reconciles covered domain invariants for all nine rows. New c6e07d6
     tests add durable cutoff0/1, partial on-disk SQLite rebuild, guarded-vault
     domain reopen and torn-journal/missing-key-facade refusal. This bounded
     daemon-service recovery slice is COMPLETE: exact-source
     [CI43626437079](https://github.com/gungortarik/cryptex/actions/runs/43626437079)
     passed Mac/Windows readiness and Windows cross-account IPC ACL. See the
     [recovery receipt](../evidence/m8/m8-restriction-derived-recovery-native.json).
     Existing write/flush/replace/readback/sync fault tests were already covered.
     Do not reopen those lower layers or repeat completed VM packages without
     a new change/failure. Process interruption is not physical power loss.
   - Accept the complete independently reviewed protocol and its explicit SCC
     Brain-wide hold/availability boundary, then choose the product image policy
     and connect the supported product paths. The owner selected macOS 26+;
     packaging/deployment/runtime enforcement remains to be done.
   - Remaining supported-product work is concrete: authenticated approval/session
     routes with truthful local/peer durability and uncertainty; incoming
     publication, canonical trust reconstruction and guarded derived refresh on
     each platform; controller clean-stop/uncertain outcomes in replication
     protection status. `incoming_publication` remains Windows-only. These
     connections require integrated failure checks; they are not another generic
     lower-layer test campaign.
3. **Final milestone acceptance.** On the actual accepted product paths, verify
   all seven scope areas together: pairing, transport controller, incoming
   validation, offline/reconnect convergence, revoke/retire/compromise, sequence
   cutoff and truthful replication protection status. Preserve existing receipts
   as their original scoped evidence; do not combine separate tests into an
   unexecuted cross-platform end-to-end claim. Required CI, security review,
   platform checks and human diff/flow review must then be complete.

Existing Windows, host/VM and Mac receipts are not discarded or automatically
repeated. The [current handoff](34_CURRENT_IMPLEMENTATION_HANDOFF.md) identifies
which exact gate is active. New device intervention must exercise a concrete new
gate. General sync stress belongs to M16 and is not added as an artificial M8
blocker. M9 starts only after the M8 completion gate passes.

Two broader areas remain. There is no reliable day/date or completion percentage yet: native fixes can
produce new findings, product decisions are not selected, and final integrated
acceptance remains distinct from the development fixtures. This checklist makes
the finite remaining categories explicit instead of claiming one last test.
