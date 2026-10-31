# ADR-0014 — OS Secure-Store Integration

**Status:** ACCEPTED

**Date drafted:** 2026-10-05

**Date revised:** 2026-10-06 (owner acceptance — P1 direct platform APIs)

**Date accepted:** 2026-10-06

**Milestone:** M1 — Brain + Device Identity

**Authority:** `docs/00_MASTER_SPEC.md`, `docs/08_SECURITY_DEVICE_TRUST_AND_RECOVERY_ROOT.md`, `docs/15_PLATFORM_MACOS_WINDOWS.md`, `docs/milestones/M1_BRAIN_DEVICE_IDENTITY.md`, `docs/30_ADR_INDEX.md`, `AGENTS.md`

This record proposes how Cryptex will integrate **OS secure storage** for device private material — the choice left unresolved by `docs/08` §21. It does not select event-signature/hash libraries (M2), Recovery Kit containers (M11), or pairing/revocation product UX (M8).

**Accepted.** Cryptex selects **P1 (direct platform APIs)** per `docs/adr/ADR-0014-os-secure-store-decision-addendum.md` and Gate A evidence. Product SecureStore implementation may proceed under `cryptexd` authority. K2 and high-level `keyring` remain unselected. Gate C both-OS product verification is recorded VERIFIED 2026-10-06 (separate Windows and macOS evidence) and remains distinct from this ADR acceptance.

## 1. Context

Master Spec and `docs/08` require:

- device private keys live in OS secure storage
- no custom cryptography
- `cryptexd` is the only supported canonical writer / privileged secret holder for device credentials
- secrets must not appear in repository files, generated Markdown, or privacy-unsafe logs
- macOS and Windows are first-class; platform APIs sit behind adapters

`docs/08` explicitly does **not** select the secure-store library/API. M0 did not accept production OS secure-store behavior.

V1 product constraint:

- **Do not require hardware-backed, non-exportable keys** (TPM / Secure Enclave / “never leaves the chip”). Full-disk encryption (FileVault / BitLocker) remains the primary local at-rest assumption.

## 2. Decision loop (acceptance vs spike vs M1 gate)

These gates are intentionally separate so acceptance cannot deadlock on unfinished cross-platform product proof, and product code cannot start from an unaccepted choice.

| Gate | What it allows / requires | Physical Win/Mac role |
|---|---|---|
| **A — PROPOSED + limited evaluation spike** | Owner may authorize a **product-separated** spike (tools/evidence only) to exercise candidate OS APIs and candidate Rust bindings. Spike code must not become `cryptexd` product secure-store, must not pin workspace dependencies for production, and must not write secrets into the repo. | Spike may gather **evaluation evidence** on one or both OSes. Incomplete Mac cells stay **WAITING**, not “pass”. |
| **B — ADR ACCEPTED** | Requires owner acceptance of the **decision** after spike comparison evidence is recorded (which candidates were tried, limitations, recommendation). An accepted revision or addendum then names the selected approach and exact pins. **Product secure-store implementation may start only after B.** | ADR acceptance requires enough spike evidence to choose among candidates **without treating unverified platforms as verified**. Both-OS **product** verification is **not** an ADR-acceptance prerequisite if the decision text and spike notes still mark unfinished platforms WAITING. |
| **C — M1 completion gate** | M1 secure-store scope is done only when the **accepted** approach is implemented behind daemon authority and the required failure/security tests pass on **both** first-class platforms per DoD / M1 contract. | **Windows and macOS physical verification of the accepted product integration are M1 completion requirements**, not a substitute for gate B. Gate A spike evidence is complete on both OSes; **product** physical Gate C is recorded VERIFIED on Windows and macOS (2026-10-06). |

Summary:

- **No product code** selecting a secure-store backend before ADR acceptance (gate B).
- **Limited spike** may run while PROPOSED when the owner authorizes it (gate A).
- **Both-OS physical product verification** = **M1 completion gate** (gate C), distinct from ADR acceptance.

## 3. Architectural direction (accepted: P1)

Integrate OS credential storage through a thin daemon-owned Cryptex `SecureStore` trait whose backends target:

