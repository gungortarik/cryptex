# M8 remaining acceptance — November 13

This is an operational checklist, not an amendment or percentage estimate.
Authority: [M8 contract](milestones/M8_DEVICE_TRUST_REPLICATION.md),
[test matrix](17_TEST_AND_FAILURE_MATRIX.md), [Definition of Done](21_DEFINITION_OF_DONE.md),
accepted [ADR-0019](adr/ADR-0019-m8-peer-trust-design-proposal.md),
[ADR-0020](adr/ADR-0020-macos-m8-owner-channel-candidate.md) and
[ADR-0021](adr/ADR-0021-disabled-macos-transport-helper.md).

## Three closure areas: bounded slice complete, two broader areas open

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

   November 13 protocol review makes the remaining items explicit:

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
     on `e49e318`; that slice is complete. Strict candidate
     v3/bootstrap JSON Schema and TypeScript artifacts remain outstanding.
   - Finish the old-product Brain-hold/immutable-byte downgrade proof and signed
     target-history cutoff `0`/`1`/`u64::MAX` before/at/after checks. Envelope-only
     refusals and cutoff string vectors do not establish those higher layers.
   - The [§5 recovery coverage map](../evidence/m8/m8-recovery-coverage-map-20261113.md)
     now links existing checks and precise platform limits to all nine rows.
     Fill only its uncovered write/flush/replace/readback/torn-journal,
     derived rebuild and missing-key cases; do not repeat completed VM packages
     or relabel process-kill tests as power-loss durability.
   - Accept the complete independently reviewed protocol and its explicit SCC
     Brain-wide hold/availability boundary, then choose the product image policy
     and connect the supported product paths. The owner selected macOS 26+;
     packaging/deployment/runtime enforcement remains to be done.
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
