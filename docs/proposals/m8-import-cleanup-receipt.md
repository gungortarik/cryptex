# M8 interrupted import cleanup: proposed receipt

Date: 2026-11-07. Status: **PROPOSED; owner approval required; not implemented**.
Base implementation: `e2dc285`. This document does not amend ADR-0019 by itself.

## Problem demonstrated

After verifying a final committed marker and every destination, recovery removes
staging, then the prepared manifest, then the committed marker. Process exit
between those removals leaves a marker without its required companion.
ADR-0019 section 5.1.1 explicitly requires a typed hold in that state, including
when it was produced by our own interrupted cleanup. Canonical bytes survive,
but unattended startup cannot finish. Repeated successful cleanup calls do not
cover this interruption window.

The new subprocess regression reproduces both holds and verifies preserved
canonical bytes and remaining evidence across two recovery attempts. It also
checks successful recovery after the last removal. See
[the evidence record](../../evidence/m8/m8-cleanup-interruption-20261107.md).

## Proposed narrow protocol amendment

Add one transaction-local cleanup receipt under `canonical/import-journal`.
It is daemon recovery metadata, not a canonical event or a trust grant. The
receipt authorizes deletion only of its own verified import bookkeeping; it
never authorizes canonical moves, repairs, policy bypass, or event deletion.

Proposed names: `<transaction_id>.cleanup.json` and
`.<transaction_id>.cleanup.tmp`. The final receipt has a closed schema with
`protocol_version: 1`, `transaction_id`, `committed_marker` (the existing closed
marker object), and `prepared_manifest` (the entire existing closed manifest).
Require canonical UTF-8/LF JSON, the existing identifier/path/segment bounds,
exact marker-to-manifest SHA-256 binding, and a total bound of 72 KiB. Reject
unknown fields, duplicate keys, extra journal entries, aliases and conflicting
transaction IDs. Do not add timestamps, memory bodies, keys or credentials.

Under the runtime instance lock and canonical inventory/device locks, followed
by the import slot, verify the final marker, exact companions and all canonical
destinations. Write, sync, no-replace place, sync and read back the receipt using
the existing platform primitives. No companion deletion may start before the
final receipt is verified. Then remove staging, prepared manifest, committed
marker, and the receipt last, syncing each parent. Canonical files stay intact.

Recovery with a final receipt must acquire the same locks and revalidate every
canonical destination against the embedded manifest before cleanup resumes.
Existing companions must match exactly; missing companions are permitted only
by this receipt-bound branch. Remaining staging contents must be a matching
subset, with no unexpected entries. The final receipt represents only a
previously verified commit; current retrieval policy still governs reading.

| Receipt state | Other evidence | Proposed action |
|---|---|---|
| Absent | Final marker missing any companion | Keep existing typed hold. Never infer successful cleanup. |
| Valid temporary only | Final marker, prepared manifest, staging and destinations all match | Complete no-replace receipt placement and read-back under locks, then cleanup. No deletion before final receipt. |
| Temporary only | Any companion missing or mismatched | Typed hold, preserve evidence. |
| Valid final receipt | All destinations match; remaining companions match or are absent | Resume only transaction-local cleanup; remove receipt last. |
| Valid final receipt | Any destination missing, changed, aliased or unverifiable | Typed hold; never repair or delete canonical files. |
| Malformed, conflicting, final plus temporary, or unknown receipt state | Any | Typed hold, preserve evidence. |
| No transaction evidence | Existing journal lock only | Existing terminal-safe result. |

Receipt cleanup must not recreate missing staging or mutate an anchored pairing.
Old binaries will reject the new journal entries rather than silently use them;
the receipt format must be documented before shipping. No automatic downgrade
or deletion of unrecognized recovery metadata is proposed.

## Implementation and validation after approval

1. Amend the normative Windows decision table in ADR-0019 with the precise
   receipt-bound exception. Preserve the missing-companion hold without it.
2. Implement closed parsing, bounded reads, placement and cleanup using existing
   no-follow, identity and lock-order helpers; no new crypto or canonical writer.
3. Interrupt real child processes before/after receipt sync/placement and each
   cleanup deletion, including within recursive staging deletion. Fresh recovery
   must terminate safely, then repeat without canonical or trust changes.
4. Cover partial receipts, hash/schema/ID mismatches, missing/changed destinations,
   extra files, links, multiple transactions, lock contention and stale policy.
5. Separately finish policy-bound startup integration, especially temporary
   committed-marker completion. A cleanup receipt does not waive those checks.

Approval is for this limited recovery-protocol amendment and its implementation,
not transport publication activation, release readiness, or acceptance of the
whole ADR. Windows power-loss guarantees, macOS parity, native peer-stream
integration and independent review remain separate gates.

## Authority boundary

AGENTS.md says: "If requirements conflict, stop implementation and report the
conflict. Do not invent a new architecture." ADR-0019 section 5.1.1 says a valid
marker with a missing prepared/staging companion is a typed hold. Replacing that
rule with automatic continuation requires the explicit amendment above.
Until approval, retain existing behavior and keep publication inactive.