| Platform | Native API surface under evaluation | Typical backing store |
|---|---|---|
| Windows | Credential Manager (`CredWrite` / `CredRead` / `CredDelete`, generic credentials) | Per-user Credential Manager vault |
| macOS | Keychain Services (generic password / keychain item) | Per-user login keychain |

**Cryptex policy constraints (proposed as fixed once accepted):**

1. Only `cryptexd` (and tightly scoped harnesses) may read/write device private material.
2. Desktop / CLI / MCP / UI never receive raw private keys.
3. Service/account naming isolates `Cryptex` vs `Cryptex-Dev` and namespaces by public ids without embedding secret bytes in names.
4. Errors are typed and redacted; diagnostics never log secret payloads.
5. In-memory / fake store is allowed for unit tests; CI must not depend on interactive keychain prompts.
6. Hardware-backed / non-exportable attributes are optional future hardening, not a V1 requirement.

## 4. Candidates compared (P1 selected)

Current public guidance for the Rust `keyring` ecosystem recommends that applications which need to audit the repository and control platform behavior prefer **`keyring-core` plus explicitly selected platform providers**, rather than treating the high-level `keyring` facade as an unaudited default. Cryptex therefore **does not** treat high-level `keyring` as the default selection.

| ID | Candidate | What it is | Audit / control | Notes |
|---|---|---|---|---|
| K1 | High-level `keyring` facade | Convenience crate selecting platform backends for the app | Weaker for apps that must audit exact provider wiring | **Not the default.** May be measured in a spike only as a convenience baseline, not presumed winner. |
| K2 | `keyring-core` + **explicit** Windows/macOS providers | Core API plus Cryptex-chosen provider crates | Stronger: providers are selected and reviewable in Cargo.toml | Aligns with “audit the store path” guidance; still requires spike evidence before pin. |
| P1 | Direct platform APIs | `windows` Cred\* APIs + macOS Security-framework / Keychain bindings behind Cryptex adapters | Maximum explicitness; no keyring facade | Higher maintenance; valid and must be compared fairly against K2. |
| X1 | DPAPI-only files under runtime root | App-data blobs | Rejected for device private keys | Master Spec requires OS secure storage for device private keys. |
| X2 | Require TPM / Secure Enclave non-exportable keys | Hardware-bound keys | Rejected for V1 | Not required by repo; not an M1 bar. |
| X3 | Private keys in replicated canonical files | Files in sync set | Rejected | Violates secure-storage and trust boundaries. |

**Selection:** Gate A compared K2 and P1 on Windows and macOS. **P1 is accepted.** K2 is not selected (error-redaction FAIL on both platforms due to payload-bearing `BadEncoding` Debug). K1 remains non-default / unselected.

## 5. Platform API comparison and limitations

### 5.1 Windows — Credential Manager

**Strengths:** per-user secret storage; mature Cred\* API; no custom crypto.

**Limitations:** same-user process access (same class of risk as ADR-0007 local IPC); enterprise/Credential Guard policy can change availability; some automation profiles are non-interactive; does not imply TPM sealing.

### 5.2 macOS — Keychain Services

**Strengths:** standard per-user secret storage; service/account labeling fits naming policy.

**Limitations:** permission prompts depend on codesign/entitlements/ACL — ad-hoc M1 pilots may differ from signed production; headless CI often cannot complete interactive Keychain UI; Secure Enclave non-extractable keys are a separate stricter surface (**not required**).

### 5.3 Shared product implications

Equivalent guarantee: device private material lives in OS-protected secret storage — not Git, Markdown, logs, or ordinary runtime files. Platform adapters own API differences; domain code sees only `SecureStore`.

## 6. Evidence status

| Evidence cell | Status | Gate |
|---|---|---|
| Spec basis | Documented | — |
| Candidate comparison table (K1/K2/P1) | Documentary in this ADR | Feeds A/B |
| Limited evaluation spike (Windows) | **Complete for Gate A** — store/read/delete/missing PASS for K2+P1; K2 error-redaction **FAIL** (BadEncoding Debug); P1 error-redaction PASS; `evidence/m1/secure-store-spike-windows-20261005.md` | A |
| Limited evaluation spike (macOS) | **Complete for Gate A** — same matrix pattern at SHA `4f4ba2784cbb3d305a8a462aa9b7286b9a8e770d` on macOS 27.0.1 (26A434) arm64; Keychain prompt not observed during that run; `evidence/m1/secure-store-spike-macos-20261006.md` | A |
| Selected approach | **P1 accepted** — `docs/adr/ADR-0014-os-secure-store-decision-addendum.md`. Product pins recorded below. K2 unselected. Gate C product verification recorded on both OSes. | B done; C done (product adapters) |
| Windows physical product verification | **VERIFIED** — evidence/m1/gate-c-product-secure-store-windows-20261006.md (product WindowsCredentialStore) | C (M1 completion) |
| macOS physical product verification | **VERIFIED** — evidence/m1/gate-c-product-secure-store-macos-20261006.md (product MacosKeychainStore at SHA `610733f8f911c67f341694c4333e34070b706d50`) | C (M1 completion) |

