# M4 Slice 5 — Materialization Input Binding

**Status:** Partial Slice 5 implementation; daemon feed/retrieval integration
remains open.

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

macOS verification and GitHub Actions have not been run for this Slice 5
change. No canonical data or M3 schema behavior was modified.
