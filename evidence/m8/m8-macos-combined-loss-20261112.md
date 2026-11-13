# Disabled Mac combined supervisor-loss acceptance record

Current state: exact-source native Mac/Windows CI and exported launcher passed
on `2890ad2`; independent source review complete, owner-Mac combined receipt
pending. See the final acceptance section and machine receipt below. Earlier
sections retain chronological checkpoints and superseded initial inventories.

The owner requested completion of the next combined daemon/helper loss test
after the exact-source owner Mac identity/pairing receipt passed. The
[bounded fixture contract](../../docs/proposals/m8-macos-combined-loss-fixture.md)
keeps ADR-0021's normal product routes closed. There is no canonical writer,
native transport Observe/Stop command or renderer capability change.

Candidate `52cdc57db5018679ae5611f208815cc256605c12` adds a dedicated combined-loss
driver and same-image authenticated test guardian. The real helper is killed
through its exact retained Child; waitid(WNOWAIT) proves exit without releasing
the PID. The worker then parks without normal cleanup. The external collector
kills only its captured daemon Popen child and requires its SIGKILL exit.

Before cleanup the test requires the authenticated guardian alive separately
from two real engine processes, the helper exited, the captured daemon exited,
unchanged active/profile/config/cert/key/receipt files and two refused replacement
attempts. It checks the existing OS credential against its unchanged binding
before and after, under the same profile lease while the guardian is alive.
Snapshots use bounded no-follow file descriptors and compare inode, mode,
modification time, size and bytes. Reported process IDs and ps output are
read-only evidence, never signal targets.

The guardian occupies its inherited private group, has no credentials or writer
role and never changes group/session. It cleans only its own current group with
kill(0) on fixed K, EOF, invalid control input or its fixed watchdog. The collector
requires group death and then performs existing lease-fenced fixture API cleanup.
No stale numeric group or PID lookup authorizes cleanup. Unresolved evidence
fails; no fallback adoption or raw Keychain deletion is allowed.

Early independent source review found an ARMED endpoint-lifetime race: the
guardian closed its inbox immediately after sending the last required message.
The candidate retains that inbox through the entire armed lifetime; the parent
closes its own endpoint only after consuming ARMED. Further review and native
execution are pending at this checkpoint.

