# M2 Security + Definition of Done Review (pre-final Actions)

**Date:** 2026-10-07
**Branch:** `m2/immutable-event-store`
**Reviewed tip (pre-fix):** `7fe206730f11d319ed9e690be7fa1f5d27a2fe7c`
**Fix tip:** *(filled after commit)*

## 1. Reviews performed

| Review | Agent / method | Result |
|---|---|---|
| Security Review | Cursor `security-review` subagent | **No Critical/High** in targeted trust-boundary pass (partial branch diff limitation noted) |
| Security Auditor | Cryptex `security-auditor` subagent | **F1 High**, **F2 Medium**, **F3 Medium** in-scope; F4–F6 Low deferred |
| DoD / Verifier | Cursor `verifier` subagent | **M2 NOT complete** — final dual-OS Actions + owner review still open |

## 2. In-scope findings and disposition

### F1 — High — Signing-key mint over existing packs (FIXED)

**Issue:** `ensure_event_signing_key(..., &[])` treated empty observed as green-field even when `.open`/`.seg` existed, minting a new key then verifying/quarantining valid history.

**Fix:**
- `ensure_event_signing_key` takes `canonical_packs_present: bool`
- `events_exist = !observed.is_empty() || canonical_packs_present`
- Incomplete seed/metadata + packs present + empty observed → `FailClosed` (no mint)
- `append_memory_note` / `load_verified_events` discover packs before ensure

**Tests:**
- `event_signing_key::fail_closed_when_packs_present_without_keys_and_empty_observed`
- `canonical_writer::fail_closed_when_packs_exist_but_signing_keys_missing_does_not_mint`

### F2 — Medium — `load_verified_events` sealed prefix accept (FIXED)

**Issue:** Sealed packs could return a verified prefix without requiring `verified_len == file_len` / quarantine.

**Fix:** Same full-consumption check + `quarantine_sealed_pack` as writer `rebuild_chain_state`.

**Test:** `load_verified_events_fail_closes_and_quarantines_corrupt_sealed_pack`

### F3 — Medium — `source_relative` API trust (FIXED)

**Issue:** Quarantine API stored caller `source_relative` without validation (absolute host paths possible).

**Fix:** `validate_source_relative` — allow only `unresolved` or `canonical/events/<stem>/<digits>.{open,seg}`.

**Test:** `source_relative_rejects_absolute_and_parent_paths`

### Deferred (not fix-now)

| ID | Severity | Reason |
|---|---|---|
| Security-review L1 | Low | Corrupt sealed `.seg` remains in canonical tree after quarantine copy — ADR fail-closed; operator repair deferred |
| Security-review L2 | Low | Desktop links `cryptexd` crate — defense-in-depth; no writer IPC today |
| Auditor F4 | Low | Absolute paths in some typed error strings |
| Auditor F5 | Low | Optional quarantine-root containment re-assert |
| Auditor F6 | Low | Staging orphans / canonical (non-quarantine) `\\?\` out of addendum scope |
| DoD | Gate | Final dual-OS GitHub Actions **not run** (reserved for this post-review tip) |
| DoD | Gap | `LongPathsEnabled=0` still **not claimed** |

## 3. Extended-path regression tighten (slice 8h) — already on branch

Required >MAX_PATH existence/read/cleanup uses prefixed adapter paths only; public `QuarantineEntry.directory` stays logical. Evidence: `m2-slice8h-extended-path-regression-tighten-20261007.md`.

## 4. Local verification after fixes (Windows)

```
cargo test --locked -p cryptex-storage quarantine
→ ok. 21 passed

cargo test --locked -p cryptexd --lib
→ ok. 126 passed

cargo fmt --all -- --check
→ PASS

cargo clippy --locked -p cryptex-storage -p cryptexd --all-targets -- -D warnings
→ PASS
```

## 5. Completion status

- **M2 complete:** NO
- **Ready for single final dual-OS Actions gate:** YES from security/DoD review disposition (Critical/High in-scope items fixed; Actions still required)
- **Owner review:** still required after Actions
