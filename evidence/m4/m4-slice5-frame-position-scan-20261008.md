# M4 Slice 5 — Verified Frame Position Scan Foundation

**Status:** Partial Slice 5. This adds an offset-reporting variant to the
existing verified frame fold; daemon-to-M4 materialization and retrieval
safety are still open.

## Change

Added `fold_verified_frames_with_offsets`, which reports the original byte
offset of each validated frame header while preserving the existing
`fold_verified_frames` API and behavior. The existing API delegates to the new
implementation and discards offsets. No record bytes, framing rules, recovery
classification, or canonical write behavior changed.

Tests verify exact offsets for sequential frames and confirm the existing
truncated-tail verified-prefix behavior remains unchanged.

## Windows local verification

Using `CARGO_TARGET_DIR=%LOCALAPPDATA%\Cryptex-Dev\cargo-target-m4`:

- `cargo fmt -p cryptex-storage -- --check`: PASS
- `cargo test --locked -p cryptex-storage`: PASS — 41 tests
- `cargo clippy --locked -p cryptex-storage --all-targets -- -D warnings`: PASS
- `git diff --check`: PASS

macOS verification and GitHub Actions were not run. Next: use the shared
verified frame scan to bind raw record bytes and offsets to the trusted
versioned history, then persist the full M4 rebuild while the relevant device
canonical locks are held. Production v2 writing remains disabled.
