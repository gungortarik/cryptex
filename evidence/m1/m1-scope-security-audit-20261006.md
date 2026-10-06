# M1 scope / security audit — 2026-10-06

**Branch:** `m1/brain-device-identity`
**Authority:** `docs/00_MASTER_SPEC.md`, accepted ADR-0015, M1 completion gate / DoD
**Actions:** final CI **PASS** - run [37533023641](https://github.com/gungortarik/cryptex/actions/runs/37533023641) @ `f40484dd6e4e8c9eee110eea68735cedf71410bb`
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

## Remaining M1 DoD / CI gaps (block completion)

Updated after final CI PASS (run 37533023641). Per M1 completion gate and `docs/21_DEFINITION_OF_DONE.md`:

1. **Security review** for identity/trust handling - scoped findings **RESOLVED** (see findings + closeout below).
2. **Required CI / GitHub Actions** - **PASS** on tip `f40484d` (run 37533023641); unsigned Windows package job skipped by design.
3. **Human review** of M1 - **still open** (not recorded complete in repository evidence).
4. **DoD checklist** as a whole - **OPEN** until human review is recorded (concise checklist in closeout).

Do **not** start M2 or declare M1 complete until human review is closed.

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
- GitHub Actions: see final CI PASS below (run 37533023641)
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

**Not changed (requires owner decision in authoritative docs first):**
- Daemon startup / `prepare_m1_runtime_foundation` still binds install identity + sequence store only; it does **not** auto-create Brain identity.
- Brain public metadata remains under the runtime `identity/` path used by current M1 wiring; no relocation to a different canonical layout.

### Owner decisions still required (do not invent)

These are **not** implemented; resolve in Master Spec / ADR-0015 / M1 contract before code changes:

1. **Daemon startup Brain bind:** Should `cryptexd` hold/login-hold create or require a Brain identity, or keep Brain creation as an explicit product/setup step (current behavior)?
2. **Brain metadata placement:** Is runtime `identity/brain_identity.json` the accepted durable location for M1, or must public Brain metadata live under a different canonical path/layout?

Until those are decided in authoritative docs, do not change startup or placement. M1 remains incomplete (human review still open).

## M1 security-review closeout status (2026-10-06)

Scoped security-review findings above are resolved in code on `m1/brain-device-identity`.

### Final required CI (recorded)

| Field | Value |
|---|---|
| Result | **PASS** (workflow conclusion `success`) |
| Run | [37533023641](https://github.com/gungortarik/cryptex/actions/runs/37533023641) |
| Workflow | CI (`workflow_dispatch`) |
| Head SHA | `f40484dd6e4e8c9eee110eea68735cedf71410bb` |
| Jobs PASS | Readiness (windows-latest); Readiness (macos-latest); IPC cross-account ACL (Windows) |
| Jobs skipped by design | Windows Desktop package (unsigned, manual only) |

This records the final-Actions gate as PASS for M1. Historical Gate C host evidence files may still say Actions were not run for those host procedures; that is host-run context, not this final CI result.

### Concise DoD evidence checklist (M1)

Authority: `docs/21_DEFINITION_OF_DONE.md` + `docs/milestones/M1_BRAIN_DEVICE_IDENTITY.md` completion gate.

| DoD / completion item | Status | Evidence |
|---|---|---|
| M1 scope implemented (Brain/install/device identity, SecureStore, sequence foundation, pairing records, RK public metadata) | PASS | Milestone scope in cryptexd/core; handoff M1 status |
| Unit / failure-path tests for identity, SecureStore, sequence, install create-once | PASS | `cargo test -p cryptexd --lib` (local); readiness jobs in run 37533023641 |
| Format + lint | PASS | Readiness jobs (fmt + Clippy) on Windows and macOS in run 37533023641 |
| Cross-platform verification (macOS + Windows first-class) | PASS | Gate C Windows + macOS SecureStore VERIFIED (separate host evidence); readiness both OS in run 37533023641; Windows IPC ACL job PASS |
| Secrets in OS secure storage; no secret bytes in public files/logs (scoped) | PASS | SecureStore adapters + redaction/payload tests; Gate C matrices |
| Security review for identity/trust handling (scoped findings) | PASS | Findings in this audit marked RESOLVED (cleanup errors, install exclusivity, Unix parent sync) |
| Relevant tests/CI green | PASS | Run 37533023641 on `f40484d` |
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
| Final CI readiness Win+Mac + Win IPC ACL | PASS | [37533023641](https://github.com/gungortarik/cryptex/actions/runs/37533023641) @ `f40484dd6e4e8c9eee110eea68735cedf71410bb` |
| Unsigned Windows package job | Skipped by design | Same run (not required for this M1 CI closeout) |
| Scoped security-review findings | RESOLVED | This audit |
| Brain identity exclusive first-create | PASS | `identity_runtime` concurrent/failure tests |
| Human review | **OPEN** | Not recorded as complete in repo |

### Remaining action (blocks M1 complete)

**Owner/human review of M1 must be completed and recorded** (milestone completion gate: "human review is complete"). Until that is documented in repository evidence, do **not** declare M1 complete, do **not** merge `m1/brain-device-identity` as an M1 completion merge, and do **not** start M2.
