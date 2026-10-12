# M5 Slice 5 — Windows packaged native review

**Source HEAD:** `3e31da06e1cc8bb485352e13c6cca309eee8cf5e`  
**Branch:** `codex/m5-capture-timeline`  
**Host:** Windows 11 x64  
**Verdict:** package and isolated launch PASS; Timeline row/detail interaction BLOCKED by the fresh pilot's retrieval-safety hold. Slice 5 is not closed.

## Package

Built the Windows NSIS bundle from the source HEAD above:

```text
npm --prefix apps/desktop run tauri:package -- --bundles nsis
```

The frontend and Rust release build succeeded. Artifacts:

| Artifact | SHA-256 |
|---|---|
| `target/release/cryptex-desktop.exe` | `000A4044677C7209929EEA348C5961A599F6517819B6824FC1DEA1E66F5C918F` |
| `target/release/bundle/nsis/Cryptex_0.0.0_x64-setup.exe` | `5A53525C12F8D900387CE0BA427395CFF9F2C86CF29144F12A0610F5023CC3DB` |

The release executable was launched without installing the bundle, using a new marker-only runtime under:

```text
%LOCALAPPDATA%\Cryptex-Dev\m5-s5-win-e6be1c07e46040dcb8c78e1103e90a75\runtime
```

No personal/Cortex memory or repository runtime data was used.

## Native observations

- Inbox opened and accepted a synthetic note body and `public` classification.
- Save returned the safe `Capture was rejected. The draft was kept. The unresolved capture id was kept.` outcome. Pending ID: `5818af28-1689-44b8-b7eb-f42811d5db13`.
- Selecting the Timeline rail produced `RETRIEVAL PAUSED` / `Cryptex could not verify the complete local event history. Search is paused to protect your memory.` The Timeline row and full-note detail could not be exercised because this fresh marker-only pilot had no accepted canonical event to show. No event was fabricated or injected outside product paths.
- File → Quit displayed the native confirmation. **No** kept the Desktop open and preserved the synthetic draft and pending capture ID.
- A subsequent File → Quit → **Yes** exited Desktop and daemon. Runtime evidence: `state=clean_quit_from_app_menu`, `daemon_pid=none`, `supervisor_alive=false`, and `note=clean shutdown released instance lock`. `Get-Process cryptex*` returned no processes.

## Remaining verification

- Create/use a supported isolated fixture with an accepted canonical v1 note, then verify Timeline ordering, source metadata, paging, and full-note detail on Windows.
- Run the Slice 5 focused checks and native Timeline/detail review on macOS. Windows results do not establish Mac parity.
- No Actions, merge, Slice 4+ work, or M5-complete claim is included here.
