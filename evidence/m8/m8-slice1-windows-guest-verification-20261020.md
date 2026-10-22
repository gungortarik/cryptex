# M8 status diagnostic — Windows guest verification receipt

**Date:** 2026-10-20  
**Status:** Owner-observed runner exit 0; detailed guest artifacts not imported.

The owner manually ran `verify-m8-windows.cmd` from
`Cryptex-M8-Windows-Setup-20261019-43b6ee3d.zip` inside the Windows VM.
The initial run exited 2 because Cargo was unavailable. After Rust setup,
the run completed with `Exit code: 0`.

Screenshot: `codex-clipboard-cdc65ba4-fcde-4c5a-8ed4-70a015905b8c.png`.
Guest report directory:
`C:\Users\Tarik\AppData\Local\Cryptex-Dev\m8-verification\setup-27583-27640`.

The packaged runner gates zero exit on pinned-toolchain discovery, format,
focused diagnostic library/CLI tests, strict daemon Clippy, build and the
missing-runtime status smoke. This records its aggregate result, not
independently inspected command logs, binary hashes or new test counts.
`verification.log`, `status.json` and the binary hash remain in the guest.

The missing-Cargo guest blocker is resolved for this package. The host's
Application Control restriction remains separate; no guest CLI is established.
This candidate is the read-only diagnostic, not pairing, transport supervision,
incoming admission or full M8 completion. No Actions were run.
