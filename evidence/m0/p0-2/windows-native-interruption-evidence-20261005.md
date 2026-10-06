# Windows native interruption and M0 foundation — scoped PASS

Recovery code: `7d7a60e865964f42af256f731d189cdf8bf50e4b`.
Golden/Doctor code: `7daadaad6cb83ad9753d6ca803e2c8e36e6332b3`.
P0-1 integration: normal merge `cb9069e`, approved by the project owner,
containing accepted P0-1 tip `6d4d10a`. ADR index conflict retains accepted IPC,
proposed background lifecycle and accepted sealed-pack layout together.
No retained corpus was changed or restarted.

## Recovery behavior

New pending native intents use schema 2 with updater PID and bounded runtime
prefix. Recovery under the native gate requires updater OS ESRCH, verified
complete slots, valid selection relationship and absent IPC endpoint.
Existing candidate/rollback runtimes require ordinary owned marker, complete
snapshot and ready PID records, both PIDs absent and daemon instance lock free.
Missing PID evidence, live/ambiguous process or legacy/corrupt metadata refuses
recovery before selection mutation.

A separate exclusive recovery claim is written before restoring selection and
starting baseline. It blocks normal startup and another recovery even if intent
was already removed. Successful baseline IPC validation clears intent and claim.
Interrupted/failed recovery retains its claim for inspection; automatic stale
recovery-claim reclamation is not implemented. No unrelated process is killed.

## Local checks and discovered harness failure

- Final-source full Node suite: **123/123**, no failures/skips, exit 0:
  previous 110, seven native interruption tests and six Golden/Doctor tests.
- Imported storage result-capture Node tests: **5/5**, exit 0.
- Windows Rust workspace: **89/89** across all test binaries, no failures/ignored
  tests, exit 0 after the harness correction below.
- Workspace format, strict all-target Clippy and locked workspace build PASS.
- Six new/changed Node module/worker/test/runner syntax checks and diff check PASS.

First full Rust workspace run failed one Mode B test: daemon exited before ready
with code 2 while another test in the same binary owned the per-user IPC endpoint.
Both real-daemon Mode B tests previously ran concurrently. A shared test mutex
now separates their lifetimes, retaining all duplicate/restart/clean-stop
assertions. Final default-threaded workspace run passed; no test was disabled,
policy changed or production endpoint made per-test. This is test scheduling,
not cross-process product serialization.

The merged P0-1 documents retain historical Markdown hard-break whitespace;
merge cached diff check reported that inherited formatting. Current new changes
pass diff check. Accepted decisions/evidence were not rewritten to erase history.

## Actual updater kill and baseline recovery

Final runner proof root:
`C:\Users\Tarik\AppData\Local\Cryptex-Dev\p0-2\native-recovery-proof-20261005-8d33666a06ff49d3bf5045237547137c`.
Report: `native-recovery-report.json` under that root.
SHA-256: `712b9f2f6ac97b44e3360ff343457e6e6098d1377214a066ae58565b68d204b2`.
Runner exit 0. Existing pilot-enabled `dd5021b` loose release input retained from
the owned-session proof was used; Rust application runtime was unchanged.

Owned baseline Desktop/daemon **13380/15060** cleanly stopped. Updater **16660**
was actually SIGKILLed after selecting candidate and before launching it.
Native gate released; a subsequent owner restored baseline and started actual
Desktop/daemon **30972/29584**. Executable paths were in the verified baseline
complete slot; Desktop parent runner **29104**, daemon parent **30972**.
Protocol 1 `health.pong`, matching daemon PID and `lock_held=true` passed.
Final clean stop exited 0, daemon PID gone, lock released and IPC absent.
Native intent/recovery claim were cleared and gate released.

Earlier passing recovery-only report remains at root suffix
`9a56f2e6ff61495ca3cab2ebc3d589a3`, SHA-256
`2300aa06ee790b2e5a8eb21f06b18412314ff0fd143618849ecfc3fe4fc6f89b`.

## Golden Brain and Doctor foundation

Golden foundation uses the integrated accepted P0-1 benchmark codec, not a new
production event schema: four deterministic synthetic records, 128 payload
bytes, two records per sealed pack. Two `.seg` objects are produced with no
active `.open` tail. Profile numbers are fixture settings, not product constants.
Full file/directory membership and SHA-256 bytes, including fixture manifest,
were unchanged across real native interruption/recovery. Runtime lives in the
owned app-data proof root outside Git; no personal data or retained corpus.

Doctor foundation checks actual helper IPC plus held instance lock and PID
matching. The recovered real daemon reported `healthy`, not configuration-only
health. Missing daemon is reported `daemon-absent`; failed probe is `attention`.
Malformed ready data is rejected; raw subprocess output/credentials are omitted.
Canonical store and Syncthing checks explicitly report `not-implemented`.
Doctor does not write canonical events or expose a recovery operation.

## Limits

Actual kill boundary is before successor launch. Live-child termination after
updater loss, post-launch PID evidence races, recovery-process interruption,
power loss and version-changing upgrade are not proved by this report.
Ambiguous cases fail closed; a stale recovery claim requires inspection.
Golden fixture proves synthetic sealed-pack byte preservation, not production
Brain/device identity, signatures or M2 event semantics. Mac parity pending.
No Actions, installed app/startup changes, signing or OS security bypass.
