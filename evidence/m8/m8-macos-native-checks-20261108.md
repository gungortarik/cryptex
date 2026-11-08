# M8 Mac readiness receipt and native-check package — 2026-11-08

## Readiness receipt

Owner returned `MAC-SONUC.zip`, SHA-256
`07B009241BAFF87A542B39CBDC6768F30CEAAD6BD7C590498BF72A0DA646452A`.
The exact single member is `platform.txt`, 497 bytes. The bounded key/value
receipt was checked for duplicate/malformed fields and bound to
`scope=macos_readiness_only`, probe `m8-macos-readiness-20261108-v1`.
Collected at `2026-11-08T21:27:31Z`.

| Field | Reported observation |
| --- | --- |
| macOS / build | 27.2 / 26B123 |
| Process architecture / translated | arm64 / 0 |
| CLT / clang / rustup | Present |
| Pinned compiler | `rustc 1.98.1 (48a229cea 2026-09-01)` |
| Scratch copy comparison | Pass |
| Scratch hardlink count | 2 |
| Scratch symlink | Pass |
| Scratch filesystem type | Unknown |
| Native Cryptex / secure-store / power-loss tests | Not run |
| Publication enabled | False |

These are the supplied runner's observations, not remote attestation. Do not
infer APFS from architecture or a successful link operation. The receipt closes
readiness collection only; it does not establish native platform completion.

## Native runner scope

The owner has already requested remaining platform checks. The portable
`cryptex-mac-test-20261108.zip` contains exactly:

- `MAC-TEST.command`, Unix executable mode 0755, LF.
- `MAC-TEST-BASLA.txt`.
- `SOURCE-COMMIT.txt`, canonical full lowercase Git commit ID.
- `SOURCE-SHA256.txt`, SHA-256 of the included exact Git archive.
- `source.zip`, only the tracked files of that commit; no `.git` or local runtime.

The packager reads runner/instructions from the same committed revision and
rejects unsafe source ZIP paths, symlinks and source expansion beyond 128 MiB.
It verifies exact outgoing members, bytes and Unix modes after creation.
No Windows/Mac native binary or private key is supplied.

On a real Mac, the runner creates a new private temporary work directory, checks
source digest and existing toolchain, extracts source, and builds with pinned
Rust 1.98.1 and `--locked`. Initial Cargo dependencies may be downloaded; no
toolchain installation or update is performed. Build output goes to an explicit
temporary target directory, and canonical test runtimes remain outside source.
Only known logs and the bounded-format result metadata enter the returned ZIP.
The source/build tree is not exported in the result.

Selected test groups, serially and offline after build:

1. `instance::tests::` — actual Mac no-follow/single-link lock-file and lifecycle
   behavior, including the Unix symlink refusal regression.
2. `import_staging::tests::` — existing platform-applicable snapshot, alias,
   bounds, journal/recovery and subprocess interruption primitives.
3. `incoming_validation::tests::` — actual policy validation/discard and retry;
   no canonical publication activation.
4. `trusted_import_probe::tests::` — signed approved/unapproved/corrupted fixture
   policy with FakeSecureStore.
5. `secure_store::macos::tests::` — native Keychain put/get/delete/missing and
   redaction tests in synthetic dev accounts, not existing owner credentials.
6. `cargo clippy --all-targets -- -D warnings` on the Mac target; missing Clippy
   is reported as failed/unavailable rather than installed or silently skipped.

A zero-test selection or missing summary fails the group. Any group failure
remains in the overall result, but other selected groups still run. A compile
failure returns `outcome=build_failed` and build log. Keychain may display an
OS permission prompt; refusal must stay a diagnostic failure, not a reason to
disable protection. The EXIT handler packages success/failure as
`MAC-TEST-SONUC.zip` and reveals it in Finder. No `sudo`, profile inspection,
normal daemon startup or paired/native transport activation is involved.

## Verification and outstanding gates

Host checks: Bash syntax, refusal to execute on non-Mac, Python packager syntax,
archive member/mode/path/hash verification and diff checks. These were wrapper
checks only. At the original package handoff native build/test/Keychain results
were pending; the actual returned result is recorded below.

The persistent publication service is deliberately Windows-only and is not
ported by removing a conditional compilation flag. Mac managed endpoint
ownership/full replication, canonical publication parity, physical power-loss
durability and independent security review remain separate gates. Windows
owned-HTTP failures from the prior review remain open. No M8 completion claim.

Static parity limits also remain: the non-Windows import directory-chain helper
currently checks the final directory and returns no pinned ancestor handles;
the Unix no-replace helper uses hardlink then unlink, rather than the Windows
single move. These are not equivalent native security/durability guarantees.
Selected passing tests must not be used to authorize Mac production publication
without the missing platform design, race/interruption review and native proof.

## Evaluate the next result

Read only the expected bounded text/log members from `MAC-TEST-SONUC.zip`;
reject path traversal, aliases, unexpected members or excessive size before
inspection. Match source commit/digest and Mac architecture/build to the outgoing
package/receipt. Distinguish build, tests, Clippy and missing tools. Confirm every
selected group actually ran with nonzero test count. Do not treat a summary-only
success as independent review or infer transport/publication from these groups.

