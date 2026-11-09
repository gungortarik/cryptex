# Native controller: Windows long-path publication correction

Date: 2026-11-08. Native end-to-end pass remains pending.

## Returned diagnostic

- ZIP `C:\Users\Tarik\Downloads\DENETLEYICI-TANI-5b92b54889a6-1729dd82.zip`.
- SHA-256 `cda053022ccafc5a52d6e5187c01594d10957085b713db0f34eabf53d2622cad`.
- Five members: checkpoint.txt, diagnostic.json, native-1.log, native-test.log,
  runner-result.json. Run/session match the previous failed receipt.
- Incoming synthetic segment: 6557 bytes, SHA-256
  `3B605CB84596D5908769BE3485BB70A651ADD7633320EC4825CBCBA8E56ADC12`,
  identical to source/request. Receiver peer canonical directory has writer.lock
  only; no peer segment is present in inventory. No completed import is claimed.
- Native log records actual delivery at 22:17:21, restart at 22:17:23 with the
  same device ID. Shutdown begins at 22:17:33 without an Exiting line. Cleanup
  remains unresolved from this receipt; it is not a clean-shutdown pass.

## Reproduction and correction

The new publication/IPC helper passes under a short TEMP directory. Running the
same test with TEMP under `%LOCALAPPDATA%\Cryptex-Dev\native-transfer\<12hex>\test-temp`
fails before the pause with Publication(Rejected(Io(os error 3))). This reproduces
a platform defect consistent with the VM's long staging paths and inventory.

Rust fs accepts long paths but the staging adapter passed ordinary DOS paths to
raw CreateFileW/MoveFileExW APIs. Added one Windows-only conversion for already
validated absolute local-drive paths: extended prefix, slash normalization,
NUL/parent/nonlocal namespace refusal, no canonicalization through links.
Applied to directory volume inspection, staged no-replace move and parent flush.
Existing owner/reparse/link/volume checks, share flags and MOVEFILE_WRITE_THROUGH
without replacement are unchanged. No security settings or owner data changed.

## Verification

- Same production-shaped TEMP reproduction now publishes one segment/three events,
  holds real named-pipe reads during publication, and returns verified search after.
- Persistent regression now uses an elongated receiver runtime independently of
  developer TEMP. It passes.
- New >260-character Unicode/slash path adapter regression verifies existing
  destination refusal and byte preservation, successful no-replace move, volume
  inspection and directory flush. File-as-directory and invalid namespace/path
  cases are refused.
- Import staging suite: 65 passed, 0 failed, 0 ignored, including process/crash,
  no-replace and ancestor/junction/hardlink safeguards.
- Incoming publication suite: 14 passed, 0 failed, 0 ignored.
- Strict daemon all-target Clippy, fmt and diff checks pass.

No new live native receiver test executed locally (known host owned-endpoint
failure). Corrected source must be run once on VM before claiming native pass.

## Fresh relay for next package

Private session `%LOCALAPPDATA%\Cryptex-Dev\native-controller-relay\45278d8e00394a62b8d3607208858878`.
Native ID `L522WBJ-BF7DO2L-MBUCWLS-JXPUBRO-BKB6ZFD-IUC2MVY-QLLUQ6T-43FUNQK`.
Private listener descriptor `tcp://192.168.177.1:14687`; native profile generated
with pinned executable and private ACL, no sender listener launched. Await fresh
VM request for this session. Prior session/evidence retained unchanged.
