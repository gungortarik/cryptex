# Windows guest development setup handoff

**Date:** 2026-10-19  
**Scope:** One-time Windows development-environment verification, not the
completed M8 Windows stage.

The existing Win11 VMware guest is running and its desktop was observed.
Host Application Control continues to deny dependency build-script execution
(4551). VMware guest CLI operations require the encrypted VM password.
No credentials were requested or guessed and no security policy was changed.

A source-only ZIP was prepared from baseline
`74bc3b14127962579ae03838bafea9336612a033` plus the current M8 status-diagnostic
candidate and documentation:

- File: `C:/Users/Tarik/Downloads/Cryptex-M8-Windows-Setup-20261019-43b6ee3d.zip`
- SHA-256: `040318e2de6585a0a3a62e2f53c63d205b28a7c238bf9fc93427324ebfd95209`
- Size: 1,661,169 bytes
- ZIP integrity check: PASS; runner has Windows CRLF and source checksums.
- Entrypoint: owner-started `verify-m8-windows.cmd` inside the intended guest.

The fixed runner checks VMware guest identity, installed Rust 1.98.1, formatting,
the current 12 library/3 CLI tests, daemon Clippy/build and read-only missing
runtime smoke. Logs and build output stay under LocalAppData outside source.
It does not install toolchains, change policies, launch a daemon lifecycle,
create keys, pair devices, run Actions, or modify Obsidian/Syncthing profiles.

The runner has not executed in the guest. No Windows verification PASS or full
M8 completion is claimed. The computer-use skill forbids terminal automation;
the single owner-started runner is required unless an authenticated supported
guest CLI becomes available. This setup does not introduce per-slice Mac
handoffs; full Windows implementation/verification remains the active stage.
