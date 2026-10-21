# M5 Slice 3 -- Windows native acknowledgement/archive/Quit PASS

**Date:** 2026-10-11
**Product SHA:** `a172d4bc45a2bd041a4e28ba6342a9ab9e62dd44`
**Package source HEAD:** `65b22417e64378dab0df10213523177888ea5225` (product tree identical; later documentation commits only)
**Guest:** Windows 11 x64 VMware guest
**Package ZIP SHA-256:** `793430e6dae75b66cc72560ad246c5bae5f3e98171f4d11992ecd46398fb7336`

## Isolated pilot

- ZIP hash verified in the guest before extraction.
- Extracted and launched the packaged Desktop with a fresh marker-only pilot runtime; no personal/Cortex memory was used.
- Runtime: `%LOCALAPPDATA%\Cryptex-Dev\m5-s3-ack-a172d4b-8e4a22fd4fa249c89c27648b8811193e\runtime`
- Desktop PID at launch: `4660`.
- Synthetic pending capture A ID: `1283e391-33d4-49df-9f3b-9c2d73d2f116` (`public`).

## Native results (owner-observed)

| Check | Result |
|---|---|
| Pending A → synthetic draft B → duplication acknowledgement Cancel | PASS; owner reports expected preservation of pending A and draft B. |
| Repeat Save → deliberate-new confirmation | PASS; owner reports expected result. |
| Earlier unresolved archive row | PASS; screenshot shows A (`1283e391-33d4-49df-9f3b-9c2d73d2f116`, `public`) under “Earlier unresolved capture” with **Resolve**. B remains in the draft editor; rejected-save notices are visible. |
| Quit → No | PASS; owner reports the app remained open and the archive/draft were preserved. |
| Quit → Yes | PASS; subsequent `Get-Process cryptex* -ErrorAction SilentlyContinue | Select-Object Id, ProcessName` returned no processes. |

The final process check is guest-reported. No `lifecycle.status` or supervisor snapshot was captured in this report, so this records the UI and process-exit result without claiming a separately inspected lock-release record.

## Scope

This closes the Windows native acknowledgement/archive/Quit check for product `a172d4b`. It does not verify archive behavior across close-to-tray/reopen, power loss, or untested OS shutdown paths. macOS archive/Quit native result is separately recorded in `m5-slice3-ack-dialog-macos-native-pass-20261011.md`.

No production code changed, no Actions or merge were run, Slice 4 remains unauthorized, and M5 is not complete.
