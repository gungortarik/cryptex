# M4 Slice 1 — Strict V2 Reader and Compatibility Boundary

**Status:** IMPLEMENTED; local Windows verification PASS; cross-platform final gate pending  
**Branch:** `m4/knowledge-engine`  
**Code commit:** `2a382eb876d5646ff3e504dc3918423fa1594735`  
**Base:** ADR-0017 documentation acceptance commit `ee7e173b903574e914a1471edb37398446efef81`  
**Host:** Windows 11, AMD64  
**Toolchain:** rustc/cargo 1.98.1

## Scope delivered

- Added a version-dispatched read-only event reader. V1 still delegates to
  `validate_accepted_record_bytes`; its schema, canonical bytes, hash,
  signature, and recovery behavior are unchanged.
- Added strict v2 common-envelope validation: canonical JCS body, duplicate
  key and resource-limit checks, typed common fields, the accepted v2
  BLAKE3/Ed25519 domain-separated verification, and device-key binding.
- Classifies v2 as known-payload-pending, cryptographically verified but
  unsupported, or invalid. Unknown envelope versions are retained as raw,
  unverifiable records and never described as verified.
- Chain scans include cryptographically verified v2 records when validating
  device binding, strictly increasing sequence values, and cross-version hash
  links. This allows a later v1 append to use the verified chain tip.
- An unverifiable envelope creates a compatibility boundary: active recovery
  returns before quarantine/truncation; sealed packs are not quarantined; append
  and retrieval stop. Tests assert canonical bytes remain identical.
- M3's current verified-event feed refuses to return content from a stream that
  contains v2 records whose semantics are not yet materializable. The separate
  `RetrievalSafetyState` enforcement for direct M3 FTS/Desktop retrieval is a
  later M4 integration slice and remains a completion blocker.
- V2 signing/record construction exists only in `#[cfg(test)]` synthetic
  fixtures. No production v2 writer or activation mechanism was added.

## Verification

All commands below ran on Windows using
`CARGO_TARGET_DIR=%LOCALAPPDATA%\Cryptex-Dev\cargo-target-m4`.

| Command | Result |
|---|---|
| `cargo test --locked -p cryptex-events` | PASS — 45/45 |
| `cargo test --locked -p cryptex-storage` | PASS — 39/39 |
| `cargo test --locked -p cryptexd --lib canonical_writer:: -- --test-threads=8` | PASS — 21/21 |
| `cargo fmt --all -- --check` | PASS |
| `cargo clippy --locked -p cryptex-events -p cryptex-storage -p cryptexd --all-targets -- -D warnings` | PASS |
| `git diff --check` | PASS |

The focused regressions cover v1 golden compatibility and strict v1 rejection
of v2 fields; v2 signature tampering; known-pending, verified-unsupported, and
unverifiable classification; v1→v2→v1 chain links with sequence gaps; unknown
envelope preservation in active and sealed packs; verified-unsupported chain
continuation; retrieval blocking; and append-after-verified-unsupported.

## Security findings and remaining gates

- No production v2 writer was enabled. Trusted-device capability checks and
  downgrade productization remain outside this slice and production activation
  remains disabled.
- Direct M3 SQLite/FTS and Desktop retrieval do not yet consume a shared
  `RetrievalSafetyState`. M4 Slice 5 must enforce it before M4 completion; a
  Brain-wide hold must override ordinary search.
- This is Windows-local evidence only. macOS parity and the required single
  dual-OS Actions run remain for the reviewed final M4 candidate; Actions were
  not run for this slice.
- No M4 SQLite changes, M8/M10/M14 behavior, real user memory, or retained
  personal/Cortex data was used.

## Next

Proceed to Slice 2 — typed v2 structured-event payload validation and validation
vectors — only after reviewing this Slice 1 checkpoint. Keep production v2
writing disabled.
