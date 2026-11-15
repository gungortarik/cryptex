# R4 — Authenticated incoming conflict retention and restart

Review candidate: `1a042f96d922d002d2feb959ca890b0432249d97`.
Production R4 code is unchanged from `7170b04`; the later commits complete the
classifier integration fixtures. This is a bounded inactive Windows publisher
change, not full M8 acceptance or Mac publication/product activation.

## Contract and implementation

The owner explicitly requested restart-persistent safe waiting under the already
selected conflict/strict-terminal rules. The [recovery contract](../../docs/proposals/m8-incoming-policy-hold-20261114.md)
uses a distinct complete-batch review intent before classification, matching
private staging/journal inventories, a monotone held-seen latch, and a separate
intent-hash-bound held decision. No receipt is a prepared publication manifest.
Existing 16-file, 1-GiB/file, 4-GiB/batch limits apply; each receipt is at most64KiB.

All staged framing, schema, signature, existing key and chain checks finish
before the narrow semantic classifier. Static restriction validation also
finishes before a terminal-tail result. Unresolved restriction/admission
dependencies, strict retirement tails and Ready-union loss of an existing or
candidate admission retain the exact staged bytes and close shared admission.
Other invalid/missing/unknown-key histories do not acquire policy authority.
An authenticated competing-fork witness is not added by this change.

Revalidation precedes legacy staging cleanup in the existing local-state
startup recovery hook, which precedes foundation/transport/IPC/readiness. It
reopens closed canonical JSON receipts and exact inventories, checks identity,
length and hashes, and authenticates the full canonical/staged union using the
OS-held local key. It does not need SQLite, issue a publication proof, append a
held union, or rebuild derived state from that union. Live recovery invokes the
same path before it can clear admission. Generic/no-store recovery refuses the
new markers and preserves evidence; Mac publication adapters remain closed.

An interrupted unsealed intent can abort after complete Ready or clean input
rejection. A held-seen latch with missing decision, a final held decision, unknown
schema, changed/missing evidence or uncertain persistence cannot become such an
abort. Even a later Ready union cannot automatically clear a final held decision.
There is no routine M8 override or automatic canonical repair.

## Independent source review

The existing owner-authorized independent reviewer worked read-only; it ran no
tests/CI and changed no files. Review corrected the initial crash window with a
complete inventory before classification, added the monotone held distinction,
and moved terminal classification after static validation.

Three implementation findings were corrected: recovered Windows staging pins
now belong to the reconstructed session and are released by discard; receipt
and tree cleanup mutations use the existing nonblocking import slot and checked
namespace; exact inventory/hash validation precedes every clean intent close,
so changed evidence cannot take the malformed-input cleanup path.

The reviewer also required distinct classifier integration cases and corrected
a fixture that had incorrectly searched the content-only view for security
metadata. The final fixture reads authenticated policy history instead. At the
candidate above, the reviewer reported no remaining actionable source finding.

## Native validation

Exact-source [CI43657350874](https://github.com/gungortarik/cryptex/actions/runs/43657350874)
passed both native readiness jobs and Windows cross-account IPC ACL. Daemon
library: Mac553passed/7ignored and Windows681passed/5ignored, zero failures.
All14 new tests passed on Windows; all7 common tests passed on Mac. The remaining
7 publisher integration tests are Windows-only, not Mac publication acceptance.
The [structured receipt](m8-incoming-policy-hold-native.json) records exact
source/job IDs, all new test names and committed-source hashes. Formatting,
workspace/release lint, foundation/recovery tests, release builds and Node
boundary tests passed. Dedicated manual Desktop startup/identity/installer and
separate historical-reader jobs were not run. An earlier broader run was
superseded/cancelled after its daemon phases passed; only the final complete
run above supplies this receipt. Earlier format-only runs supplied diagnostics.
Local Rust/rustfmt execution remains blocked by Application Control4551; no
bypass was used. Native formatter diagnostics supplied the formatting changes.

Coverage includes genuine signed dependency, SCC, strict-tail and Ready-union
admission-removal publisher/restart cases; daemon-written C approval and content;
all shared admission clones; first-reopen clean Ready and whole mixed-malformed
batch aborts after actual child interruption; interruption before classification,
between held latch/decision, and after final decision; SQLite independence;
missing/corrupt/changed evidence, unsupported versions, busy import slot, and
distinct missing/mismatched OS-store signing seeds. Existing ordinary unknown-key,
bad-signature and divergent-input rejection tests remain required.

## Remaining scope

M8 remains OPEN. Final protocol acceptance, Mac filesystem publication adapters,
configured identity/OS policy, supported approval/session/incoming/protection-status
wiring and integrated product acceptance remain separate gates. No owner-device
package rerun, installer publication, feature activation or M9 start is claimed.
