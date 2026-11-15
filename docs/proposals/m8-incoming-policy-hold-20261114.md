# R4 — Restart-persistent authenticated incoming policy hold

Implementation reviewed and native-verified on `1a042f9`:
[review](../../evidence/m8/m8-incoming-policy-hold-review-20261114.md),
[exact-source receipt](../../evidence/m8/m8-incoming-policy-hold-native.json).
This closes the bounded R4 correction, not full protocol/product acceptance.

Scope: the existing, inactive Windows incoming publisher. No product route or
Mac publication adapter is enabled. This implements the owner's selected
whole-Brain conflict policy and explicit request for restart persistence.

Before evaluating the post-import policy, persist a closed, purpose-bound
review intent covering the **complete** remaining staged batch, Brain and local
device/signing identity. Store matching copies in the private staging transaction
and import journal, with the existing 16-file/4-GiB batch limits and a 64-KiB
receipt bound. Both files must be durable before classification. This is local
recovery evidence, never a prepared publication manifest or trust authority.

Verify every staged frame, signature, key binding and chain against the locked
canonical snapshot before classifying any policy conflict. Bad signatures,
unknown keys, malformed records and a mixed malformed batch reject as a whole.
Only authenticated unresolved restriction/admission dependencies, terminal
retirement tails and loss of an existing/candidate admission become typed policy
conflicts. Other invalid policy histories remain rejection; competing stream
fork admission is not added in this change.

On conflict preserve all staged bytes and the intent, and persist a distinct
held-seen latch in staging followed by a separate held decision in the journal,
both bound to the intent hash. A missing final decision after the latch is
recovery uncertainty, never an unclassified intent. Close shared ordinary admission. Never
publish the held union or rebuild derived state from it. On clean rejection or
Ready policy, close the review intent before the existing discard/prepare path.
Uncertain persistence/cleanup is RecoveryRequired, never clean rejection.

Before startup readiness, IPC, transport or derived work, retained intents must
be reopened with ownership/no-follow checks, closed-schema/canonical JSON checks,
exact file inventory, identity binding and hashes. Reverify the **entire** batch
and canonical union using the local OS-store anchor, without requiring SQLite.
A still-conflicting union
remains held. A final held decision never automatically clears, even if the
current union is Ready; M8 supplies no reviewed resolution/override operation.
An interrupted intent with no held decision can abort only after complete
verification proves Ready or a clean input rejection. Missing/changed evidence,
unknown versions, partial writes and mismatched identity preserve the evidence
and refuse startup. An orphan staging sentinel also refuses legacy cleanup, so
a missing journal intent cannot turn into an ordinary unprepared abort.

Recovery-only revalidation is separate from publication. Mac's conservative
startup guard must continue refusing these markers; this change must not relax
its missing filesystem publication adapter. Native tests must distinguish that
scope from Windows end-to-end publisher/restart checks.
