# M4 final dual-OS CI gate — 2026-10-10

- **Status:** PASS
- **Branch:** `m4/knowledge-engine`
- **Tested SHA:** `b1c43e2c36534e360cb06fb5cacef5bf6fd210ee`
- **Run:** [38021522882](https://github.com/gungortarik/cryptex/actions/runs/38021522882)

## Results

- Readiness (`windows-latest`): **PASS**
- Readiness (`macos-latest`): **PASS**
- IPC cross-account ACL (`windows-latest`): **PASS**
- Unsigned Windows package: skipped by workflow design

Both readiness jobs completed the formatting, strict workspace lint/check,
Rust test, schema drift, frontend, release namespace, and Node boundary steps.
The Windows readiness job also built the embedded-frontend release Desktop and
ran the release-only lint and foundation binary checks.

## Candidate changes covered

Since the prior CI-tested product behavior SHA `f8e7636`, this candidate adds
framed guarded-search IPC round trips for allowed and held retrieval states,
an interrupted SQLite migration/reopen regression, and shorter Unix-only test
socket paths for `sockaddr_un` compatibility. The production peer identity
check remains in `handle_connection`; the test seam receives the resulting
peer classification. No production v2 writer or activation path was added.

The first run on the IPC test candidate, [38020497927](https://github.com/gungortarik/cryptex/actions/runs/38020497927),
failed on macOS because the test socket path exceeded `sockaddr_un.sun_path`.
The Unix test fixture path was shortened in `b1c43e2`; this final run covers
that correction and passed on both operating systems.

## Gate status

This CI gate does not complete M4 by itself. The macOS native visual review and
final Definition of Done/owner review remain open. Production v2 structured
event writing remains disabled. No merge or M4-complete claim is made.
