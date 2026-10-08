# M4 Slice 3 — Deterministic Knowledge Reducer

## Candidate

- Branch: `m4/knowledge-engine`
- Code commit: `45c78fda35036e85f1b25ba0ab50750db9625b26`
- Branch tip after manifest whitespace cleanup: `07f37291dc19911afb4b48ecdb3b53300c8574d4`
- Base includes the accepted ADR-0017 documentation change at `ee7e173`.
- Production v2 writing remains disabled.

## Implementation

- Added the pure `cryptex-knowledge` reducer crate. It consumes only a complete
  `VerifiedV2History`, does not write canonical events or SQLite, and performs
  no free-text extraction.
- Added deterministic normalization, entity/alias consolidation, explicit
  entity merge/revoke, cardinality-policy-v1 fact handling, relation/fact
  provenance, corrections, withdrawals, conflict resolution, and pending
  dependency handling.
- Pinned alias case-fold output to the repository's Unicode 16.0.0 lookup
  data; included its MIT and Unicode license texts.
- The history verifier now requires an explicitly named trusted-daemon key
  resolver. Receipt UUIDs remain provenance references and are not presented
  as independent proof of human action.
- Event-ID collision detection covers the complete verified history across v1,
  supported v2, and verified-unsupported v2 records. Exact-byte replicas are
  idempotent; differing verified bytes fail closed.
- Terminal conflict resolutions must name the complete correction and other
  competing resolution set. Known wrong-kind references produce diagnostics,
  remain out of safe materialization progress, and are distinct from missing
  dependencies.
- `cryptexd` call sites were updated only for the newly private verified-event
  fields; no production v2 writer or M3 retrieval behavior was enabled.

## Windows verification

On Windows, format, the combined locked package test run, strict Clippy, and
`git diff --check` passed:

```text
cargo fmt --all -- --check
cargo test --locked -p cryptex-core -p cryptex-events -p cryptex-storage -p cryptex-projection -p cryptex-knowledge -p cryptexd -- --test-threads=8
cargo clippy --locked -p cryptex-core -p cryptex-events -p cryptex-storage -p cryptex-projection -p cryptex-knowledge -p cryptexd --all-targets -- -D warnings
git diff --check
```

Test results: core **30**, events **60**, storage **39**, projection **47**,
knowledge **20**, cryptexd library **150**, lifecycle **9**, supervisor **3**,
and update rollback **2**; **360 passed**, no failures. Strict Clippy exited 0.

Security review covered trusted-daemon resolver semantics, cross-version event
ID collisions, terminal resolution completeness, wrong-kind references, and
safe materialization progress. Findings were fixed and re-reviewed; no blocker
remains in this scoped review.

## macOS verification

Owner-provided report: `M4 Slice 3 macOS verification — PASS.txt`.
Exact tested SHA `c4e692c2d335a3abdf8fa6edbe70dc38661e97dd` matches the
Windows-verified candidate above. Host: macOS 27.0.1 (Build 26A434), arm64;
clean detached checkout.

- `cargo fmt --all -- --check`: PASS (exit 0)
- Locked tests for core, events, storage, projection, knowledge, and cryptexd
  with 8 test threads: PASS (core 30, events 60, knowledge 20, projection 47,
  storage 38, cryptexd lib 145, plus lifecycle/supervisor/rollback suites)
- Strict Clippy: PASS (exit 0)
- `git diff --check`: PASS (exit 0)

The Mac run made no repository changes and dispatched no Actions. Combined
with the Windows verification above, Slice 3 has local verification on both
supported operating systems at the same source SHA. Slice 4 may begin.

No GitHub Actions were dispatched. M4 is not complete, production v2 writing
is not enabled, and no M5/M8/M10/M14 functionality was added.
