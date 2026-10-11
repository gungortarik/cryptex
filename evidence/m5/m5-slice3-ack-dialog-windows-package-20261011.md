# M5 Slice 3 -- Windows ack-dialog package for product a172d4b

**Date:** 2026-10-11  
**Branch:** `codex/m5-capture-timeline`  
**Source HEAD:** `65b22417e64378dab0df10213523177888ea5225`  
**Product SHA:** `a172d4bc45a2bd041a4e28ba6342a9ab9e62dd44` (ancestor of HEAD; product tree identical — docs-only commits after product)  
**Host (package build):** ASUSROG, Windows 11 Pro 10.0.26300  
**Target guest:** `D:\VMs\Win11\Windows 11 x64.vmx`  
**Authorization:** Slice 3 verification/closeout only. No production code change. No Actions.

## Checkout

Tracked working tree clean at tip matching `origin/codex/m5-capture-timeline`.  
Untracked local scratch preserved (no reset/clean/stash). No sync required.  
`git merge-base --is-ancestor a172d4b HEAD` → success.

## Package (PASS)

Command: `pnpm run tauri:package-smoke` with  
`CARGO_TARGET_DIR=%LOCALAPPDATA%\Cryptex-Dev\m5-s3-ack-win\cargo-target`.

| Artifact | Bytes | SHA-256 |
|---|---:|---|
| `cryptex-desktop.exe` | 11096064 | `d1ebac57f5782ea67223c68014849959caf8c41ef51d0132b93527c8404534f8` |
| `cryptexd.exe` | 4573696 | `4493b210527e9dcfc0d6a317607d753b64cd0c68ac8a4c29325f4140e3216fd7` |
| zip | 5839957 | `793430e6dae75b66cc72560ad246c5bae5f3e98171f4d11992ecd46398fb7336` |

Stage: `%LOCALAPPDATA%\Cryptex-Dev\m5-s3-ack-win\Cryptex-M5-S3-Windows-Ack-a172d4b\` (+ `provenance.json`)  
Zip: `D:\VMs\Win11\Cryptex-M5-S3-Windows-Ack-a172d4b.zip`  
Host-only transfer (listening):  
`http://192.168.177.1:8766/m5-s3-ack-d2e79e7d37dd8edd649d176565894376/Cryptex-M5-S3-Windows-Ack-a172d4b.zip`  
Serve script: `D:\VMs\Win11\serve-m5-s3-ack-zip.ps1` (guest peer `192.168.177.131` only).

Unsigned. No networking/firewall/host security settings changed.

## Interactive verification — BLOCKED

`vmrun list` → **Total running VMs: 0**.  
Per prior instruction, `vmrun start` was not retried and the encryption password was not requested.

No Desktop/daemon launch, no pilot PIDs, no ack/archive/Quit observations on Windows for `a172d4b`.  
Mac native PASS for this product remains at `evidence/m5/m5-slice3-ack-dialog-macos-native-pass-20261011.md`.  
Automated capture-policy coverage is not native UX evidence.

### Prepared host-side pilot stub (unused)

`%LOCALAPPDATA%\Cryptex-Dev\m5-s3-ack-0abdeff7a7504dcd\runtime`  
contains only `pilot-runtime.marker` = `m0-native-desktop-runtime-v1\n`. Not launched.

## Owner steps after guest unlock

1. Unlock/start the Windows 11 VMware guest (owner).  
2. In guest, download the zip URL above; verify SHA-256 `793430e6…`.  
3. Extract under `%LOCALAPPDATA%\Cryptex-Dev\m5-s3-ack-a172d4b\pkg`.  
4. Fresh pilot (marker only):

```powershell
$pilot = "$env:LOCALAPPDATA\Cryptex-Dev\m5-s3-ack-$(Get-Date -Format yyyyMMddTHHmmss)\runtime"
New-Item -ItemType Directory -Force $pilot | Out-Null
[IO.File]::WriteAllBytes("$pilot\pilot-runtime.marker", [Text.Encoding]::UTF8.GetBytes("m0-native-desktop-runtime-v1`n"))
$pkg = "$env:LOCALAPPDATA\Cryptex-Dev\m5-s3-ack-a172d4b\pkg"
& "$pkg\cryptex-desktop.exe" --cryptex-pilot-runtime $pilot
```

5. Supported interactive sequence (no personal/Cortex data; no fabricated state):  
   - Create pending **Uncertain** capture A (synthetic public text) via a real transport/uncertain outcome — same supported path used in the Mac PASS; do not add fault hooks.  
   - Draft B (new text), classification public, **Save**.  
   - Expect Cryptex-owned dialog **“Start another capture?”** with Cancel / Start new capture.  
   - **Cancel** → draft B and pending A remain.  
   - **Save** again → **Start new capture** → A under **Earlier unresolved captures** with **Resolve**.  
   - With archive row present: Quit → **No** → both processes alive; draft/archive retained.  
   - Quit → **Yes** → clean shutdown; no orphans; record `clean_quit_*` / lock release.  
6. Stop at the first failure; record button labels, PIDs, and notes.

## Remaining gates

- Windows native ack/archive/Quit at product `a172d4b` (this package) — OPEN (guest offline).  
- Optional close-to-tray/reopen with archive state — OPEN.  
- No Actions, merge, Slice 4, or M5-complete.
