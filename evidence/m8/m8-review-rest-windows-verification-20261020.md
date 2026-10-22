# M8 review policy and bounded REST — Windows verification

**Date:** 2026-10-20  
**Source:** `codex/m8-device-trust`, uncommitted implementation on base
`74bc3b14127962579ae03838bafea9336612a033`. No new product commit SHA is claimed.  
**Scope:** Policy helpers and read-only REST adapter; not full M8.

## Delivered

- `services/cryptexd/src/peer_review.rs`: complete public identity/key candidate
  binding; bounded, single-use, cancellable, stale-snapshot-resistant local
  review; full-u64 cutoff arithmetic bound to Brain/device/full public key.
- `services/cryptexd/src/syncthing_rest.rs`: existing-credential-only loopback
  read adapter with explicit proxy/redirect/decompression refusal, exact native
  version/identity checks, streaming body bounds and a total deadline.
- Exact existing reqwest 0.13.5 pin, default features off, blocking only.
  Cargo.lock changes three dependency/feature edges; no version changes.
- ADR-0019 product-direction acceptance and the scoped implementation contract.

No production peer resolver, authenticated pairing transcript, durable trust
commit, new canonical schema, transport process controller or incoming-memory
admission is enabled. Status continues to report those components unimplemented.

## Final observed checks

Executed on the Windows host using pinned Rust 1.98.1, with target directory
`%LOCALAPPDATA%\Cryptex-Dev\build\m8-device-trust-target`.

| Command / observation | Result |
|---|---|
| `cargo fmt --all -- --check` | PASS, exit 0 |
| `cargo test --locked -p cryptexd -p cryptex-events --lib` | PASS: events 61/61, daemon 305/305 |
| New `peer_review::` cases included above | 25/25 PASS |
| New `syncthing_rest::` entries included above | 19/19 PASS, including one subprocess worker entry |
| `cargo test --locked -p cryptexd --bin cryptexd` | 8/8 PASS |
| `cargo clippy --locked -p cryptexd --all-targets -- -D warnings` | PASS, exit 0 |
| `cargo build --locked -p cryptexd --bin cryptexd` | PASS, exit 0 |
| Built daemon status against a missing runtime | PASS; target remained absent, no trust/protection overclaim |
| `git diff --check` | PASS |

Final binary SHA-256:
`f737bfe3155db96113a18116323831251cf10dfefe6b5231cfa18a42079b750b`.

Local full logs and LF-normalized eight-file source manifest:
`%LOCALAPPDATA%\Cryptex-Dev\m8-verification\policy-rest-final-20261020-87c1ce90bb234498aaa22641df4ac8fa`.
The repository manifest is `m8-review-rest-source-sha256-20261020.txt`.

## Failure retained and disposition

The first review run passed 23/24 cases. Its simulated backwards-Instant test
subtracted 1 ns, which can round to the same QPC tick on Windows. The test now
constructs a value one second earlier and asserts that it is actually earlier;
the fail-closed assertion remains. No production expiry check was relaxed.

The broad invocation `cargo test --locked -p cryptexd -p cryptex-events`
passed the 61 event library tests, then exited 101 because Windows Application
Control refused to execute the auxiliary `cryptex-events-schema-gen` test
binary (OS 4551). It never ran that target. This is not a full all-targets PASS.
No security settings, execution-policy workaround, renamed binary or alternate
loader was used. The relevant library suites and daemon CLI were then selected
explicitly and passed as listed; Clippy's all-targets compile check also passed.
The auxiliary launcher remains a Windows completion follow-up.

## Coverage and limits

Review checks cover wrong Brain/key/DeviceId, key reuse, weak key rejection,
cancel, replay, concurrent callbacks, replacement snapshots, RNG failure,
monotonic expiry, strict decimal parsing, cutoff boundaries including zero and
u64::MAX, mismatched bindings and order-independent minimum arithmetic.

REST tests use synthetic real loopback TCP/HTTP fixtures, not a native Syncthing
process. They cover API-header delivery, exact pin/identity mismatch, denied HTTP
status, credential redaction, secret-store missing/invalid state, no redirects,
proxy refusal in an isolated child environment, known/chunked/unknown-length
body bounds, exactly-at-limit JSON, malformed/duplicate/non-UTF-8/truncated
responses, stalled bodies and total budgets across both requests.

These tests do not prove peer authentication, real native process/profile
ownership, native ID checksum verification, OS secret provisioning, private
profile ACLs, bilateral approval or revocation issuer authority. Prefix algebra
is not an authenticated policy reducer. No `TrustedDaemonKeyResolver` consumes
the new helper types. Existing event golden-vector/strict-schema library tests
passed without schema/source changes to the events crate.

M8 remains open for ADR-0019 §8 implementation/security review, actual pairing,
owned transport lifecycle, incoming validation, offline convergence, revoke/
retire/compromise integration, native UI, remaining Windows gates and the later
consolidated Mac stage. No new Mac transfer is requested. No Actions, commit,
push, merge, personal-runtime access or M7 managed-note write-back.
