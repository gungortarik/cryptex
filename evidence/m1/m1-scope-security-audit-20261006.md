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
