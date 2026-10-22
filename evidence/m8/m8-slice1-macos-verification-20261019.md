# M8 Slice 1 macOS verification — owner-reported scoped PASS

**Date:** 2026-10-19  
**Base:** `74bc3b14127962579ae03838bafea9336612a033`  
**Candidate:** Uncommitted Slice 1 source patch; not a new product commit.

## Candidate identity

- Patch SHA-256: `f5cf1dda729d2329fe140cc809e926e6270622e52d3dc020b469390fd92cd2ab`.
- Bundle was extracted outside Git; every `SHA256SUMS.txt` entry verified.
- Fresh Mac worktree: `/tmp/cryptex-m8-s1-verify-Wnpert`; clean baseline before
  applying the source patch, then `git apply --check` / apply succeeded.
- `SOURCE_SHA256SUMS.txt`: all four source files matched.
- Windows source reinspection independently confirmed that the current four
  candidate source files still match that exact LF source manifest. No product
  source changed after the transferred candidate was verified on Mac.
- The dirty original Mac checkout at `87e9c4b` and designated read-only M7
  worktree were preserved.

## Reported checks

| Check | Result |
|---|---|
| `cargo fmt --all -- --check` | Exit 0 |
| `cargo test --locked -p cryptexd --lib replication_status::` | 12 passed |
| `cargo test --locked -p cryptexd --bin cryptexd replication_status_` | 3 passed |
| Strict daemon Clippy (`-D warnings`) | Exit 0 |
| Daemon-only build | Exit 0 |
| `git diff --check` | Exit 0 |

Built daemon SHA-256:
`29e12fa8f8bb2b5eefe0e99a7bee74816b79ccd11b6ef1cbca66796e32fff166`.

CLI smoke returned protection `not_configured`, transport controller / peer
authentication / incoming pipeline `not_implemented`, both metadata states
`missing`, and every pairing declaration count zero. The nonexistent synthetic
runtime child remained absent after the command.

## Evidence and limits

Owner report retained on Mac outside Git:
`/Users/tarikgungor/Downloads/Cryptex-M8-Slice1-20261019-d5cd44e7/evidence-macos/m8-slice1-macos-verification-2026-10-19.md`.

The Mac filesystem report and binary were not independently read from this
Windows session; the results above are supplied by the owner. Source/patch
identity was independently checked against the local transfer manifest.

Windows compilation/tests remain unverified because Application Control blocked
dependency build scripts (4551); see the Windows blocker record. This scoped
Mac PASS does not establish Windows parity, transport, authenticated pairing,
or M8 completion. M7 remains deferred with write-back disabled. No Actions,
commit, push, merge, personal runtime, or real peer trust was exercised.
