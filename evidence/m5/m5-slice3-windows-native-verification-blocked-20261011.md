# M5 Slice 3 -- Windows native verification attempt (blocked on encrypted VM)

**Date:** 2026-10-11
**Branch:** `codex/m5-capture-timeline`
**Source / docs HEAD:** `566a79838c6a53c6d704453a2252c45a83cbd7d5`
**Required product ancestor:** `b9b3e4cdfdc22077c5dd5ec78b0dbbf22969863f` (confirmed ancestor)
**Host performing preparation:** ASUSROG, Windows 11 Pro 10.0.26300 (physical host with VMware Workstation)
**Target guest:** `D:\VMs\Win11\Windows 11 x64.vmx` (`displayName=Windows 11 x64`, guestOS=windows11-64)
**Authorization:** Launch authorized for isolated verification; no Actions; scratch files preserved.

## Checkout status

Tracked working tree clean at tip `566a798` matching `origin/codex/m5-capture-timeline`.
Untracked local scratch (`tmp_*`, `_s3r_*`, etc.) present — **not** cleaned, reset, or stashed.
Therefore **no `git pull` / sync was performed** beyond confirming already-up-to-date tip.

## Native build preparation (PASS)

Pinned toolchain: Node v24.19.0, Rust 1.98.1, pnpm 11.28.0.
Command: `npm run tauri:package-smoke` with
`CARGO_TARGET_DIR=%LOCALAPPDATA%\Cryptex-Dev\m5-s3-win-native\cargo-target`.

| Artifact | Bytes | SHA-256 |
|---|---:|---|
| `cryptex-desktop.exe` | 11078144 | `1b7ff0b7445d9d8e2c58dfcca4bb5c44dea076a47a5bbbc061d70b96da62b1f0` |
| `cryptexd.exe` | 4573696 | `780c9bd4b00823f25f695059432c3be8bbd78573ff9cd80ff94509c59ae8ada7` |

Staged package (outside repo):
`%LOCALAPPDATA%\Cryptex-Dev\m5-s3-win-native\Cryptex-M5-S3-Windows-Native-b9b3e4cdfdc2\`
including `provenance.json`.

Transfer zip (prior M4 host-only HTTP method):
`D:\VMs\Win11\Cryptex-M5-S3-Windows-Native-b9b3e4cdfdc2.zip`
SHA-256 `69bd69d29d18196c48c25857ec75fcf12f6b37126a1388b9f8d59e68ba24e8c3` (5835615 bytes).
Unsigned. No networking/firewall/host security settings changed.

## VM access / launch — BLOCKED

```text
vmrun list → Total running VMs: 0
vmrun start "D:\VMs\Win11\Windows 11 x64.vmx" gui
Error: Cannot open VM: ... A password is required for this operation
```

VMX has `vmx.encryptionType = "partial"` / encryption keySafe. No guest shared-folder mount was available from the host (`\\vmware-host\Shared Folders` absent while powered off). Prior successful transfer pattern (M4) was host TCP serve on `192.168.177.1:8766` to guest `192.168.177.131` after the guest was already running.

**Exact missing step:** unlock/start the encrypted Windows 11 VMware guest (encryption password), then serve/fetch the staged zip and run the isolated pilot sequence inside the guest.

## Native Quit checks — NOT EXECUTED

No Desktop launch, no pilot runtime, no owned PIDs, no dialog observations.
No native PASS/FAIL claimed for File/tray Quit Cancel/Confirm, close-to-tray, or reopen.

Expected Windows button labels when re-run (from source `MessageBoxW` `MB_YESNO`, title `Cryptex`):
**Yes** (confirm / discard-and-quit path) and **No** (cancel). Message text begins
`Discard unsaved or unresolved capture input and quit?`.
(Mac uses Cancel / Discard and Quit; do not assume those labels on Windows.)

## Archive / unresolved UX — NOT VERIFIED natively

Safe native reproduction of `uncertain A → deliberate B → B committed → A remains tracked`
is **unavailable** in the current isolated pilot without fabricating renderer/daemon
uncertain/commit outcomes or adding production fault hooks.

### Mapped automated coverage (not a native substitute)

| Behavior | Automated evidence |
|---|---|
| Uncertain A then deliberate B archives A; B commit keeps A tracked + protected | `capture-policy.test.mjs` — `uncertain A then deliberate B then B committed keeps A tracked and protected` |
| Late A committed updates archive without clearing B draft | `late A committed updates stored archive without clearing B draft` |
| Resolve archived A without replacing pending B | `archived capture can be resolve-started without replacing pending B` |
| Archived state participates in Quit prompt policy | `shouldPromptBeforeNativeQuit` cases with `hasArchived: true` |
| Quit Cancel/Confirm/repeat/single-owner menu routing | Desktop `exit_lifecycle_routing_tests` (11) + `capture_quit_guard_tests` (5) at product `b9b3e4c` |

### Remaining native gap / narrowly scoped proposal

After VM unlock: use a **separate** synthetic pilot only if a reviewed, non-production
test path can create a real Uncertain capture (for example transport timeout against a
controlled local cryptexd that does not write personal data) without weakening auth or
shipping fault hooks. Until then: keep archive UX **NOT VERIFIED** interactively;
do not waive acceptance.

## Explicit non-claims

- No Windows native Quit PASS.
- No app launch on host as a substitute for the VMware guest.
- No Actions, merge, Slice 4, production v2 activation, or M5-complete.

## Resume checklist (owner unlocks VM)

1. Unlock/start `Windows 11 x64` in VMware Workstation.
2. On host, serve zip (reuse M4 pattern on `192.168.177.1`, new one-time route).
3. In guest: fetch zip, verify SHA-256 against provenance, extract outside repo under LocalAppData.
4. Fresh empty pilot with `pilot-runtime.marker` = `m0-native-desktop-runtime-v1\n` only.
5. Launch: `cryptex-desktop.exe --cryptex-pilot-runtime <pilot>`.
6. Announce before desktop control; perform Quit Cancel/Confirm matrix + close-to-tray.
7. Record button labels, PIDs, notes (`clean_quit_from_app_menu` / `clean_quit_from_tray`), lock release.

## Re-attempt 2026-10-11 ~15:00 UTC (same host)

Checkout re-inspected: tracked tree clean at 6aa984b7d024293b23d2c2c9204182bfa95b6b7f (docs tip after 566a798); product ancestor 9b3e4c still present; untracked scratch preserved; no sync/reset/stash.
Staged artifacts re-hashed; SHA-256 unchanged vs provenance.
Transfer zip present; host-only VMnet8 still 192.168.177.1; one-time serve script prepared at D:\VMs\Win11\serve-m5-s3-zip.ps1 (**not started** — guest offline).
mrun path corrected to C:\Program Files\VMware\VMware Workstation\vmrun.exe.
mrun list → Total running VMs: 0.
mrun start "D:\VMs\Win11\Windows 11 x64.vmx" gui → **Error: A password is required for this operation** (exit -1).
Native Quit pilots and archive UX still **NOT EXECUTED / NOT VERIFIED**.
