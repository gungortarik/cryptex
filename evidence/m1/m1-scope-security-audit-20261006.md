# M1 scope / security audit — 2026-10-06

**Branch:** `m1/brain-device-identity`
**Authority:** `docs/00_MASTER_SPEC.md`, accepted ADR-0015, M1 completion gate / DoD
**Actions:** not run (deferred)
**M1 complete:** **not declared**

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

Per M1 completion gate and `docs/21_DEFINITION_OF_DONE.md`, still open:

1. **Security review** for identity/trust handling — not complete.
2. **Required CI / GitHub Actions** — intentionally deferred; not green as a gate.
3. **Human review** of M1 — not complete.
4. **DoD checklist** as a whole — not claimed satisfied.

Do **not** start M2 or declare M1 complete until the above are closed.

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
- GitHub Actions: not run (deferred)
- M1 complete: **not declared**

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

## M1 security-review closeout status (2026-10-06)

Scoped security-review findings above are resolved in code on `m1/brain-device-identity`.

| Gate | Status |
|---|---|
| Scoped security-review code findings | Resolved (see findings above) |
| Gate C Windows + macOS SecureStore | VERIFIED (separate host evidence) |
| Local Windows checks (install + identity + lib + Clippy) | PASS at tip after this follow-up |
| Final GitHub Actions / required CI | **PENDING** — operator will run once against the final code SHA |
| Human review / DoD checklist | Open until CI result is recorded |
| M1 complete | **not declared** |

Do not start M2 until CI pending is closed and DoD/human review are satisfied.
