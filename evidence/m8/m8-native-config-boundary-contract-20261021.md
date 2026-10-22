# M8 Windows native configuration and managed session boundary

Date: 2026-10-21. Authority: Master Spec, ADR-0004 implementation addendum,
ADR-0014 and M8 contract. This implements explicit backend primitives, without
activating production startup/IPC, pairing, canonical security events or trust.

## Provisioning without replacement

`ManagedProfile::prepare_native` requires the existing exclusive private profile,
a pinned native package, the daemon SecureStore and two distinct nonzero backend
ports. Native generation runs through the atomic owned job in a private child
`generation/`; that directory is never served. The fixed command and minimal
environment retain the earlier package/process boundary.

Initialization intent and a generation-started marker are flushed before launch.
Only clean owned-tree exit plus bounded generated input validation allows a
generation-complete marker. File presence alone cannot prove successful generation.
A crash before this marker, including before launch or between native exit and
the marker, refuses further generation of that profile. It does not delete,
regenerate or silently replace a potentially existing identity. This conservative
gap requires explicit recovery/new-profile handling; automatic recovery is not
claimed for every interruption point or physical power loss.

Prepared config, certificate and native private key are written into a separate
private child `native/` using CREATE_NEW, explicit current-user ownership,
inherited private permissions and file flushes. Generated files are not edited
in place or atomically replaced by path. Interrupted staging accepts only
byte-identical existing private ordinary files. Extra staging entries, reparse
points, hard links, oversized/empty files, corrupt data or altered settings refuse.

The native-generated API key is preserved exactly and enrolled using the existing
immutable binding / Windows Credential Manager flow. A completed profile with a
missing, changed or denied credential never generates a replacement. A failed
first enrollment may resume with the identical staged identity/key. The ready
receipt binds ports/native ID and certificate/key digests; JSON is bounded and
rejects duplicate/unknown fields. Ready reopen validates all bindings and current
config before any process launch. No configured data is imported from a user
Syncthing installation.

`native-ready.json` means local preparation, not active replication, verified
Cryptex trust or incoming-event admission. Its digests are local corruption
checks, not a new signature/security-event protocol. Native certificate/private
key bytes remain in user-only native files; the API key necessarily also remains
in the private native XML as well as OS storage. The private generation directory
is retained, including its original native key files; no secure erasure is claimed.
PEM bounds/header checks are not a cryptographic certificate validator. Native
startup and its owned authenticated identity response must also succeed.

## Configuration policy

Reuse already-locked quick-xml **0.42.0**, with no package version upgrades or
external entity resolver. The supported pinned 2.1.5 native config is version
**52**. Input is UTF-8, at most 256 KiB, 4,096 elements, depth 16, 32 attributes
per element and 64-byte ASCII element/attribute names. XML 1.0 characters and
decoded attribute values are checked. DTDs, general text references, namespaces,
CDATA, comments and processing instructions refuse. Escaped Unicode attribute
values roundtrip through the standard XML writer; output is deterministic LF.
Unknown versions/root sections, duplicate security fields, unknown GUI fields,
active folders, extra peers, introduction/auto-accept settings and LDAP setup
refuse. Only the single native self device and inactive defaults are admitted.

Before the first server launch, set and verify:

- GUI `127.0.0.1:<api>`; enabled, TLS false, unauthenticated metrics false.
- One `tcp://127.0.0.1:<sync>` listener; no active folders or peer devices.
- Global/local announcement, relays, NAT and browser launch false.
- Independent upgrade interval 0, prereleases false; CLI also disables upgrades.
- Usage reporting -1; crash reporting and audit output false.

Ready reopen checks these values rather than rewriting a drifting config.
Changed native identity, certificate, key, API key, ports or policy settings
refuse. The engine may serialize its own compatible defaults during a clean
start; those changes do not authorize new peers or settings outside this policy.

## Process session and cleanup

`PreparedNativeProfile::start` validates again, then launches the pinned serve
role. It returns only after version/native-ID/key readiness succeeds on the
OS-attested owned socket. A shared monotonic 12-second startup deadline bounds
transient unavailability retries; each HTTP observation gets at most the remaining
budget or three seconds. Foreign owner, wrong identity/version and malformed
responses refuse. No API bytes go to an unattested/foreign connection. The
blocking API is intended for a backend blocking worker, not a nested async runtime.

A session exclusively borrows its prepared profile and independently retains
root/home lock pins. An in-use flag prevents replacement even if a caller forgets
the session; independent Arc ownership keeps its OS exclusion locks retained.
Unresolved cleanup poisons that prepared owner and retains profile/package pins
until daemon exit. This exceptional fail-closed resource leak emits structured
redacted diagnostics; it is not a clean stop or replacement authorization.

Explicit stop rechecks version/identity, requests authenticated shutdown and
waits at most 25 seconds for the entire owned job. Nonzero/forced exits refuse.
The private native log must be bounded to 1 MiB, valid UTF-8 and free of the
pinned engine's `Database failed to stop within 10s` failure. No log/config body
is returned in errors. Abandonment performs bounded forced cleanup and reports
it as forced. Graceful HTTP acknowledgement alone never means clean shutdown.

This remains a current-user process/file boundary, not a sandbox against same-user
injection, administrators or a hostile kernel. Other platforms refuse pending
their adapter/verification stage. No public CLI/Desktop command starts this code.

## Remaining gates

Automatic supervisor/restart policy, durable health/protection UI/IPC mapping,
ADR-0019's exact bootstrap/security-event/issuer-authority contract and security
review, trusted pairing, incoming validation, convergence/cutoff behavior and the
consolidated Mac stage remain open. M7 write-back stays deferred. No Actions,
commit, push, merge or M8 completion is authorized by this implementation.
