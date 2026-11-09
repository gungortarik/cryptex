# Native coordination candidate packages — 2026-11-08

## Source binding and scope

Candidate: `5708cb7bad7f3bc35d43df2568414de2267a18a4`.
Shared source ZIP SHA-256:
`e7492ce071a38fbdf2453b4d959cbf9b30f13d2316fc97ae98beb84a2c8ba2e1`.
Daemon implementation is unchanged from the reviewed `141ff83` checkpoint.
Only test runners, instructions and runner regressions changed.

Owner Downloads artifacts:

| Package | SHA-256 |
| --- | --- |
| `cryptex-win-coordination-5708cb7.zip` | `BE186CC6467D588DB8845C7820C89664FF383BD4D5B8179BF9EA69693974154B` |
| `cryptex-mac-coordination-5708cb7.zip` | `285C1C327BE5F4649FBE9EFE4F73A10F20EE15DB4777A777C6C6A379D56EAFE6` |

The Windows package selects instance 9, runtime admission 5, publication 14,
managed incoming controller 4, IPC 66 and owned HTTP 8: **106 total**. Two
additional TCP probe checks produce summary/parser logs. Exact native owned
HTTP results are required here; the development-host exclusion is not applied.

Mac selects instance 8, runtime admission 5, staging 58, incoming validation 9,
trusted import probe 3 and native Keychain 2: **85 expected**, plus strict
all-target daemon Clippy. Windows-only production publication and managed
incoming controller remain unavailable on Mac; successful compilation of shared
code is not proof of that missing parity.

Both runners demand an exact single success summary/count, zero ignored/failed
entries and successful process exit for every test group. Empty, duplicate or
wrong-count summaries cannot pass. Windows reports work and test-temp filesystem
types; Mac reports the temporary work volume's type, or explicitly unknown.
Neither reports volume names/UUIDs or infers durability from filesystem type.

## Author validation

- Windows PowerShell 5.1 `test-platform-runner.ps1`: passed. Exercises the actual
  runner helpers with valid, missing, duplicate, zero-test, wrong-count, ignored,
  failed and nonzero-exit results; real subprocess stdout/stderr and launch
  failure capture; filesystem observation.
- Git Bash `test-macos-runner.sh`: passed. Extracts only the actual Mac summary
  helper and uses a fake Cargo function to exercise successful and all six
  failure conditions. It does not execute native macOS commands.
- Bash syntax check for `MAC-TEST.command` and Git whitespace checks: passed.
- Candidate Windows Rust test inventory: all six filter counts above match.
  Listing tests is not running those 106 tests.
- Both existing packagers verified exact output members against the pinned Git
  blobs/source archive. The Mac package verifies executable command-file mode.
- No production profile, native secure store or VM/Mac was accessed by these
  local checks. No security settings changed. No production code changed, so
  the prior daemon-suite result is retained rather than presented as rerun.

## Owner run and pending evidence

Extract Windows ZIP in the existing VM, then open `WIN-TEST.cmd`. Extract Mac
ZIP on the Mac, then open `MAC-TEST.command`. Both use installed Rust 1.98.1 and
fresh isolated build/test directories. Initial locked dependency acquisition
may need Internet; selected tests subsequently run offline. No Git/Node needed.
Mac Keychain tests use synthetic development entries and may show an OS prompt.

Return `WIN-TEST-SONUC.zip` and `MAC-TEST-SONUC.zip` even on failure. Bind returned
reports to this exact commit/archive; inspect actual build/test logs and every
exit code rather than trusting the aggregate outcome alone. Receipts are
**pending**, not passed. Do not ask the owner to redo pairing.

This is a native execution checkpoint for isolated signed fixtures and shared
coordination. Controller transport observations are still synthetic; no real
cross-device feeder, endpoint-to-publication flow or power-cut experiment is
performed by these packages. Normal publication activation, Mac production
parity, filesystem durability evidence, remaining cross-device integration,
host TCP compatibility, required CI and human review remain open. M8 remains
incomplete.
