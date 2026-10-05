# Mac managed Syncthing — scoped PASS

## Provenance

The project owner supplied the Mac executor's result. This Windows executor did
not open the Mac-local artifact or independently recompute its digest.
Tested source: `501d11d64cb7d77e8b1c51ad453fdf15f326fc8d`.
Platform: macOS **27.0.1 arm64**, Node **v24.19.0**.
Syncthing **2.1.5**, `go1.27.1 darwin-arm64`.
Native input: `/Applications/Syncthing.app/Contents/Resources/syncthing/syncthing`.
Binary SHA-256:
`6de41132c8a3fc78fc57486df32f9ac69d6f53db1e1910bb46f548ee7f29725e`.

## Checks and actual proof

- Managed tests **22/22**, no failure/skip, exit **0**.
- Five syntax checks and diff check PASS; notice index/working-tree bytes LF.
- Live runner exit **0**; distinct isolated identities/homes and loopback verified.
- Incorrect API keys rejected **403/403**; pairing, pause/resume and idle verified.
- Native version history and new transfer after restart verified.
- Conflict preserved both exact divergent contents.
- Staged native binary/LICENSE/AUTHORS hashes and POSIX modes verified.
- Observed global config digest unchanged.

| Monitor PID | Clean shutdown milliseconds | Exit |
| --- | ---: | ---: |
| 83988 | 36 | 0 |
| 84004 (immediate restart) | 10028 | 0 |
| 84014 | 791 | 0 |
| 83984 | 155 | 0 |

All four PIDs absent after shutdown. No forced cleanup or native database-stop
failure. Earlier native tests were not repeated. No source edits, commit/push,
Actions or startup changes; Mac repository reported clean.

Retained report:
`/Users/tarikgungor/Library/Application Support/Cryptex-Dev/p0-4/mac-managed-proof-20261005-67A14240-E34E-4505-8D2F-98280AF3A392/managed-syncthing-report.json`.
Reported SHA-256:
`6a89daf4bd6307c0a089f084990c2878eacdd79fdc99038564e292bb22d332cf`.

## Acceptance scope

This closes the managed Mac parity task, including the notice-byte correction
and target-matched staged-package lifecycle. The original 7 PASS / 8 FAIL result
at `502abf4` remains in `notice-byte-portability-failure-and-fix-20261005.md`.
Two local identities do not establish cross-host trust or synchronization,
changed-version upgrade, installed distribution, publisher authentication or
production secret storage. ADR-0004 remains PROPOSED; P0-4/M0 acceptance requires
the remaining review and compatibility evidence.
