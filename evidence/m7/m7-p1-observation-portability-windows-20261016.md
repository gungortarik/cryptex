# M7 P1 observation-only source and Windows runtime inventory

**Date:** 2026-10-16
**Scope:** prerequisite diagnostics only; managed-note IO remains disabled
**Repository product base:** `74bc3b14127962579ae03838bafea9336612a033`
**Diagnostic source:** local/uncommitted, `scripts/spikes/m7-obsidian-observation/`
**Status:** Windows scoped PASS; corrected owner-reported macOS local fixtures
18/18 PASS; containment and the M7 completion gate remain open

## Changes

The tested local observation-only spike is now reproducible from repository
diagnostic source. This is not a production plugin package and implements no
proposal lifecycle or new IPC capability. It adds a bounded runtime inventory
using the documented Obsidian `apiVersion` export and Node runtime metadata.
No TypeScript API package is used by this plain CommonJS probe.

A single endpoint adapter mirrors the existing daemon paths: Windows
`\\.\pipe\cryptex-cryptexd-v1`, macOS `/tmp/cpx-cryptex-<uid>/s`. The plugin
does not create endpoints or start a daemon. Endpoint selection on Windows
fixtures is not native macOS verification.

The probe refuses activation outside an explicitly opted-in `Cryptex-Dev`
vault-path component. This is a synthetic-scope restriction, not physical
containment proof. Managed-note reads/writes remain absent. The three existing
observation/status commands are the only registered commands; bounded plugin
metadata evidence writes remain enabled. Only a synthetic vault with the
default `.obsidian` config directory is in this probe's scope.

## Windows checks

Commands from the repository root:

```sh
node --check scripts/spikes/m7-obsidian-observation/main.js
node --test scripts/spikes/m7-obsidian-observation/guard.test.cjs scripts/spikes/m7-obsidian-observation/real-socket.test.cjs
```

Both commands exited 0 after correcting response cleanup: **18/18**, no
failures, cancellations, skips, or todos.
These comprise 15 VM checks and 3 actual local named-pipe fixture checks. The
four added VM checks cover bounded runtime inventory, refusing a non-synthetic
vault, selecting the existing Mac UID endpoint, and refusing a missing Mac UID
before connecting. The actual-transport tests use unique fixture endpoints;
they are not `cryptexd` peer/auth or real-daemon pending-operation evidence.

Manifest parsing, LF-file checks, and `git diff --check` passed.

## Native Windows observation

Only the existing synthetic vault's plugin files were updated, after preserving
the previous source and manifest under:

`%LOCALAPPDATA%\Cryptex-Dev\m7-writeback-spike-27551853ab304734b2bc7f0e74b0a525\proof\portable-observation-20261016-bd3fbb69edf9`

The actual Obsidian command palette reloaded the app. Its accessibility tree
then showed `M7 spike: observation only; note IO disabled` and 362 note
characters. The newly written runtime inventory reported:

| Field | Observed value |
|---|---|
| Visible Obsidian app | `1.14.4` |
| Windows executable file version | `1.14.4` |
| Obsidian runtime API | `1.14.4` |
| Platform / architecture | `win32` / `x64` |
| Embedded Electron | `43.7.7` |
| Embedded Node | `24.21.0` |
| API-types package | `none: plain CommonJS runtime probe` |
| Temporary manifest loader minimum | `1.5.0`, not a selected product minimum |
| Product baseline selected | `false` |

The installed `main.js` matched the tested repository source. Managed-note
SHA-256 remained
`e8606fb2943b72494432c3c29dbaa4764e70edbfaccaf4ad5a6ffbcad2b3dc12`;
the sentinel remained
`c0cd8b0f544e3fc65638674df720b974b1615446dadbdbd286535f6cdb0316d5`.
No managed-note mutation or health request was performed for this native
inventory check. Obsidian was left in the synthetic vault with the observation
plugin enabled.

## Mac transfer — corrected socket cleanup

The self-contained transfer includes source, manifest, README, both test files,
an English `MAC_CURSOR_PROMPT.md`, and `SHA256SUMS.txt`:

- Artifact (first socket-fix package; its checksum manifest used CRLF): `C:\Users\Tarik\Downloads\Cryptex-M7-Observation-P1-socketfix-20261016-174044.zip`
- ZIP SHA-256: `0d292c0b0d946d51feff525c846ce5b66ffdb72851edb4eb75eabf05cca7597e`
- `main.js` SHA-256: `0f1c024f48120d955c2edd6a0415f0b5e6d3dd4beeb49d17217a82384f081f24`
- Manifest SHA-256: `89a514688e69dfcd7824644a439d27676ee563456c43e20ebf03e47418c2fd26`

Archive integrity and every archived source byte were checked locally. The
package is not claimed to be on origin. The Mac prompt uses the exact existing
owner-identified synthetic root, preserves dirty checkouts, verifies transfer
hashes, and keeps note IO off. It requests actual runtime/startup/lifecycle
observations without treating a no-event unload interval as proof of listener
silence. The first Mac run at the preceding source reported **16/18** local
fixture checks. The corrected package now passes **18/18** local fixtures on Mac.
On the first package, native `shasum -c` rejected the CRLF manifest, though the
owner normalized it and verified 6/6 source files. A packaging-only replacement
ZIP carries the same Mac-verified source with LF-only manifest lines:

- Artifact: `C:\Users\Tarik\Downloads\Cryptex-M7-Observation-P1-socketfix-lf-20261016-175009.zip`
- ZIP SHA-256: `1cbbb4478822c1c4f670d16dc297ab931dad8bd3493e81fd8aa622a5c81f98bd`

Local archive integrity, LF-only manifest, and all 6/6 payload hashes pass.
This packaging-only replacement does not require repeating the Mac plugin run.
These fixtures do not prove native daemon health or cleanup.

No product source, dependency pin, architecture, Actions, commit, push, or merge
was changed. The owner-selected containment decision remains in force.

API reference: [official Obsidian declarations](https://github.com/obsidianmd/obsidian-api/blob/master/obsidian.d.ts)
document `apiVersion` as the API version of the running app. The current daemon
endpoint definitions were read from `services/cryptexd/src/ipc.rs`.
