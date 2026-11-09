# Native VM receipt: publication pause timeout

Date: 2026-11-08. Partial integration evidence; native test failed.

- ZIP: `C:\Users\Tarik\Downloads\DENETLEYICI-SONUC-d1e5c535-5b92b54889a6.zip`.
- SHA-256: `6278662371925b66acd49fc30e979e9c56bf0027bd48e6c4542af75b37d41f7d`.
- Five members: runner-result.json, build.log, native-test.log, checkpoint.txt,
  relay.json; no native result.json.
- Source `393d0c4dcdfc36ddd72c0d6386d067ada6634d0c`, archive
  `c039876748e35f947661d9a754f6465b3464d1c39b536f3ce906192cd9505e92` match delivery binding.
- Build 0, test 101, runner 1; 0 passed/1 failed/0 ignored/589 filtered, 149.93s.
- VM Windows 10.0.26200.0, NTFS; session d1e5c535ae0e459fb699be8f7324c149.

Reached owned_rest_ready, waiting_for_host_delivery,
actual_delivery_verified_before_publication,
owned_job_crash_restarted_identity_unchanged. Paired host receipt confirms 6557
bytes, 100% peer completion and clean host relay stop.

At source line 403, started_recv.recv_timeout(10s) returned Timeout. It awaited a
test SecureStore hook after the canonical destination exists and before rebuild.
This does not show whether publication returned a transient tick, rejected, held,
or stalled before that hook. Final result JSON was not produced. Worker Drop
reported managed_worker_ShutdownDeadline; VM cleanup is unresolved in this receipt.
Do not infer successful canonical publication or cleanup, or call M8 complete.

## Local investigation

Extracted the same busy-admission/IPC/publisher pause sequence into a shared test
helper and ran it using a real signed fixture, real publication and named-pipe IPC,
but explicitly synthetic transport readiness. It passed (one segment, three events,
zero identical), including busy IPC during commit and successful later search.
Added early poll result/error and worker-status timeout diagnostics; no production
code or security/readiness assertions changed. Cause remains unconfirmed.

Strict all-target daemon Clippy, fmt and diff checks pass. New read-only diagnostic
collector tested on Windows PowerShell 5.1 with isolated fake roots: correct session
exports only bounded allowed diagnostics and inventory/hashes; wrong session fails;
key/config/canonical body sentinels are absent and input bytes remain unchanged.

Next: owner opens a small collector for retained run 5b92b54889a6. No test rebuild,
new source package or repeated live transfer yet. Inspect native logs and exact
incoming/canonical/staging inventory from its result before deciding a correction.

Collector delivered: `C:\Users\Tarik\Downloads\cryptex-denetleyici-tani-6c88bb8.zip`,
2158 bytes, SHA-256 `69f59d86168454a4dcf494871a1463dffe176b47b472fea2fe77583baddc93aa`.
Four exact members verified (TANI.cmd, TANI.ps1, BASLA.txt, SOURCE-COMMIT.txt).
Script bytes bind source `6c88bb86e21c5edeac367624255ef009f244c4b5`.
Launcher pins prior run/session, uses Windows PowerShell 5.1 and leaves all test
processes/data unchanged. Await DENETLEYICI-TANI-5b92b54889a6-*.zip.
