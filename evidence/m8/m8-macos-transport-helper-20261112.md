# Disabled Mac transport helper — implementation checkpoint

ADR-0021 records the owner's delegated selection of the proposed helper role.
This checkpoint adds a feature-only entry in the existing signed daemon before
ordinary lifecycle/canonical initialization. A purpose/root/release/epoch-bound
native channel authenticates the actual parent and captured helper child.

The helper's private session/group owns the unmodified pinned Syncthing 2.1.5
monitor/server tree. Existing profile/XML/API-credential and owned-socket HTTP
logic is reused through Mac adapters. New task-generation and established-socket
FD checks use public SDK declarations; no application bytes precede ownership
validation. A read-only peer-generation duplicate carries no owner authority.

The canonical daemon's private reader submits a fresh bounded helper observation
for each identity request. A ready cache is scheduling information only. Native
read/offer/proof dispatch uses this reader only in explicit transport fixture
create/resume modes; all ordinary Mac product gates remain closed.

An active helper marker precedes spawn. The parent retains the helper unreaped
through group cleanup; unresolved cleanup leaves the marker and prevents another
start. Clean and forced results remain distinct. The helper watches its parent
generation during engine waits, and a forced group cleanup terminates the helper
too; its daemon parent owns cleanup verification. Combined supervisor-loss
acceptance remains a separate required test, not a claimed Windows Job equivalent.

Local checks: Rust format, all-target daemon strict clippy and Python tooling
(76 tests, 5 platform skips) pass on Windows. No Mac execution is inferred from
those checks. Mac private-profile/protocol unit tests and the actual signed
helper/engine startup, foreign-socket refusal, reopen and helper-loss driver are
added to the scoped CI path. Their execution result is pending at this checkpoint.

The new source has not had independent review or owner-Mac execution. Native
Desktop positive pairing and remaining loss/credential cases still require
evidence before this source is described as completing the identity connection.
