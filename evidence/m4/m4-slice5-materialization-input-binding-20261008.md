# M4 Slice 5 — Materialization Input Binding

**Status:** Partial Slice 5 implementation; Windows and owner-reported macOS
checks PASS on exact code SHA `4909013c5d9098db9391757a9caa2ca6f4c4c455`.

## Change

`rebuild_from_verified_history` now requires a one-to-one set of canonical
frame positions for the records in `VerifiedV2History`. It checks each event
ID, source device ID, and content hash against the cryptographically verified
record before reducing or writing any M4 row. Duplicate, missing, extra, or
mismatched positions fail closed. The regression proves a hash mismatch leaves
`knowledge_meta.rebuild_status` at `not_built`, while the matching verified
position permits the rebuild.

This binds the reducer's progress metadata to its verified input. It does not
claim the positions were obtained from a live canonical scanner; that
daemon-owned scanner integration remains the next Slice 5 work. Retrieval
safety enforcement also remains open. Production v2 writing remains disabled.

## Windows local verification

Using `CARGO_TARGET_DIR=%LOCALAPPDATA%\Cryptex-Dev\cargo-target-m4`:

- `cargo fmt -p cryptex-knowledge -- --check`: PASS
- `cargo test --locked -p cryptex-knowledge -p cryptex-projection`: PASS —
  knowledge 29, projection 47
- `cargo clippy --locked -p cryptex-knowledge -p cryptex-projection
  --all-targets -- -D warnings`: PASS
- `git diff --check`: PASS

## Owner-reported macOS verification

Exact tested HEAD: `4909013c5d9098db9391757a9caa2ca6f4c4c455` on
`m4/knowledge-engine`; macOS 27.0.1 (26A434), arm64; clean working tree.

- `cargo fmt -p cryptex-knowledge -- --check`: PASS
- `cargo test --locked -p cryptex-knowledge -p cryptex-projection`: PASS —
  knowledge 29, projection 47
- Strict Clippy for both crates: PASS
- `git diff --check`: PASS

Daemon scanner/retrieval integration remains open. GitHub Actions were not
dispatched. No canonical data or M3 schema behavior was modified.