## Exact outgoing package binding

Built and verified after committing the runner:

- Source commit: `74c6c400d5ff9229625c0397dab5e3ef4f8344ab`.
- Embedded `source.zip` SHA-256:
  `6c03ac3629aec97c1904e8a216a53e21e5356ec8459fbc80b07539ad011ae771`.
- Outgoing `cryptex-mac-test-20261108.zip`: 2,266,096 bytes; SHA-256
  `02DE6B1D58A7209885C1E3EE3C346BD835B8614F168AC6CF56245EBF6B9AB110`.
- Outer five-member allowlist, contents and Unix modes verified by the packager;
  inner tracked-source paths/symlink refusal and expansion bound verified.
- Runner Bash syntax and non-Mac refusal passed. The exact pinned Cargo
  `--offline --locked --color never ... --no-run` argument shape was accepted
  by the host toolchain; that is not a Mac build result.

Expected return `MAC-TEST-SONUC.zip` contains `result.txt` and only created
members of `build.log`, `instance.log`, `staging.log`, `incoming.log`,
`trusted.log`, `keychain.log`, `clippy.log`. Metadata may be present even when
compilation fails and no test group ran. Actual Mac execution was pending at
this package checkpoint; the returned receipt follows.

## Actual native Mac receipt

The owner returned `MAC-TEST-SONUC.zip`, SHA-256
`FBCD68F17FBA6C4C510AF575EA38A33D240867D8B08C5473AE946CBBCF8C2F0E`.
The eight exact members and expanded lengths were bounded before reading:

| Member | Bytes |
| --- | ---: |
| result.txt | 1,335 |
| build.log | 9,886 |
| instance.log | 1,814 |
| staging.log | 6,929 |
| incoming.log | 2,083 |
| trusted.log | 1,471 |
| keychain.log | 1,343 |
| clippy.log | 4,935 |

Metadata matches the exact outgoing source commit/archive digest above,
`scope=macos_native_primitive_validation_and_keychain_tests` and
`probe_id=m8-macos-native-20261108-v1`. Collected timestamp is
`2026-11-08T21:44:20Z`; OS/build/architecture remain macOS 27.2/26B123/arm64.

| Native check | Observed result |
| --- | --- |
| Library test harness build | Passed; 23.33 seconds |
| Instance safety/lifecycle | 8 passed |
| Import staging/recovery | 58 passed |
| Incoming validation | 9 passed |
| Signed trusted/unapproved fixture policy | 3 passed |
| Native Keychain adapter | 2 passed |
| Strict all-target Clippy | Failed, exit 101 |

All 80 selected test entries passed with zero failed/ignored entries. Group
summaries agree with the actual log endings. The instance log includes both
the hardlinked immutable-file refusal and Unix symlink refusal. The Keychain
log names the native missing/delete/read/redaction matrix. Subprocess entries
are included in normal harness counts; this is not 80 distinct manual scenarios.

The overall `selected_native_checks_failed`, `failed_checks=1`, `runner_exit=2`
is correct and preserved. Clippy was installed and ran; it was not an unavailable
tool. It rejected `Operation::Config` and `test_read_config` as unused in Mac
test builds. Both callers (`owned_process::tests` and
`managed_profile::native::tests`) are gated to Windows. The helper variant,
its match arm and method previously used only `cfg(test)`.

Correction: gate all three with `cfg(all(test, windows))`. This matches the
actual supported test consumer set and keeps the Windows config-read tests
intact. No `allow`/`expect`, warning-flag relaxation or production REST/trust
behavior change is introduced. Strict Windows all-target Clippy passed after
the correction; native Mac Clippy on corrected source is still pending.

## Lint-only continuation

The shared runner now accepts `--lint-only`, exposed by `MAC-LINT.command`.
Its scope is `macos_native_compile_and_clippy_only`, probe
`m8-macos-lint-20261108-v1`. It verifies the pinned archive, builds the test
harness and runs the same strict all-target Clippy. It returns
`MAC-LINT-SONUC.zip` with `result.txt`, `build.log`, and `clippy.log` when
reached. Outcome distinguishes `compile_and_clippy_passed`/`failed` or build
failure. Metadata explicitly says `native_tests=not_rerun`.

Package `cryptex-mac-lint-20261108.zip` has six allowlisted members:
`MAC-TEST.command`, `MAC-LINT.command`, `MAC-LINT-BASLA.txt`,
`SOURCE-COMMIT.txt`, `SOURCE-SHA256.txt`, `source.zip`. Both launchers preserve
executable Unix mode/LF. The new invocation does not run the successful test
groups or access Keychain. Bash syntax, non-Mac refusal and Windows strict
Clippy were checked; actual corrected Mac compile/lint results remain pending.

Successful native primitive tests do not close the production Mac parity gaps
listed above, Windows endpoint failures, physical power-loss or independent
review gates. No source signature, remote attestation or normal publication
activation is inferred from the supplied ZIP.
