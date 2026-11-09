# Offline convergence and restricted-tail integration

Date: 2026-11-08. Local daemon integration; no native transport or product activation.

Added two tests to the existing persistent incoming-publication fixture:

1. Both directly approved peers independently append and seal an offline note
   through cryptexd's canonical writer. Initialize each derived view from its own
   signed history, exchange candidate files in the isolated incoming test tree,
   publish with reversed segment hint order on one side. Both canonical segment
   inventories/hashes and guarded note search converge; each retains all previous
   immutable segments. Replaying adds zero new segments and preserves both hashes.
2. For each of retired/compromised, receive the approved peer's seq 1–3 prefix,
   build a restriction snapshot from verified policy at exact cutoff 3, persist
   and confirm its owner journal transaction, append through the existing daemon
   owner-operation path. The disconnected target (not informed of restriction)
   appends seq 4. Receiver rejects that signed tail before canonical publication,
   preserves incoming evidence and prior canonical bytes, retains guarded search
   access to the authorized prefix, and never exposes the stale note.

Synthetic keys and explicit test-owned journal confirmation are used. No Desktop
owner-credential attestation or real human approval is inferred from this helper.
No network runs in these tests: explicit file copies only feed test incoming
candidates and are not a cross-device offline/reconnect receipt. No production
writer, admission, cutoff, transport feeder or security setting changed.

Focused offline selection passes (two new tests plus one existing recovery-store
test). Whole incoming publication group: 16 passed, 0 failed, 0 ignored
(39.78 seconds). Strict all-target daemon Clippy, formatting and diff checks pass.

M8 requirement-to-evidence mapping and exact next gates are consolidated in
`checklists/M8_DEVICE_TRUST_REPLICATION_GATE.md`. The previous actual native
Windows pilot remains passed and does not need repeating.
