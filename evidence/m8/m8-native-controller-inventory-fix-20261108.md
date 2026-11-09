# Native controller pilot: VM inventory helper failure

Date: 2026-11-08. This is a failed native receipt followed by a test-only fix,
not an actual transfer/publication or milestone pass.

## Returned evidence

- ZIP: `C:\Users\Tarik\Downloads\DENETLEYICI-SONUC-d1e5c535-4975885cbff4.zip`
- SHA-256: `c3d5e2228947bfaa599d845a297eef39e79dcf589bcca3e87409f2a50fd2ee63`
- Exact members: `runner-result.json`, `build.log`, `native-test.log`, `relay.json`.
- Source: `e1b2aedcf5f94461f15e02ba49e2662bd77d62ab`.
- Source archive: `a720e511f72bd93c31bb382abaa3abbd802605f375336f6956fed31bd16c7227`.
- Session: `d1e5c535ae0e459fb699be8f7324c149`; VM Windows 10.0.26200.0, NTFS.
- Build exit 0; native test exit 101; runner exit 1.
- Test summary: 0 passed, 1 failed, 0 ignored, 585 filtered, 1.27 seconds.
- `native_transfer_test.rs:40`: `NotADirectory`, Windows error 267.
- No checkpoint/native pass receipt or public request was produced.

## Diagnosis and bounded correction

`canonical_hashes` assumed every entry immediately below `canonical/events`
was a device directory. The canonical writer also creates `inventory.lock`
there. The failure occurred after synthetic fixture preparation and before
native worker startup/export. It says nothing about network or import success.

Only a regular file named exactly `inventory.lock` is excluded. Unexpected
root files and non-directory entries fail; a directory in the lock slot fails.
Event `.open`/`.seg` hashing and all immutable-byte assertions remain intact.
Production writer, transport ownership and import gates are unchanged.

## Checks

- Added a real signed-pair fixture regression: observed the identical error 267
  before the correction; after correction it detects changed bytes, added
  sealed segment and deleted event file. These deliberate mutations are confined
  to that fresh disposable test fixture, never owner data.
- All three `canonical_inventory_` regressions pass, none ignored.
- `cargo clippy --locked -p cryptexd --all-targets -- -D warnings` passes.
- `cargo fmt --all -- --check` and `git diff --check` pass.

Do not repeat the known failing host owned-endpoint preflight or weaken its
checks. Next live execution is the corrected package on the VM. Retain the
unchanged host relay session from the preceding pilot evidence. M8 remains open.