Local checks: 97 Python tests (5 platform skips), cargo format/diff checks and
strict Windows daemon clippy pass. New owner/export/parser/regression checks
cover exact package dependencies/mode, missing/changed state, distinct live
engines versus guardian, failed/expired owner runs and truthful cleanup fields.
Mac-only Rust code still requires actual Mac execution. Exact-source scoped
[CI 43356227775](https://github.com/gungortarik/cryptex/actions/runs/43356227775)
was dispatched. No current candidate execution is claimed before its result.

The separate MAC-BIRLIKTE one-click package runs only this new scenario and
emits MAC-BIRLIKTE-SONUC-<nonce>.zip. The exported launcher is also executed in CI
and must return the matching source, passed outcome and three true scenario,
credential and runtime checks before delivery. Older accepted Mac identity/
pairing owner tests are not requested again.

This fixture proves combined-loss exclusion under explicit test containment,
not production automatic orphan cleanup, Windows Job equivalence or M8 closure.
Owner-candidate execution, other endpoint/exec races, product publisher/support
floor, activation and final protocol/platform/security/human gates remain open.

## Pre-loss continuity correction

Independent review identified a second P2 coverage gap at `52cdc57`: the first
file snapshot was captured after both injected losses. A changed valid marker
or receipt during injection could become the accepted baseline. The initial
CI dispatch was cancelled before acceptance; it supplies no complete pass.

`8623dc34810fbb8fd310f511116c76d1db6c1756` adds a two-phase fixed control:
prepared readiness exposes only read-only group evidence while both supervisors
are live. The collector captures the original file/marker metadata and bytes
and live engine rows, then sends X. Only after consuming X does the daemon
worker kill its exact helper, observe exit unreaped and park. The collector
requires that second ready phase before killing its Popen daemon. The guardian
cannot steal X because it does not read the pipe while its actual parent is live.
Both ready phases retain the same absolute 55-second startup bound.

The same original snapshot must match immediately after both deaths, around
both refused replacement attempts and after guardian cleanup. A new regression
requires a valid-shape marker mutation during the loss transition to fail before
any post-loss integrity/replacement command. All 98 Python tests pass (5 native
skips); format/diff checks pass. Focused source review and exact-source scoped
[CI 43356619079](https://github.com/gungortarik/cryptex/actions/runs/43356619079)
are pending; no native execution or owner-Mac acceptance is inferred yet.

Focused independent read-only source review through `8623dc3` found no remaining
actionable blocker. Both P2 findings are closed in source: retained ARMED inbox
and a real pre-loss baseline shared across transition/refusal/cleanup checks.
The review covered fixture containment, direct invocation refusal, owner/export
mode exclusivity and teardown; it performed no execution or edits. Actual native
CI and a matching owner-Mac combined-only receipt remain separate requirements.

Native feature CI at `8623dc3` stopped during strict clippy on two one-byte array
comparisons (`byte_char_slices`), before combined-loss execution. `70a5840a01d057f112ea5da3e9aa38a02ff0b218`
uses the lint-requested equivalent byte-string array expressions. Focused source
review confirmed the delta has no behavior, authority, cleanup, deadline or
assertion change. No warning was suppressed. Format/diff checks pass. Current
exact-source [CI 43357303333](https://github.com/gungortarik/cryptex/actions/runs/43357303333)
is pending; the earlier failure supplies no native fixture acceptance.

## Lost diagnostic pipe correction and explicit second variant

The guardian's error path originally used eprintln before mandatory group kill.
Rust documents that a failed stderr write can panic; a disappeared collector
could therefore bypass cleanup at that point. `1bcf5dd73eca5e4d288bd0d789868de6425295cd`
uses a non-panicking best-effort writeln Result solely for that diagnostic and
keeps the mandatory own-group kill afterward. A Rust BrokenPipe-writer regression
requires the diagnostic to return normally. No validation/refusal is swallowed.

Two fresh combined-only cases now run: normal K cleanup and an invalid control
byte with the sole collector output reader already closed. Each requires the
pre-loss baseline, both supervisors dead, engines alive, two unchanged refused
restarts, existing-key continuity, group death and fenced API cleanup. Aggregate
markers require both cases; the owner/CI receipt requires a fourth separate
guardian_output_loss_cleanup_verified field. The exported launcher's absolute
180-second bound and all existing native/request/cleanup bounds are unchanged.
EOF/watchdog behavior is source-inspected; these two cases do not newly prove
that entire matrix or production automatic orphan cleanup.

Independent source review found no actionable issue in this delta. Local Python
tests total 99 (5 platform skips), and diff checks pass. The last local cargo fmt
invocation was blocked by Windows Application Control (4551), so no local format
pass is claimed for this final delta; native CI format/lint remain required.
Earlier source `70a5840` CI is a preliminary, superseded candidate. Current exact
[CI 43358923466](https://github.com/gungortarik/cryptex/actions/runs/43358923466)
must pass before its matching combined-only owner package can be delivered.

That dispatch stopped at pinned cargo fmt on the diagnostic macro layout, before
new native execution. `273716d117652bcf214569c3c47b5bda082f8fff` applies exactly
the formatter's requested whitespace; no logic/assertion/deadline change. Current
exact-source [CI 43359098593](https://github.com/gungortarik/cryptex/actions/runs/43359098593)
is pending. Do not infer a native pass from any earlier cancelled/failed run.

The superseded `70a5840` run subsequently passed Windows and native Mac feature
lint/build checks, but stopped in the PREEXISTING identity driver with typed
mac_transport_Deadline / real_managed_mac_transport_identity_failed, before any
combined-loss control ran. There is no combined-loss pass from that dispatch.
No existing deadline is raised or old assertion weakened in response; inspect
the current exact candidate independently.

`273716d` also passed Windows, native Mac feature lint/build and existing native
certificate exchange, but its preexisting identity driver failed at
native_channel/helper_accept with Deadline before reaching the new cases.
Comparison against the accepted `0d98a16` normal helper path found no functional
change before native accept: false mode keeps FLAG, null stdin, clear environment,
setsid, image policy, hint and all deadlines. Added application hello bytes run
after native connect and are identical in false mode. This comparison does not
establish the cause of the runtime timeout; it is not dismissed as a flake.

`a473f82617b66c319b4ef3fa87a1c5992c19764e` adds fixed non-secret elapsed startup
stages on both sides and a typed child-connect diagnostic, with the same original
15-second outer deadline and all native caps/checks unchanged. The scoped CI job
uses DEV_DEBUG=0 from its first build, matching the fixture wrapper's existing
profile and avoiding repeated debug2/debug0 builds; debug assertions and test
selection are unchanged. This scheduling/build correction alone is not evidence
of the timeout's cause or a native pass. Current exact
[CI 43361542189](https://github.com/gungortarik/cryptex/actions/runs/43361542189)
and focused source review are pending. No failed candidate package is delivered.

Focused read-only source review of `a473f82` found no actionable issue: phase logs
contain only fixed static stages, elapsed time and typed closed errors; original
deadline origins and all native caps/checks remain unchanged. The job-wide debug
information setting matches the wrapper without altering debug assertions.
No review execution or edits occurred. Actual phase observations and native
combined-loss acceptance remain pending, separately from that source verdict.


### Timing observations and scoped optimized fixture candidate

The `a473f82` run completed with Windows success (job `131675977993`) and Mac
failure (job `131675978248`). Core transport identity/restart/loss checks and all
eleven live mutation refusals passed; existing native pairing failed at
`request_receive` Deadline before the combined fixture. Measured initial
parent Release verification was 5146 ms, child 4910 ms; hint-to-accept was 17 ms
and child connect about 27 ms. The 2.5-second initial handshake cap is therefore
not increased. These observations do not prove the cause of the later request
receive deadline, nor establish a combined-loss pass.

`7aba620` applies CLI-only Cargo overrides identically on all four call sites
(five executed commands: clippy/build/owner unit tests/two adapter test runs).
Both dev and test retain explicit debug assertions and overflow checks while
optimizing sha2:0.10.9 and blake3:1.8.2 at 3 and their generic consumer cryptexd
at 1. No normal Cargo manifest/profile, panic, LTO or target-feature change.
Every file byte, comparison, native operation and timeout stays in place.
[Cargo configuration](https://doc.rust-lang.org/cargo/reference/config.html#command-line-overrides)
permits scoped profile overrides; generic code may be emitted by its
[calling crate](https://doc.rust-lang.org/cargo/reference/profiles.html#overrides-and-generics).
This is an exact-fixture build recipe, not an opt0 timing fix or product
performance claim.

Slow pre-package/session/post-package observation diagnostics include only a
fixed stage, elapsed time and success flag, also on failure; stderr failure is
ignored and the original typed result and outer package-pin lifetimes remain.
A Rust regression checks exact success/error propagation. Local Python checks:
100 tests, 5 platform skips; full CLI argv parses with locked Cargo metadata;
diff checks pass. Application Control continues to block local rustfmt, so
native CI must establish formatting and feature lint/tests.

The existing independent reviewer approved this exact delta read-only with no
actionable finding; no execution was attributed to source review. Exact-source
[CI 43364164957](https://github.com/gungortarik/cryptex/actions/runs/43364164957)
is pending at `7aba620`. No failed candidate package is delivered.


`7aba620` stopped at pinned rustfmt's closure-layout check before native runtime
execution. `2890ad2` applies exactly that whitespace correction. The current
exact-source dispatch is
[CI 43364276809](https://github.com/gungortarik/cryptex/actions/runs/43364276809),
pending; no runtime logic or test selection changed in the formatter follow-up.


For `2890ad2`, Windows job `131684883575` completed successfully: startup 10,
Desktop 46, standard-user approval/session selections 17/80/15 passed, zero
failures. Native Mac job `131684883357` remains pending. The Windows result does
not establish the Mac-only combined-loss behavior.


## Exact-source acceptance and owner continuation

The final candidate `2890ad24359705ef34775422bccf6198d274a171` passed both selected
jobs in [CI 43364276809](https://github.com/gungortarik/cryptex/actions/runs/43364276809):
Mac `131684883357`, Windows `131684883575`. Exact-source format and strict feature
lint/tests passed. The four combined markers establish both K and invalid-control
with closed output cases; `exported_combined_loss_launcher=passed` additionally
requires the actual exported launcher, exact source and all four true result
fields. Existing native identity/pairing/loss, eleven mutations, protected
content, owner sessions, durable restart/replay and old exported launchers also
passed. Fixture credential cleanup passed. No timeout or full-byte validation
was removed. Successful output does not include new elapsed values, so no claim
is made about the precise speed-up or opt0 timing.

The existing independent reviewer rechecked current HEAD and final diff through
`2890ad2`, finding no remaining actionable source issue. Cumulative review covers
exact-child/native guardian binding, retained inbox, own-group cleanup, pre-loss
baseline/X ordering, captured supervisor death, unchanged engine identities,
no-repair replacement refusal, nonpanicking diagnostic with closed output,
profile-lease-fenced credential teardown, two-case collector, exported receipt
requirements and scoped optimization/diagnostic delta. No tests or CI were run
by the reviewer. This is technical agent source review, not human certification.

The [machine-readable receipt](macos-combined-loss-native-2890ad2-ci.json) records
all exact markers, job IDs, configuration and artifact checks. Downloaded artifact
`14010440338` contains the new ZIP: 23,791,841 bytes, 16 unique safe regular members;
all signed-inventory member sizes/hashes match, executable modes match, and the
launcher plus three dependency files match current source exactly. Only fixed
allowlisted bundle/launcher/metadata files are present; no private signing
material or runtime profile is shipped. CMS/image trust verification is native
CI evidence, distinct from the local member-hash check.

Owner package in Downloads: `CRYPTEX-MAC-BIRLIKTE-2890ad2.zip`, SHA-256
`f8d844c2d6a14b49d0471a274e6f26e04d858892c64808fec99db0d2787a3d8f`.
Architecture arm64, fixture macOS 26+ (no product OS-floor decision), development
certificate expiry **2026-11-14 04:17:07 UTC**. Owner execution is still pending:
extract on the Mac and double-click `Cryptex-Mac-Birlikte/MAC-BIRLIKTE.command`,
then return `MAC-BIRLIKTE-SONUC-*.zip`. Require source `2890ad2...`, outcome passed
and all four booleans true. Do not repeat old owner identity/pairing checks.
The aggregate receipt does not independently attest hardware or OS.

The new native acceptance applies only to this optimized disabled fixture.
Production orphan cleanup, EOF/watchdog native cases, other endpoint/exec races,
publisher/OS floor, product activation, cross-device product acceptance and final
M8 gates are not closed by these results. M8 remains open.
