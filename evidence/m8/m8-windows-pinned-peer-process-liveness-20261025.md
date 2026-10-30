# M8 Windows pinned peer-process liveness test

**Date:** 2026-10-25
**Source HEAD:** `ae61603b517f68b63d559ccb5ee77d744176e6a6` (local changes present)
**Host:** Windows NT 10.0.26300.0, x86_64-pc-windows-msvc
**Rust:** `rustc 1.98.1 (48a229cea 2026-09-01)`
**Scope:** test-only coverage of the existing Windows `DesktopProcessIdentity`
retained process handle. No authentication policy or production IPC behavior
changed.

## Probe

Added `desktop_process_identity_rejects_exited_peer_with_matching_pid` in
`services/cryptexd/src/ipc.rs`. The test creates a real local named pipe and
launches an isolated copy of the test binary as a synthetic client. It waits
for a bounded readiness marker, accepts the client connection, reads the
kernel-reported peer PID, and verifies it equals the spawned child's PID. It
then opens a process handle for that exact child, verifies that the pinned
identity matches while the connected client is alive, terminates/reaps the
child, and verifies that `matches_peer` rejects the same numeric peer PID
because the retained process object is signaled. A guard terminates/reaps the
child and removes the temporary readiness marker on every exit path.

This joins kernel peer-PID retrieval with the retained-handle liveness
predicate. It does **not** exercise actual PID reuse, executable signature/path
replacement, Desktop-to-daemon launch races, approval sessions, or an
adversarial same-user process. It is not a complete end-to-end authorization
or trust proof.

## Verification

- `cargo test -p cryptexd --lib ipc::tests` — **47/47 PASS**.
- `cargo test -p cryptexd --lib` — **438 passed, 0 failed, 4 ignored**.
- `cargo clippy -p cryptexd --lib --tests --release -- -D warnings` — PASS.
- `cargo fmt --all -- --check` — PASS.
- `git diff --check` — PASS.
- Test-owned worker processes were confirmed absent after execution.

This is Windows host test evidence only. It is not Mac peer-identity evidence,
an independent security review, protocol acceptance, trust activation, or M8
completion. No Actions, commit, push, or merge.
