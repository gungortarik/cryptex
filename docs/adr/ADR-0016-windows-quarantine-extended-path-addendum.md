# ADR-0016 — Windows quarantine extended-path addendum

**Status:** ACCEPTED

**Date drafted:** 2026-10-07

**Date accepted:** 2026-10-07 (owner approval of the Windows `\\?\` quarantine filesystem adapter approach)

**Parent:** `docs/adr/ADR-0016-immutable-event-store-productization.md` (**ACCEPTED**)

**Milestone:** M2 — Immutable Event Store

**Authority:** Parent ADR §8.1 layout and quarantine safety; ADR-0015 DeviceId path-safe policy; `docs/15_PLATFORM_MACOS_WINDOWS.md`; owner decision to select extended-length path handling inside the quarantine filesystem adapter without changing §8.1 directory names or DeviceId policy.

## 1. Decision

**Accepted:** On Windows, the **quarantine filesystem adapter** (implementation in `crates/storage` quarantine module) MUST perform create/open/rename/remove/sync path I/O using Win32 **extended-length** paths (`\\?\` / `\\?\UNC\...`) for absolute quarantine paths.

This decision:

- **Preserves** ADR-0016 §8.1 directory layout names: `{utc_compact}_{reason_code}_{entry_id_hex}` under `quarantine/canonical-events/<attribution_dir>/`
- **Preserves** ADR-0015 DeviceId validation and hex filename-stem policy (no DeviceId length/policy change)
- **Does not** require modifying the machine registry or relying on `HKLM\...\LongPathsEnabled`
- **Does not** change quarantine metadata rules (still no absolute host paths in `quarantine.json` / default logs)
- **Does not** authorize extended-path handling outside the quarantine filesystem adapter in this addendum

## 2. Why this option

Measured M2 evidence showed §8.1 object paths for UUID-length and max-length DeviceIds exceed classic Win32 MAX_PATH (260) under production-like LocalAppData roots, while short DeviceIds remain under budget. Owner selected extended-path I/O over:

- embedding `longPathAware` + requiring OS `LongPathsEnabled` as the sole portable fix
- unilaterally shortening §8.1 names or DeviceId policy

Extended-length prefixes allow the accepted layout to operate for supported DeviceId lengths without ambient OS policy changes.

## 3. Normative adapter rules

1. **Scope:** Only quarantine adapter filesystem operations (directory exclusive-create / no-replace, object placement, metadata write, staging cleanup, and sync of those quarantine paths).
2. **Logical vs I/O paths:** Public quarantine results continue to expose ordinary path values for the §8.1 layout. The `\\?\` form is an I/O adaptation, not a layout rename.
3. **Absolute paths only for prefixing:** Extended prefixes apply only after the adapter has an absolute path. Relative roots are resolved to absolute form before prefixing, or rejected with a typed quarantine/storage error.
4. **No path-escape bypass:** Before I/O, reject path components that are `..` or `.` (current/parent directory). Do not accept arbitrary caller path text as `reason_code` or attribution leaves; continue allow-listed `reason_code` tokens and ADR-0015-validated DeviceId stems.
5. **UNC:** UNC roots use `\\?\UNC\...` form.
6. **Idempotent prefixing:** Do not double-prefix paths that already begin with `\\?\`.
7. **Semantics unchanged:** Exclusive create / no-replace, typed errors, durable sync expectations, and fail-closed behavior from parent ADR §7.2 / §8 remain mandatory.
8. **Non-Windows:** No `\\?\` adaptation; existing POSIX path I/O remains.

## 4. Explicit non-goals

- Changing ADR-0016 §8.1 directory name format
- Changing DeviceId maximum length or path-safe alphabet
- Registry edits or CI/host policy changes for `LongPathsEnabled`
- Applying extended paths to non-quarantine writers (canonical `.open`/`.seg` layout) in this addendum
- Claiming verification on hosts with `LongPathsEnabled=0` without actual measured evidence

## 5. Acceptance consequences

1. M2 implementation may land the Windows quarantine extended-path adapter under this addendum.
2. Parent ADR §8.1 remains the layout authority; this addendum only selects Windows I/O adaptation.
3. Evidence for UUID-length and 64-character DeviceId quarantine create/read/cleanup belongs under `evidence/m2/`.

## 6. References

- Parent: `docs/adr/ADR-0016-immutable-event-store-productization.md` §8
- Prior measurement: `evidence/m2/m2-slice8f-windows-path-ops-and-device-binding-20261007.md`
