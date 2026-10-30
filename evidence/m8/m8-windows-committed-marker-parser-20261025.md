# M8 Windows committed-marker parser and startup hold

**Date:** 2026-10-25
**Branch:** `codex/m8-device-trust`
**Source HEAD:** `ae61603b517f68b63d559ccb5ee77d744176e6a6` (working tree changes uncommitted)

## Change

Added a closed v1 committed-marker parser in `cryptexd` for the proposed
`<transaction-id>.committed.json` record. It enforces the 4-KiB bound, ordinary
single-link no-follow file identity, read-time identity stability, exact closed
fields, protocol version 1, lowercase canonical UUID, lowercase 64-character
SHA-256 text, canonical JSON bytes, and filename/transaction binding.

Startup recovery now recognizes canonical final-marker and marker-temporary
names. Both are parsed against the same closed marker schema. A recognized
final marker returns a typed recovery hold because publication replay and
companion verification are not implemented; a recognized temporary marker
also holds. Marker evidence is preserved. This
change does not publish segments, complete markers, replay partial transactions,
or enable trust/transport.

## Verification

- Initial parser-stage `cargo test -p cryptexd --lib import_staging::tests` —
  **PASS, 25/25**.
- Added two subprocess interruption points: after marker-temp sync and after
  final-marker placement plus parent-sync return. Both restart probes return a
  typed hold and preserve the marker evidence.
- Focused `import_staging::tests` after interruption coverage — **PASS, 28/28**.
- The two committed-marker interruption probes were repeated with `TEMP`/`TMP`
  directed to each of `D:`, `E:`, and `F:` — **PASS, 2/2 per volume**. Windows
  volume inventory identified each as a healthy fixed NTFS volume. Together
  with the default `C:` run, this is process-crash coverage across four local
  NTFS volumes on this host, not across distinct filesystem implementations.
- `cargo test -p cryptexd --lib` — **PASS, 426 passed, 0 failed, 4 ignored**.
- `cargo fmt --all -- --check` — **PASS**.
- `cargo clippy -p cryptexd --lib --tests --release -- -D warnings` — **PASS**.
- `git diff --check` — **PASS**.

The focused tests cover canonical acceptance, unknown fields, noncanonical JSON,
unsupported version, malformed/uppercase digest, oversize, filename mismatch,
and startup fail-closed preservation of valid final and temporary markers when
replay is unavailable. The interruption probes emulate abrupt process exit;
they are not power-loss evidence, production publication/replay, or proof that
the underlying storage honored a durable-write request.

## Remaining gates

No production marker writer, canonical publisher, partial-publication replay,
locked trust/cutoff/`RetrievalSafetyState` admission, approval/bootstrap flow,
Windows marker-parent durability proof, macOS peer-binding run, or M8 completion
is claimed. No Actions, commit, or push.

## Read-only review follow-up and companion classification

An independent Cursor review of the parser/typed-hold slice reported no
Critical/High findings. It identified stale ADR implementation wording and a
stale recovery rustdoc description; both were corrected. It also noted missing
explicit evidence-preservation assertions for several conflicting-marker
combinations. The review did not re-run the reported test commands; follow-up
regression tests now cover malformed recognized marker bytes and final/temp
coexistence.

Follow-up implementation validates a recognized marker against its exact
prepared-manifest SHA-256, transaction ID, complete staged tree and file hashes,
and current canonical destination presence/content under inventory/device
locks. Missing companions, digest mismatch, malformed files, and destination
mismatch remain typed fail-closed holds. Complete and partial destination
matches are classified for diagnostics, but no marker is completed, no
canonical file is published, and no evidence is cleaned up. Focused
At that earlier checkpoint `import_staging::tests` passed **34/34**; full daemon
library tests passed **432** with 4 ignored; release Clippy, fmt, and diff checks
passed. A matching-subset classification test and temporary-marker
full-companion test were added later. The subsequent lock-bound revalidation
checkpoint below supersedes the test status here.

Follow-up after the independent review added explicit malformed recognized
final-marker and temporary-marker cases with a staging transaction present,
asserting exact marker and staged-file bytes survive startup recovery. The valid
temporary-marker companion test also asserts byte-identical preservation.
Verification on Windows, 2026-10-25:

- `cargo fmt --all -- --check` — PASS.
- `cargo test -p cryptexd --lib import_staging::tests` — **40/40 PASS**.
- `cargo test -p cryptexd --lib` — **440 passed, 0 failed, 4 ignored**.
- `cargo test -p cryptexd` — **440 library + 12 binary + 9 lifecycle + 3
  supervisor + 2 rollback tests passed; 4 ignored**.
- `cargo clippy -p cryptexd --lib --tests --release -- -D warnings` — PASS.
- `git diff --check` — PASS.

This closes only the explicit byte-preservation assertion gap in this parser /
typed-hold slice. It does not change the no-publication/no-cleanup boundary.

## Workspace library regression check

`cargo test --workspace` could not complete because Windows Application Control
blocked the generated Tauri WebView2 macro DLL (`os error 4551`). Retrying with
the desktop crate excluded reached a generated `cryptex-events-schema-gen`
test executable, which was also blocked by the same policy. No security
settings were changed. The supported non-Desktop library suite completed with
`cargo test --workspace --exclude cryptex-desktop --lib --quiet`: **671 passed,
0 failed, 4 ignored** across core, events, knowledge, projection, storage, and
daemon libraries. Desktop/UI tests remain unverified on this host.

The independent review noted that digest/tree companion validation started
before canonical lock acquisition. The lock-bound follow-up below now re-reads
and compares the marker and manifest and rechecks the complete tree under
canonical locks. The current path still always returns a typed hold. The review
found no Critical/High issue. It also found a stale recovery rustdoc comment,
now corrected; diagnostic mismatches fail closed inside the file verifier, and
the separate mismatch wording is only reachable for a state change during
inspection. Assertions explicitly preserve the staging transaction in
match/subset/mismatch cases, and a temporary marker with its full companion is
covered.

The checks are process-level integrity observations only. They do not implement
ADR recovery rows, trust/cutoff/`RetrievalSafetyState` revalidation, or establish
power-loss durability.

## Latest lock-bound companion revalidation follow-up

The observation-only recovery path now re-reads the recognized marker and its
prepared manifest after acquiring canonical inventory/device locks. It compares
the marker and parsed manifest with the pre-lock snapshot, rechecks the exact
manifest digest, and verifies the complete staged tree again under those locks
before classifying destinations. Marker/manifest replacement or staged-tree
change during lock acquisition therefore holds. The path still never completes
a marker, publishes, cleans up, or authorizes trust.

Verification on the latest working tree, Windows host, 2026-10-25:

- `cargo fmt --all -- --check` — PASS.
- `git diff --check` — PASS.
- `cargo test -p cryptexd --lib import_staging::tests` — **38/38 PASS**.
- `cargo test -p cryptexd --lib` — **436 passed, 0 failed, 4 ignored**.
- `cargo clippy -p cryptexd --lib --tests --release -- -D warnings` — PASS.

These runs supersede the earlier 4551-blocked attempt. Tests remain
process-level observations, not power-loss or cross-platform proof. Current
coverage includes explicit marker/manifest replacement rejection, malformed
recognized marker preservation, final+temporary conflict preservation, and a
valid marker plus prepared companion held byte-identically without cleanup.
