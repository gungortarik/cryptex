# M7 P1 macOS status-only health IPC — owner-reported result

**Date:** 2026-10-18  
**Product source worktree:** `/tmp/cryptex-m7-verify-20261018` at
`74bc3b14127962579ae03838bafea9336612a033`  
**Scope:** synthetic-vault status-only health IPC; not a write-back or
containment result.

## Result

The installed observation-only plugin `main.js` matched the verified source
SHA-256:

`0F1C024F48120D955C2EDD6A0415F0B5E6D3DD4BEEB49D17217A82384F081F24`

In the synthetic vault
`…/m7-p1-obsidian-038a331d135f423da4fe8a7473fea2f8/synthetic-vault`, the
existing `M7 Spike: local Cryptex health IPC` command returned:

```json
{"ok":true,"operation":"health.pong","daemon_pid":12905}
```

At the time of the probe the pilot runtime was
`~/Library/Application Support/Cryptex-Dev/m7-p1-health-ipc-20261018T164432Z/runtime`,
Desktop PID `12883`, daemon PID `12905`, and lifecycle state `instance=held`,
`health=active`.

After `Discard and Quit`, the owner reports `clean_quit`,
`instance=stale`, `note=clean shutdown released instance lock`, no held socket,
and no remaining `cryptex-desktop` or `cryptexd` process.

## Exact-source rerun

The owner then rebuilt and reran the probe from the exact clean source worktree
at `74bc3b14127962579ae03838bafea9336612a033` using a fresh Cargo target:

- Build command: `CARGO_TARGET_DIR=/tmp/cryptex-m7-verify-target-20261018 pnpm --filter @cryptex/desktop tauri:package --bundles app`
- App: `/tmp/cryptex-m7-verify-target-20261018/release/bundle/macos/Cryptex.app`
- Desktop SHA-256: `5c304b647ec2952c833aef121d3cce2618d8618ce3217cfd70571a78b1f11215`
- `cryptexd` SHA-256: `1181f00e51e41794e3486f78e07bc9c5249f956e8f4046b9ed3e457457015c6a`
- Observation plugin SHA-256: `0F1C024F48120D955C2EDD6A0415F0B5E6D3DD4BEEB49D17217A82384F081F24` (match)
- Runtime: `~/Library/Application Support/Cryptex-Dev/m7-p1-health-ipc-built-20261018T165149Z/runtime`
- Desktop PID `19035`; daemon PID `19061`
- Response: `{"ok":true,"operation":"health.pong","daemon_pid":19061}`
- Shutdown: `clean_quit`, `instance=stale`, lock released, no held socket, and no remaining Cryptex process.

This closes only the exact-source, status-only health IPC check on the reported
Mac host. It does not prove Obsidian peer authorization, proposal authorization,
real-daemon pending-request cleanup, or physical containment. Managed-note I/O
remains disabled; M7 is not complete. No source changes, Actions, commit, push,
or merge were reported.

## Initial-run limitation (superseded by the rerun above)

The app binary's mtime was 2026-10-14 19:38, before commit `74bc3b1` on
2026-10-14 21:32. The source worktree was verified at `74bc3b1`, but the
running binary was not rebuilt from that source. Treat this as evidence that
the installed observation plugin exchanged a status-only response with an
isolated Cryptex daemon, not as exact-source verification for `74bc3b1`.

The exact-source rerun above supersedes this limitation. The original run's
result remains recorded for provenance.
