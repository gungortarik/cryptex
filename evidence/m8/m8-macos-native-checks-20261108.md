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
archive member/mode/path/hash verification and diff checks. These are wrapper
checks only. Actual native build/test/Keychain results are pending.

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
