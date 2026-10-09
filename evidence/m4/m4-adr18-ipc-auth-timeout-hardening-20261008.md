# M4 ADR-0018 — IPC peer authentication and bounded frames

**Status:** Windows local verification PASS; macOS verification pending on this change  
**Branch:** `m4/knowledge-engine`

The first owner macOS attempt at commit `53d0182` found a compile-only API mismatch in the Darwin peer-PID adapter: `LocalSocketStream` does not implement `AsRawFd` directly. The adapter now obtains the raw descriptor from `LocalSocketStream::inner()` (the wrapped `std::os::unix::net::UnixStream`). No tests ran on that failed attempt. The fix must be rechecked on macOS at its new commit before parity is claimed.

## Security findings resolved in this change

- Sensitive `content.search` and `identity.enroll_local` requests no longer trust the caller-supplied `client_role` alone. `cryptexd` captures the OS parent Desktop identity when the retrieval endpoint starts and checks the kernel-reported peer PID before serving sensitive IPC requests.
- Windows uses named-pipe peer credentials and retains an `OpenProcess(PROCESS_SYNCHRONIZE)` handle to the original Desktop process. If that process exits, the handle is signaled; PID reuse cannot revive authorization.
- macOS uses `LOCAL_PEERPID` and also requires `getppid()` to remain the captured Desktop PID. If the Desktop exits and the daemon is reparented, retrieval fails closed; a reused PID is not sufficient.
- Unavailable peer identity fails closed for `content.search` and `identity.enroll_local`; health-only IPC remains available to its existing Desktop/CLI/MCP roles. Non-Desktop health clients are handled by a bounded worker pool; the accept loop handles authenticated Desktop requests directly. The package lifecycle starts `cryptexd` as a child of the Desktop.
- Desktop identity is checked by canonical executable path beside the daemon (`Cryptex` / `cryptex-desktop`), not by code signature or executable hash. This excludes ordinary unrelated launchers when the installation is intact; it does not resist a same-user attacker who can replace the sibling Desktop binary. Installation-directory write protection remains an assumption, not a verified gate in this change.
- IPC streams use nonblocking I/O with one total frame deadline: 2 seconds for incoming requests and 15 seconds for client exchange/response handling. Deadlines apply to slow-drip traffic too. Unauthenticated health clients run in a bounded four-worker pool; the accept loop handles authenticated Desktop connections directly, so stalled health clients cannot occupy the Desktop request path. Excess unauthenticated connections are dropped while the pool is full. Frame sizes and one-frame-per-connection semantics remain bounded; malformed-frame diagnostics remain distinct from oversized-frame diagnostics.

## Windows verification

- `cargo fmt --all -- --check` — PASS
- `cargo test --locked -p cryptexd --lib` — **180/180 PASS**
- `cargo test --locked -p cryptexd --bin cryptexd` — **2/2 PASS**
- IPC-focused tests — **12/12 PASS**, including kernel peer PID, live-parent identity binding, forged Desktop role rejection, health-role compatibility, bounded unauthenticated worker capacity, bounded/malformed frames, and stalled/slow-drip read/write deadlines
- `cargo clippy --locked -p cryptexd -p cryptex-desktop --all-targets -- -D warnings` — PASS
- `cargo check --locked -p cryptex-desktop` — PASS
- `pnpm --filter @cryptex/desktop test:search-policy` — **2/2 PASS**
- `pnpm --filter @cryptex/desktop build` — PASS
- `git diff --check` — PASS

A macOS cross-target check from Windows was attempted but cannot compile the bundled C dependencies because this host has no Apple C compiler/SDK (`cc` unavailable). This is an environment limitation, not a passing Mac check.

## Required macOS run

On the Mac, pull this change and verify the exact resulting commit SHA with:

```bash
git fetch origin
git checkout m4/knowledge-engine
git pull --ff-only origin m4/knowledge-engine
git rev-parse HEAD
cargo fmt --all -- --check
cargo test --locked -p cryptexd --lib ipc:: -- --nocapture
cargo test --locked -p cryptexd --lib
cargo clippy --locked -p cryptexd -p cryptex-desktop --all-targets -- -D warnings
cargo check --locked -p cryptex-desktop
pnpm --filter @cryptex/desktop test:search-policy
pnpm --filter @cryptex/desktop build
git diff --check
```

The IPC test suite exercises the platform peer-PID API, including Darwin `LOCAL_PEERPID` and the live-parent check. No Actions were dispatched. This closes only the reviewed IPC findings after Mac verification; M4 acceptance/security/visual reviews remain open. Production v2 writing remains disabled.
