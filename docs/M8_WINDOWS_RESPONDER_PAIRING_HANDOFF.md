# M8 Windows responder pairing handoff — 2026-11-05

## Status

**M8 remains in progress. The interactive Windows pairing is not complete.**
This note records the latest owner-guided responder pairing attempt and its
unresolved failure. It is an operational handoff, not a change to the Master
Spec, accepted ADRs, or milestone scope.

## Current repository checkpoint

- Repository: `D:\github_repos\cryptex`
- Branch: `codex/m8-device-trust`
- Last observed code commit before this handoff note: `38d41b0`
  (`feat(m8): resume initialized Windows transport pilot`)
- At that checkpoint the branch was clean and matched
  `origin/codex/m8-device-trust`.
- This handoff note and its link from `docs/34_CURRENT_IMPLEMENTATION_HANDOFF.md`
  are new documentation changes and may be uncommitted.

## What was being tested

The owner has one Windows VM. The test used two separate, disposable Cryptex
pilot runtime profiles on that VM to simulate the initiator and responder
roles. Two VMs are not required for this specific signed-message protocol
exercise; a second independent endpoint would provide stronger platform and
transport coverage later.

The test uses a synthetic Brain and temporary pilot data only. It must not use
the owner's personal Cryptex runtime or memory. Pairing is an explicit signed
approval exchange; it does not by itself mean that production transport or
synchronization has started.

## What has succeeded

1. The initiator pilot created and journaled a signed offer.
2. The responder pilot accepted the offer and public Brain identity and
   journaled a signed proof.
3. The initiator reviewed the exchange and signed its approval. The UI reported
   that the approval was created but was **not yet a durable trust record**;
   no peer had been added to the trusted roster.
4. The owner closed the app. On return, the same initialized responder pilot
   was reopened using the supported `--cryptex-pilot-resume` option.

These are observed intermediate steps only. Do not describe the devices as
paired or trusted.

## Current failure and exact stopping point

On the responder, the owner supplied the initiator's signed offer, the public
Brain identity JSON, and the initiator's signed approval. The responder had a
journaled proof. After loading the exchange into the review field and choosing
**Verify and review**, the UI displayed:

> Cryptex could not verify this signed transcript (`invalid_transcript`). No approval was created.

Therefore no responder approval was created, no durable trust record was
added, and no synchronization was activated. The app was closed overnight and
reopened on 2026-11-05. The last instruction was to navigate to **Device Trust
· M8 Windows Flow** and check that the role is **Responder · create proof**;
the owner has not yet reported that check. Do not assume the current screen or
role without confirming it.

The assistant previously transcribed one signed approval field incorrectly,
then supplied a corrected full message. The owner replaced the field with the
corrected original signed message, but verification still failed. Treat the
transcript failure as unresolved; do not ask the owner to repeat the same
copy/paste loop or hand-edit a signed message. Any change to a signed field
invalidates its signature.

## Source-level findings

- In `apps/desktop/src/App.tsx`, `loadExchangeIntoApprovalReview()` assembles
  the responder transcript in this order:
  `incomingOffer`, `exchangeProof`, `incomingInitiatorApproval`.
- In `crates/events/src/bootstrap.rs`,
  `verify_bootstrap_approval_request()` expects exactly three signed messages
  for the responder prefix: offer, proof, initiator approval. It validates
  signatures, message types/order, transaction and Brain binding, identities,
  transport association, previous-message hashes, and the agreement hash.
- `services/cryptexd/src/ipc.rs` maps any such verification error to the
  generic `invalid_transcript` string. The UI result does not identify which
  message or binding failed.
- Source inspection alone has not identified the failing message. Do not infer
  that the UI ordering is wrong solely from a partially visible textarea; the
  field may be scrolled. Verify the complete transcript or inspect it safely.

## Next actions

1. Confirm the resumed app is using the responder pilot and the responder role.
2. Diagnose the exact signed-message failure using the original offer, proof,
   and initiator approval, preserving every byte of each signed JSON message.
   Compare message types/order, signatures, transaction/Brain IDs, device and
   signing-key identities, transport IDs, hash-chain fields, and agreement
   hash. Keep raw signed messages and device identifiers out of this public
   repository.
3. If the current disposable journal or profile conflicts with the verified
   messages, decide on a fresh disposable test profile only after diagnosing
   the cause; never point the test at personal Cryptex data.
4. Once the responder review succeeds, inspect the exact owner-review values
   before asking the owner to sign. Then verify the final trust-recording step
   separately. A signed approval alone is not proof that the peer is in the
   durable trusted roster.
5. Record the observed result in this note and in
   `docs/34_CURRENT_IMPLEMENTATION_HANDOFF.md`.

Keep the owner interaction short and one step at a time. The owner prefers
Turkish instructions and wants us to compare identifiers ourselves. Do not
make them manually compare long hashes or repeatedly re-enter the same data.

## Other M8 gates remain open

Even if this pilot pairing succeeds, M8 is not complete. Production
Syncthing-to-staging/publication and safe replay, macOS authenticated peer
identity and owner-operation parity, independent security review, remaining
durability/crash-boundary checks, and the Definition of Done remain open.
Focused tests and package smoke do not close those gates. See the authoritative
specification, accepted ADRs, milestone documents, and
[`34_CURRENT_IMPLEMENTATION_HANDOFF.md`](34_CURRENT_IMPLEMENTATION_HANDOFF.md).

## Evidence scope and privacy

Earlier owner-reported Windows results are recorded in `README.md` and the
dated evidence under `evidence/m8/`; those results apply to their stated source
revisions and test groups. They do not prove this later interactive pairing
passed. This note intentionally omits raw signed messages, device IDs, key
fingerprints, personal runtime paths, and cryptographic signatures.
