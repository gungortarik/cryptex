# M8 local canonical verification evidence

Implementation of the daemon-verified digest requirement already accepted in
`docs/proposals/m8-product-flow-activation-20261115.md`. This is an unsigned local
diagnostic and comparison artifact. It is not a remote acknowledgement protocol,
publication authority, protection-state transition, or milestone acceptance.

## Invocation and retained state

`cryptexd replication verify-local` requires explicit runtime, environment, run
UUID, Brain UUID, local device UUID and peer device UUID. The application must
be normally stopped. The verifier pins the existing runtime before acquiring
the ordinary daemon instance lock; a running owner refuses. No force-stop,
identity creation, key replacement, canonical repair, sealing or projection
rebuild occurs. Instance/coordination locks may be created or updated.

Import staging must be empty and its journal may contain only the ordinary
single-link import lock. Any pending transaction, policy-review/held marker,
unknown entry or unsafe namespace refuses without deletion or recovery.
The verifier loads the existing OS-held signing identity and uses the normal
locked local-anchor canonical scan and trust reducer. All canonical records,
including security controls and retained cutoff tails, must authenticate.
The expected peer must be directly approved and have a canonical stream.
This is not a claim that either endpoint currently has transport authority.

The local receipt exposes public run/Brain/device IDs, total record/device counts
and one SHA-256 checksum. It never exports bodies, keys or per-note hashes.
Its checksum is SHA-256 over the fixed `cryptex-local-canonical-set-v1` domain
(with trailing NUL), Brain UUID UTF-8, big-endian u64 record count, and sorted
32-byte SHA-256 hashes of the complete authenticated canonical record bodies.
It includes signatures/security records and does not depend on pack boundaries,
filenames, active/sealed state, platform paths or enumeration order. This is
ordinary checksum aggregation, not new cryptographic authentication.

The report explicitly leaves remote durability, network delivery and derived
result verification false. Two reports can be compared only with the expected
shared run ID, Brain and reciprocal device identities and the exact diagnostic
schema. Matching checksums/counts establish the same locally verified record
set at those two observations. They do not prove the route, interruption, restart,
later freshness, or protection. Malformed/extra fields and invented protection
claims are refused. Keep original reports rather than promoting a match into a
product `Protected` state.

## Owner packet

One ZIP carries an exact-source CI Windows daemon and committed Mac source.
Windows verifies the binary checksum; Mac reuses the existing local certificate
and normal signed-build helper in a separate private cache. Existing apps,
pairing, credentials and canonical records remain intact. Keychain permission
can still require an owner response. Both launchers produce bounded result ZIPs;
the Mac build helper is not invoked as an enrollment or application launcher.
The packet asks for normal application quit and never kills an existing process.

This packet addresses only the missing canonical-set evidence. The owner has
already observed bidirectional notes and paused reopen. Physical interruption
and final cross-platform security acceptance still require separately attributed
evidence; do not repeat existing pairing/note tests or call this full M8 closure.
