# M4 final dual-OS CI gate — 2026-10-10

- **Current M4 CI acceptance:** PASS
- **Tested candidate:** `f9bd8945151a44d450bd8340a6794a5f33df3e72`
- **Product-code baseline:** `b1c43e2c36534e360cb06fb5cacef5bf6fd210ee`
- **Coverage correction:** `9500e2f467565f7dad68dd72612ade7eb86b265f`
- **Required run:** [38033047073](https://github.com/gungortarik/cryptex/actions/runs/38033047073)
- **Integration PR:** [#10](https://github.com/gungortarik/cryptex/pull/10)

## Corrected final result

GitHub reports this exact candidate completed successfully. Windows readiness,
macOS readiness and Windows cross-account IPC ACL all PASS. Unsigned packaging
is skipped by workflow design, not certified by this result.

Both readiness jobs explicitly passed `Test M4 knowledge reducer and SQLite
recovery` and `Test Desktop retrieval response policy`. This closes the actual
test-invocation gap in the historical run below, including interrupted migration
and process reopen coverage. Formatting, strict workspace lint/check, full daemon
tests, event/storage/projection tests, schema drift, frontend, release namespace
and Node failure/recovery boundaries also passed their configured steps.

Subsequent closeout changes are documentation only; no production behavior,
dependency, test or workflow differs from the tested candidate. No additional
Actions run is needed for those documentation changes.

Owner-delegated final review and scoped completion are recorded in
`evidence/m4/m4-final-review-20261010.md`. Production v2 activation remains CLOSED.
Historical pending/open statements below describe their capture time and are
superseded by this corrected result and the final review.

## Historical configured-job result

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

## Historical gate status (superseded by corrected result above)

### Final DoD coverage correction — 2026-10-10

This run passed its configured jobs, but the final acceptance audit found
that readiness did not invoke the `cryptex-knowledge` unit suite or Desktop
`test:search-policy`. In particular, the interrupted-migration test described
above was compiled by workspace lint but was not executed by this workflow.
The missing test steps are being added to both readiness platforms. The full
M4 CI acceptance gate is therefore **PENDING corrected coverage**; do not
infer that these tests passed from this historical run. See
`evidence/m4/m4-final-dod-audit-20261010.md`.

Corrected run [38033047073](https://github.com/gungortarik/cryptex/actions/runs/38033047073)
is **PENDING**, dispatched once at `f9bd8945151a44d450bd8340a6794a5f33df3e72`.
The API confirmed both readiness jobs and Windows IPC ACL started on that SHA.

The later macOS visual record closes the scoped retrieval-safety visual check
at product-equivalent tip `ebdd8c9`; actual tray Quit remains NOT VERIFIED.
The following paragraph records the gate state when this CI run was captured.

This CI gate does not complete M4 by itself. The macOS native visual review and
final Definition of Done/owner review remain open. Production v2 structured
event writing remains disabled. No merge or M4-complete claim is made.
