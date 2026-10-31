# M2 final macOS verification and dual-OS CI gate — 2026-10-08

**Status:** Platform checks and required CI gate PASS. M2 remains **IN PROGRESS** pending owner review; this record does not authorize merge or M2 completion.

**Branch:** `m2/immutable-event-store`  
**Tested source SHA:** `9ade9f7db18ec0e383594c19e592809e3b194531`  
**Workflow:** CI (`ci.yml`)  
**Run:** [37706283147](https://github.com/gungortarik/cryptex/actions/runs/37706283147)  
**Overall conclusion:** `success`  
**Run window:** 2026-10-08T00:09:54Z–2026-10-08T00:32:54Z

## Mac host verification

The owner-provided Mac report identifies the same branch and tested SHA, with a clean working tree:

| Field | Value |
|---|---|
| Host | Darwin / macOS 27.0.1 (26A434) |
| Architecture | arm64 |
| Branch / HEAD | `m2/immutable-event-store` / `9ade9f7db18ec0e383594c19e592809e3b194531` |
| `cargo test --locked -p cryptex-storage quarantine` | exit 0; 22 passed |
| `cargo test --locked -p cryptexd --lib` | exit 0; 121 passed |
| `cargo fmt --all -- --check` | exit 0 |
| `cargo clippy --locked -p cryptex-storage -p cryptexd --all-targets -- -D warnings` | exit 0 |

The Mac verification made no source changes, commits, or Actions dispatches. Source: the owner-provided `## Mac M2 four-check — PASS.txt` attachment; no raw log bundle was supplied.

## GitHub Actions job outcomes

All executed jobs ran against the exact tested source SHA above.

| Job | Conclusion | Notes |
|---|---|---|
| Readiness (`macos-latest`) | **success** | Includes workspace checks, M2 events/storage tests, schema drift, and platform readiness suite |
| Readiness (`windows-latest`) | **success** | Includes workspace checks, M2 events/storage tests, schema drift, release-only Desktop checks, and Node foundation suite |
| IPC cross-account ACL (Windows) | **success** | Different-user access denial verified |
| Windows Desktop package (unsigned, manual only) | **skipped** | Skipped by workflow design; not required by this gate |

## Scope and remaining gate

- The required macOS four-check, Windows four-check previously recorded in `m2-staging-cleanup-ownership-20261007.md`, and dual-OS CI are now recorded as passing for source SHA `9ade9f7`.
- The Windows extended-path test host observed `LongPathsEnabled=1`; this record and CI do **not** claim validation with `LongPathsEnabled=0`.
- Security/DoD dispositions remain in `m2-security-dod-review-20261007.md` and `m2-closeout-l1-l2-f4-f6-20261007.md`; this CI result does not replace owner review.
- M2 remains **IN PROGRESS** until the owner reviews the candidate and confirms the milestone gate. Do not merge or start M3 before that review.
- No additional Actions workflow was run for this documentation update.
