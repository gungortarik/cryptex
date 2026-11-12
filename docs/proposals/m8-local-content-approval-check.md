# Disabled protected UI and native approval continuation

This extends the explicitly selected local-certificate startup experiment under
ADR-0020. Ordinary builds and all ordinary Mac product owner routes remain
closed. No release publisher, new platform floor or product activation is chosen.

## Content

The signed inventory lists actual built frontend files. After the complete
snapshot verifies, only members under Contents/Resources/frontend become an
immutable shared content catalog. The actual Tauri Context installs an Assets
provider over those bytes and discards the embedded provider. It clears devUrl,
forces a directory-style frontendDist, preserves compiled CSP for both normal
and development policy, and forces local App window URLs. Tauri's own index
routing fallback may still select verified index bytes; it cannot select disk,
embedded or development-server content through this provider.

The candidate has its own identifier and app-data directory inside the owned
temporary runtime. The actual window forces incognito WKWebView storage and
rejects navigation outside its local Tauri origin. Its builder registers health,
the three owner review commands and the bounded pairing exchange/approval
commands. It does not register arbitrary filesystem, shell, ordinary
capture/retrieval, autostart or product lifecycle commands.

## Bound requests and sessions

The native channel remains on one worker thread; only immutable assets and
typed queue messages cross threads. After actual Context installation, Desktop
sends the catalog binding. The daemon independently derives it from its retained
snapshot. The content-bound connection then mints a private non-clone received
request only after native per-message validation and closed decoding. Its
private allocation seal cannot be supplied by JSON, a PID or a boolean.

The adapter invokes the existing approval domain and journal. Each pending
session retains its original root, registry, store and admission. Prepared and
final replies retain admission through the actual native send; owner think-time
holds no operation lease. Peer process/endpoint loss, malformed requests,
mismatched or replayed followups, and delivery loss invalidate ephemeral grants.
No failure invents durable cancellation or rolls back an uncertain append.

Desktop retains wire capabilities privately and returns opaque local handles.
Pairing preparation independently verifies the signed role-specific prefix and
compares the entire returned snapshot, including both transport IDs, before
issuing a local handle. Confirmation/cancellation uses the saved exact prefix,
request ID and grant; renderer calls supply only the local handle. Owner and
pairing handles cannot cross families. All commands use the existing worker's
deadline, interest and failed-delivery invalidation rules, with no Unix fallback.

Native transport-identity reading is a closed, read-only exchange method; it
accepts no renderer-supplied identity or grant. It uses the same daemon getter
as offer/proof validation. There is no authenticated Mac managed-transport
status reader yet, so native transport read, offer and proof return
`transport_identity_unavailable`. This change must not manufacture an adapter
ID or count its refusal fixtures as successful end-to-end pairing.

Queued calls have an absolute deadline and interest lease; cancelled or expired
work cannot start later. Interest is checked again after native work and delivery;
successful callers disarm cancellation before dropping their interest guard.
Failed result delivery and all post-prepare failures
burn pending local state and the connection. Shutdown uses an out-of-band flag,
so a full work queue cannot prevent cleanup.

## Explicit synthetic profile

FakeSecureStore is selected only by the authenticated feature-specific content
mode, on a fresh owned TEMP directory containing only its fixed marker before
lifecycle initialization. The ordinary IPC server is omitted for that mode,
and the native driver consistently uses the same process-local synthetic store.
The daemon remains the only canonical writer. Enabling the feature alone does
not switch any ordinary runtime's store or unlock any product route.

The native fixture creates one public synthetic event, then exercises real
domain prepare/cancel/confirm. It verifies unchanged canonical prefixes,
one append, mismatched digest/replay refusal, lost connection and old local
handle isolation. Additional faults cover accepted prepare without its snapshot,
failed handle allocation, expired queued work and lost result receivers. A gated
real prepare cancels the caller while keeping the blocking receiver alive; a
second gate completes normal receipt before the worker's post-send check. The
first burns the connection/grant and the second still permits real cancellation.
Native subprocess tests also exercise concurrent process and endpoint loss
against a long receive deadline; the delayed loss is scheduling-based rather
than a deterministic observation of entry into the Mach receive syscall.
The synthetic fault dispatcher is not registered as a renderer command.

Headless Context/provider checks do not establish WebView loading or human
review. Fresh-connection handle tests do not establish same-runtime restart
recovery because the fixture store is intentionally ephemeral and the content
mode refuses existing profiles. Signed interactive acceptance, durable recovery
with the intended platform store and product activation remain separate gates.

The OS-store restart prerequisite is exercised independently by
`platform_restart_tests` on macOS. It seeds an isolated profile through daemon
library code, reloads the Keychain-backed key in two fresh processes, verifies
the canonical signatures, and compares every profile file byte for byte.
A separate process must report Missing after that fixture key is deleted,
leaving all profile files intact. The test removes its unique development
credential and runtime. It does not change the content-mode profile guard or
select a persistent store for the native candidate.

## Isolated native Keychain restart

The feature-only Desktop check now has separate Keychain-create and
Keychain-resume startup modes. The daemon authenticates the signed parent,
release snapshot, native connection and frontend binding on every launch.
Create accepts a fresh private TEMP root with the distinct durable test marker.
Resume requires that marker, the fixed synthetic Brain and a verified original
signed probe. It loads the existing development Keychain seed and verifies
canonical history before admitting native owner operations.

The check kills a daemon with a pending owner review, reopens that same profile
and rejects the previous local handle. It exercises cancellation and one
confirmation, then reopens again to verify the two events and byte-identical
canonical and identity files. The CI harness deletes that unique fixture seed
and requires a subsequent launch to fail with Missing while keeping profile
files intact and leaving the seed absent. Anchored journal recovery may detect
this loss before the content driver runs.

Runtime-ready attribution now precedes reading the ready file. The daemon sends
that native phase after replacing the progress file, preventing a leftover
previous PID from racing the new launch. The live-child and expected-PID checks
retain the same absolute startup deadline.

The existing ephemeral UI continues to use its fresh-profile fixture. The new
Keychain experiment is currently invoked by the headless native Desktop check.
The durable check also captures exact confirmation requests in a non-Send,
non-serializable replay probe retained on the same native owner thread.
It retains one pending request across a forced daemon exit and one successfully
used confirmation across a clean exit. New authenticated connections resend the
identical serialized request, including its request ID, transaction, digest and
capability. The adapter must reply with an explicit missing-session refusal.
The completed request is repeated twice in the final daemon. Every refusal is
followed by a status check and byte comparisons of canonical/identity files and
the journal. Fresh approvals remain exercised between these replay attempts.
The harness retains probes only in memory and exposes no renderer command.
Owner hardware acceptance of this replay extension remains a separate check.
