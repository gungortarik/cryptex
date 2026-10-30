# M8 Windows security-event signing primitives

## Change

Added typed v3 signing helpers for owner-approved `security.device_restricted`
and terminal `security.device_self_retired` records. The helpers use the
existing immutable v3 envelope, internal classification, canonical JSON,
content-hash and signature domains, then reparse/authenticate the generated
bytes before returning them. Restriction signing checks the target public-key
and signing-key-ID binding. Self-retirement signing rejects a cutoff that does
not precede the control event.

The APIs produce bytes only. They do not establish owner approval, verify the
target cutoff against canonical history, reserve a sequence, write a journal,
append an event, or activate a peer restriction. Those remain daemon gates.

## Verification

- `cargo test -p cryptex-events`: **86 passed**, plus compile-fail doctest **1
  passed**.
- Focused new signing tests: **2/2 passed**.
- `cargo clippy -p cryptex-events -- -D warnings`: **PASS**.
- `cargo fmt --all`: completed.
- `git diff --check`: **PASS**.

No native Windows package or macOS build was run for this slice. M8 remains
open.
