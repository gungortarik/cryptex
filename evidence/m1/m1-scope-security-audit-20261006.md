# M1 scope / security audit — 2026-10-06

**Branch:** `m1/brain-device-identity`
**Authority:** `docs/00_MASTER_SPEC.md`, accepted ADR-0015, M1 completion gate / DoD
**Actions:** current implementation candidate CI **PASS** - run [37542107413](https://github.com/gungortarik/cryptex/actions/runs/37542107413) @ `bcc2673dd92d1568d1a5da60816afd4bf041decf`.
**M1 complete:** **not declared** (human review open)

## Findings and resolutions

### 1. SequenceStore daemon authority (RESOLVED)

**Finding:** `SequenceStore` lived in `cryptex-core`, was publicly exported, performed durable disk writes, and had **no** `cryptexd` call site, while ADR-0015 §2.3 requires sequence allocation only by local `cryptexd` and §4 requires canonical identity metadata writes through the daemon.

**Resolution (M1-scoped, no new ADR):**
- Moved durable allocator to `services/cryptexd/src/sequence_store.rs` (daemon crate only).
- Added `sequence_runtime` product helpers under the app-data runtime root (`initialize` / `allocate` / `last_issued`) with repository-path refusal.
- `cryptex-core` retains only `DeviceSequenceCutoff` (M8 shape) — no durable counter writer.
- Existing sequence unit/crash/concurrent tests now run under `cryptexd`.

### 2. Install identity persistence (RESOLVED)

**Finding:** `InstallId` was a validated type only; no durable, daemon-owned install identity existed, while M1 / ADR-0015 §2.2 define install vs device and §4 requires daemon writes for identity metadata.

**Resolution:**
- Added `InstallIdentity` schema in `cryptex-core` (exact schema fail-closed).
- Added `cryptexd::install_runtime` to write/load/`load_or_create` `identity/install_identity.json` under the runtime root (create-once; stable across reload; duplicate write refused).

### 3. Gate C evidence references (VERIFIED intact)

| Host | Evidence file | Claimed SHA | Ancestor of tip |
|---|---|---|---|
| Windows | `evidence/m1/gate-c-product-secure-store-windows-20261006.md` | `e7b83cfb95b565f5eb30d7e4a01c24485122f7b3` | yes |
| macOS | `evidence/m1/gate-c-product-secure-store-macos-20261006.md` | `610733f8f911c67f341694c4333e34070b706d50` | yes |

ADR-0014 / ADR index / handoff point at these files. Neither host’s Gate C result is inferred from the other. Screenshot-only WAITING notes remain historical context where present; VERIFIED claims are the dated host-run matrices.

## M1 DoD status after current-candidate CI

The initial audit identified security-review and current-candidate CI as open. Scoped
security findings have been resolved and the required Windows/macOS readiness + Windows
IPC ACL workflow passed on candidate `bcc2673` (run 37542107413). The unsigned Windows
package job is skipped by design. **Human review remains open**, so the DoD table below
and M1 completion gate remain open. Do **not** start M2 or declare M1 complete until the
owner review is recorded.

## Local checks for this audit fix (Windows)

- `cargo test -p cryptex-core` PASS
- `cargo test -p cryptexd --lib` PASS (77)
- `cargo clippy -p cryptex-core -p cryptexd --all-targets -- -D warnings` PASS

## Follow-up security review (2026-10-06, tip after wiring)

### Finding: test-only install/sequence call sites

**Contract determination:**
- **Install identity:** ADR-0015 section 2.2 defines `install_id` as local runtime binding (new install => new id). M1 therefore requires the **live cryptexd process** to bind/persist install identity when it holds a runtime root — not library tests alone.
- **Sequence allocation:** ADR-0015 section 2.3 / acceptance criteria require a daemon-owned durable allocator foundation with tests. Kickoff slice 6 "Daemon wiring" lists Brain/SecureStore public metadata, not product IPC or event issuance. Sequence **allocate** has no M1 consumer (events are M2; cutoff productization is M8). Opening the daemon-owned sequence store directory at hold time is the M1 runtime bind; allocate remains a cryptexd library API for later milestones.

**Resolution:**
- `prepare_m1_runtime_foundation` ensures install identity + opens sequence store dir.
- `cryptexd lifecycle hold` / `login-hold` call that helper after `ensure_runtime_root`.
- Process harness `hold_binds_install_identity_and_sequence_store_dir` verifies binary wiring.
- Limitation recorded: no M1 sequence allocate CLI/IPC; no overstated product integration.

### Finding: RFC 3339 impossible values

**Resolved:** `parse_rfc3339_str` now rejects impossible calendar/clock/offset values (schema shape unchanged).

### Finding: discarded compensating cleanup errors (RESOLVED)

**Finding:** `rollback_brain_identity` discarded `remove_file` errors (`let _ = ...`), and verifying create paths ignored compensating `SecureStore::delete` failures after a primary create/verify failure. Callers could treat a failed create as cleanly rolled back (and safe to retry) even when public identity removal or private-material delete did not succeed.

**Resolution (M1-scoped):**
- `rollback_brain_identity` returns `IdentityRuntimeResult` (NotFound is success; other IO errors surface).
- Typed error `IdentityRuntimeError::CompensatingCleanupFailed` preserves `primary` plus optional `public_rollback_error` / `private_cleanup_error`.
- Explicit flags `public_rollback_succeeded` and `private_cleanup_succeeded` describe each cleanup step.
- `whole_operation_retry_safe()` is always `Some(false)` for this variant — any compensating cleanup failure means whole-operation retry must not be implied.
- Display reports orphaned private credentials when SecureStore delete fails and never claims whole-operation retry is safe.
- `FakeSecureStore::fail_next_delete_with` supports delete-failure injection.
- Focused tests: public rollback failure is unsafe to retry; private cleanup failure surfaces orphaned credential and is unsafe to retry; clean put-failure after successful rollback still permits retry (plain `SecureStore` error).

### Follow-up: retry_safe implied whole-operation retry after private cleanup failure (RESOLVED)

**Finding:** The earlier `retry_safe` field was `true` whenever public Brain-file rollback succeeded, even if compensating `SecureStore::delete` failed and left an orphaned private credential. Callers could treat `retry_safe_after_cleanup() == Some(true)` as permission to retry the entire Brain creation.

**Resolution:** Replaced `retry_safe` with explicit `public_rollback_succeeded` / `private_cleanup_succeeded` (and corresponding error fields). Whole-operation retry is never safe on `CompensatingCleanupFailed`.

**Local checks (Windows, this follow-up):**
- `cargo test -p cryptexd --lib identity_runtime` PASS (5)
- `cargo test -p cryptexd --lib` PASS (84)
- `cargo clippy -p cryptexd --all-targets -- -D warnings` PASS
- `git diff --check` PASS
- GitHub Actions: current-candidate final CI PASS (run 37542107413)
- M1 complete: **not declared** (human review still open)

### Finding: install-identity first-create race / overwrite (RESOLVED)

**Finding:** `write_install_identity` used exists-then-rename. On Windows, Rust `fs::rename` replaces an existing destination, so concurrent `ensure_install_identity` / `load_or_create_install_identity` callers could overwrite the winning `install_id`.

**Resolution:**
- Exclusive no-replace placement: Windows `MoveFileExW` without `MOVEFILE_REPLACE_EXISTING`; Unix `hard_link`.
- Concurrent losers load the persisted winner; explicit duplicate `write_install_identity` still returns `AlreadyExists`.
- Concurrent first-create regression asserts one shared ID and unchanged persisted bytes.

### Finding: Unix install-identity parent directory not synced after hard_link (RESOLVED)

**Finding:** After `hard_link(tmp, dest)`, success was returned without syncing the parent directory, so the new directory entry might not survive power loss. Dest cleanup on sync failure used discarded `remove_file` errors.

**Resolution:**
- Unix path syncs the parent directory after hard_link and only then returns success.
- Sync failure removes the uncommitted destination link; cleanup failures are compounded into the returned error (never silently discarded); create is never reported committed on sync failure.
- Injectable Unix hooks (`place_new_file_no_replace_unix`) cover sync-failure (dest removed, not committed) and sync+cleanup failure (both errors reported).
- Windows placement path unchanged (`MOVEFILE_WRITE_THROUGH`, no replace).


### Finding: Brain identity first-create race / overwrite (RESOLVED)

**Finding:** `write_brain_identity` used exists-then-`fs::rename`. On Windows, Rust `fs::rename` replaces an existing destination, so concurrent first-create callers could overwrite the winning `brain_id`, violating ADR-0015 section 2.1 ("`brain_id` is not silently rewritten in place").

**Resolution (M1-scoped; mirrors install-identity exclusivity):**
- Exclusive no-replace placement: Windows `MoveFileExW` without `MOVEFILE_REPLACE_EXISTING`; Unix `hard_link` + parent-directory sync before success.
- Unique synced staging temps; losers return `AlreadyExists` without mutating the winner.
- Tests: concurrent first-create single winner; failure-path bytes unchanged / no tmp leftovers; Unix injectable parent-sync failure does not commit.

### M1 lifecycle and placement boundary (recorded from the accepted scope)

The M1 kickoff specifies synthetic Brain creation through daemon wiring with public metadata under app-data; the Master Spec and ADR-0015 classify Brain identity as canonical and immutable. The resulting boundary is recorded in ADR-0015 §2.1.1, `docs/06_STORAGE_ARCHITECTURE.md` §7, and the M1 contract:

- `hold` / `login-hold` neither create nor require a Brain identity; creation is an explicit daemon-owned setup operation, not a startup side effect.
- `identity/brain_identity.json` under the selected app-data root is durable canonical metadata, not disposable runtime-control state or a derived cache. Normal shutdown/restart and application upgrade preserve it.
- M2 event records bind to the existing identity, M8 replication transfers it unchanged, and M11 restore preserves it. These future behaviors are not claimed as M1 evidence.

This clarifies current accepted scope; it does not implement startup creation, replication, or recovery.

### Brain identity persistence cleanup (RESOLVED)

Staging write/flush and placement failures now attempt temp cleanup; cleanup failures are returned with the primary failure and mark whole-operation retry unsafe. Rollback directory enumeration and individual entry errors are surfaced rather than ignored. Unit tests inject staging write and cleanup failures; Unix tests cover staging-link cleanup and parent-sync behavior.

## M1 security-review closeout status (2026-10-06)

Scoped security-review findings above are resolved in code on `m1/brain-device-identity`.

### Prior required CI on earlier source candidate (historical; superseded below)

| Field | Value |
|---|---|
| Result | **PASS** (workflow conclusion `success`) |
| Run | [37533023641](https://github.com/gungortarik/cryptex/actions/runs/37533023641) |
| Workflow | CI (`workflow_dispatch`) |
| Head SHA | `f40484dd6e4e8c9eee110eea68735cedf71410bb` |
| Jobs PASS | Readiness (windows-latest); Readiness (macos-latest); IPC cross-account ACL (Windows) |
| Jobs skipped by design | Windows Desktop package (unsigned, manual only) |

This earlier CI result was PASS, but it covered the earlier source SHA only. The current-candidate CI result is recorded in the DoD table and platform gate table below. Historical Gate C host evidence files may still say Actions were not run for those host procedures; that is host-run context, not the CI result.

### Concise DoD evidence checklist (M1)

Authority: `docs/21_DEFINITION_OF_DONE.md` + `docs/milestones/M1_BRAIN_DEVICE_IDENTITY.md` completion gate.

| DoD / completion item | Status | Evidence |
|---|---|---|
| M1 scope implemented (Brain/install/device identity, SecureStore, sequence foundation, pairing records, RK public metadata) | PASS | Milestone scope in cryptexd/core; handoff M1 status |
| Unit / failure-path tests for identity, SecureStore, sequence, install create-once | PASS | Local focused tests + Windows/macOS readiness jobs in run 37542107413 |
| Format + lint | PASS | Readiness jobs (fmt + Clippy) on Windows and macOS in run 37542107413 |
| Cross-platform verification (macOS + Windows first-class) | PASS | Gate C SecureStore is VERIFIED on both OS; current Brain identity Unix/macOS failure-path tests passed in run 37542107413 |
| Secrets in OS secure storage; no secret bytes in public files/logs (scoped) | PASS | SecureStore adapters + redaction/payload tests; Gate C matrices |
| Security review for identity/trust handling (scoped findings) | PASS (scoped) | Windows source review/local failure tests and Unix parent-sync/staging-link tests passed on current candidate |
| Relevant tests/CI green | PASS | Run 37542107413 passed Windows/macOS readiness + Windows IPC ACL on implementation candidate `bcc2673`; unsigned package skipped by design |
| Human review complete | **OPEN** | No repository artifact records owner/human M1 review as complete |
| Definition of Done / M1 completion gate fully satisfied | **OPEN** | Blocked on human review |
| M1 complete / merge / start M2 | **not declared** | Remaining action below |

### Platform / security / acceptance gates (from repository evidence)

| Gate | Status | Evidence |
|---|---|---|
| ADR-0015 Brain/device identity foundations | ACCEPTED | ADR + ADR index |
| ADR-0014 SecureStore P1 | ACCEPTED | ADR + pins |
| Gate C Windows product SecureStore | VERIFIED | `evidence/m1/gate-c-product-secure-store-windows-20261006.md` |
| Gate C macOS product SecureStore | VERIFIED | `evidence/m1/gate-c-product-secure-store-macos-20261006.md` |
| Final CI readiness Win+Mac + Win IPC ACL | PASS | [37542107413](https://github.com/gungortarik/cryptex/actions/runs/37542107413) @ `bcc2673dd92d1568d1a5da60816afd4bf041decf` |
| Unsigned Windows package job | Skipped by design | Same run (not required for this M1 CI closeout) |
| Scoped security-review findings | PASS (scoped) | Windows local checks + current-candidate Windows/macOS CI |
| Brain identity exclusive first-create and staging cleanup | PASS | `cargo test -p cryptexd --locked` (93 tests), `cargo test -p cryptex-core --locked` (28 tests, isolated target due Smart App Control), strict Clippy, rustfmt; Unix-specific tests passed in macOS CI 37542107413 |
| Human review | **OPEN** | Not recorded as complete in repo |

### Remaining action (blocks M1 complete)

**Owner/human review of M1 must be completed and recorded** (milestone completion gate). CI is green on candidate `bcc2673`; until the owner review is documented in repository evidence, do **not** declare M1 complete, do **not** merge `m1/brain-device-identity` as an M1 completion merge, and do **not** start M2.
