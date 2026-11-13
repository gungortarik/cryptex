# M8 remaining acceptance — November 13

This is an operational checklist, not an amendment or percentage estimate.
Authority: [M8 contract](milestones/M8_DEVICE_TRUST_REPLICATION.md),
[test matrix](17_TEST_AND_FAILURE_MATRIX.md), [Definition of Done](21_DEFINITION_OF_DONE.md),
accepted [ADR-0019](adr/ADR-0019-m8-peer-trust-design-proposal.md),
[ADR-0020](adr/ADR-0020-macos-m8-owner-channel-candidate.md) and
[ADR-0021](adr/ADR-0021-disabled-macos-transport-helper.md).

## Three remaining work areas

1. **Current bounded native safety checks.** Combined supervisor-loss native CI
   and owner-Mac acceptance are complete. Finish the two endpoint/exec lifetime
   findings and fixed [eight-case SDK inventory](proposals/m8-macos-endpoint-lifetime-check.md),
   preserving the existing actual Syncthing/Desktop regression inventory. Record
   exact source, native CI and independent review. A source fix alone is pending.
2. **Mac product integration and activation decisions.** The reviewed native
   development/test paths remain default-off. Resolve configured product image
   policy, supported OS floor, remaining product-path integration and explicit
   activation acceptance. Development/test certificates do not establish those
   decisions. Do not silently activate ordinary routes from a fixture pass.
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

There is no reliable day/date or completion percentage yet: native fixes can
produce new findings, product decisions are not selected, and final integrated
acceptance remains distinct from the development fixtures. This checklist makes
the finite remaining categories explicit instead of claiming one last test.
