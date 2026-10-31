# Windows package transaction filesystem candidate — scoped PASS

Prior base `5c58f29`; implementation and tests are committed with this evidence.
Node `v24.19.0`, Windows; no dependency/toolchain change or GitHub workflow.
All work was headless: no Desktop/daemon launch, installer, game input or focus change.

## Verification

- `node --test tools/m0-lifecycle-compare/scripts/package-transaction.test.mjs`:
  **28/28 PASS**, no skipped tests.
- `node --check` for candidate engine, crash helper, worker and proof runner:
  PASS. No dedicated JS formatter/linter is configured for this directory;
  syntax and `git diff --check` were checked. Rust/application code is unchanged.
- Tests include declared pair incompatibility, payload mismatch, unsupported
  schemas/protocols, malformed selection/journal, retained diagnostic journal,
  complete previous-pair restoration, health rejection/exception/tampering,
  incomplete slots, existing-root/slot refusal and path/link guards.
- Owned subprocesses were force-killed after staging, intent, selection, health
  and commit; recovery selected baseline before commit and candidate after commit.
  Repeated recovery produced the same selection.
- Every applicable test compares synthetic fixture membership and SHA-256 bytes
  before/after. No personal data or retained P0-1 corpus was accessed.

Initial development failures were corrected without weakening assertions:
Windows file fsync needs a writable handle on the copied destination (`r+`);
fixture comparison must run before temporary-directory cleanup. Final checks
passed after these fixes and the additional negative cases.

## Retained real-binary-copy experiment

`run-package-transaction-proof.mjs` passed all **8/8 scenarios** with retained
baseline/candidate binary copies. Root outside Git:
`%LOCALAPPDATA%\Cryptex-Dev\p0-2\package-transaction-e6a0d07b841b4618bc9186c7d60a9b01`.
It retains `report.json`, per-case slots, selection records and unchanged fixtures.

Baseline Desktop was the actual retained NSIS payload:
`2b91af7c6d497ca91399f9dd831ffd0759bbff4f7491476b738260a90f8002a7`.
Candidate Desktop was the retained **loose** 0.0.1 build:
`2da93e194cda42e9d50993fb04d40951ef4aa17bdd368d33d498655edf12e9e5`.
This is explicitly not the NSS-patched installed candidate hash. Daemon copies
both hash to `a958e52813d5f59561e20e1f80a1d261f3028b9fefc5fa3df093fa1589493658`.

| Case | Selection after recovery |
| --- | --- |
| Compatible commit | candidate |
| Incompatible declared pair | baseline |
| Failed health callback | baseline |
| Kill after staged (PID 25776) | baseline |
| Kill after intent (PID 25508) | baseline |
| Kill after selected (PID 21516) | baseline |
| Kill after healthy (PID 16440) | baseline |
| Kill after committed (PID 31068) | candidate |

Fixtures before/after had identical membership and digests:

- `blob.fixture`: `6c54da56d546a413666e56c6d8574f0f2c6f3e0e06cd8a7c15fe6e0a8aa67946`.
- `sealed-events.fixture`: `1d2a3dfc7eb1aea3b84e6600a438de2a7dc5f881faf3e621430c304b52eefa36`.

## Acceptance boundary

This proves serialized filesystem-candidate behavior under updater-process kill
and injected health rejection. It is not integrated automatic rollback for the
installed application. No real Desktop or daemon health ran in this experiment;
protocols were declared metadata, not negotiated. Fixture bytes are not canonical
events. Windows power-loss durability, concurrency, signed macOS `.app` resources,
native helper/startup integration and actual canonical-data acceptance remain open.

The candidate is proposed in
`docs/adr/ADR-0012-update-orchestration-candidate.md`. ADR-0012 stays unaccepted.
Next is the focused Mac test plan, then review of native integration and its
failure/data-preservation gates; do not repeat passed login/tray/lint suites.