Gate A evaluation is recorded on both first-class platforms. K2 satisfies store/read/delete/missing-item on both hosts but fails overall error-redaction because `keyring_core::Error::BadEncoding` Debug recovers secret bytes; missing-item (`NoEntry`) surfaces are safe. P1 typed errors pass redaction checks on both hosts. That is Gate A evidence only. It does **not** accept this ADR, does **not** verify product `cryptexd` integration, and does **not** claim Gate C product verification.

## 7. Recovery / security implications

- Offline Recovery Kit signing secret remains out of normal device runtime; secure-store holds ordinary device credentials only.
- No custom crypto wrappers around OS stores.
- Same-user compromise can expose stored secrets; Cryptex does not claim otherwise.
- Logging redaction remains mandatory.

## 8. Criteria checklist

### 8.1 Before ADR → ACCEPTED (gate B)

1. Owner review of this revised proposal.
2. Limited spike evidence comparing K2 and P1 at minimum (K1 optional), with limitations recorded.
3. Selected approach named in this ADR or a dated addendum, including exact pins only when evidence supports them. **Done:** P1 accepted 2026-10-06 with product pins below.
4. Explicit confirmation that hardware-backed non-exportable keys are not required.
5. Unfinished platform spike cells remain labeled WAITING — never silently marked verified.

### 8.2 Before M1 secure-store completion (gate C)

1. ADR-0014 accepted (gate B) and product adapters implemented under `cryptexd` authority.
2. Windows physical matrix: store / read / delete / missing-item / redacted errors.
3. macOS physical **product** matrix: same as Windows (Gate A spike complete; product Gate C VERIFIED 2026-10-06).
4. Tests show secret bytes absent from log fixtures.

## 9. Acceptance status

| Field | Value |
|---|---|
| ADR-0014 | **ACCEPTED** (2026-10-06) |
| Selected candidate | **P1** (direct platform APIs); **K2 unselected**; high-level `keyring` unselected |
| Decision addendum | **ACCEPTED** — `docs/adr/ADR-0014-os-secure-store-decision-addendum.md` |
| Product secure-store implementation | **Authorized** under `cryptexd` only (trait/adapters/wiring per M1 plan) |
| Product pins (evidence-backed) | Windows: `windows-sys = 0.61.2`, `widestring = 1.2.1`; macOS: `security-framework = 3.7.0` |
| Limited evaluation spike | **Complete** (Gate A Windows + macOS); product-separated spike retained for history |
| Hardware-backed keys required | **No** |
| Keychain prompts | Run-specific caveat preserved: macOS Gate A and Gate C product prompts were **not observed during those short runs** — not proof prompts never occur |
| Both-OS physical product verification | **Complete for Gate C product adapters** — Windows VERIFIED 2026-10-06; macOS VERIFIED 2026-10-06 at SHA `610733f8f911c67f341694c4333e34070b706d50` |

## 10. References

- `docs/08_SECURITY_DEVICE_TRUST_AND_RECOVERY_ROOT.md` §§4, 21
- `docs/15_PLATFORM_MACOS_WINDOWS.md` §§8, 17
- `docs/milestones/M1_BRAIN_DEVICE_IDENTITY.md`
- `docs/adr/ADR-0007-local-ipc-implementation-addendum.md`
- `evidence/m1/m1-kickoff-plan-20261005.md`
- `docs/adr/ADR-0014-os-secure-store-decision-addendum.md`
- `evidence/m1/secure-store-spike-windows-20261005.md`
- `evidence/m1/secure-store-spike-macos-20261006.md`
