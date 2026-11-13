# Disabled Mac endpoint/generation lifetime correction

Current state: strengthened source `3a12bcb` passed exact Mac/Windows CI and
independent source review. Eight SDK cases and five HTTP tests per platform
passed; the two bounded P2 findings are closed. Earlier pending checkpoints
below are chronological; final acceptance and limits are at the end. M8 is open.

The owner authorized the remaining process/connection security checks after the
combined-loss owner receipt passed. This work stays inside ADR-0021's default-off
transport seam. The [bounded contract](../../docs/proposals/m8-macos-endpoint-lifetime-check.md)
fixes the test inventory and explicitly excludes product activation and atomic
per-byte FD-handoff attribution. It introduces no runtime process role.

## Findings and source corrections

Independent technical agent review at `2114cd9` found two P2 boundary gaps:

- `find_socket` finished its double read before retaining the task. The new pin
  records exact FD, reverse tuple, socket/PCB identity and generation count;
  native validation checks this same witness after retention and before send,
  bracketing it with retained root/server generation and image checks. Changed
  discovery evidence returns C4/typed PeerChanged; the caller's accept-lag loop
  retries only an undiscovered PeerNotOwned, never a changed original witness.
- HTTP kept a passive guard. The same connected HTTP implementation now invokes
  the adapter immediately before credential-bearing send and checks its original
  absolute deadline again. A complete observation requires retained root/server
  generation/image validity. Normal Connection:close need not keep a server FD
  open. Shutdown ACK does not claim live identity and still requires the existing
  independent exact owned-tree clean-exit check.

A private HTTP seam is not a public connector or guard factory. Windows retains
its existing native policy. Typed refusal and all request/body/deadline limits
remain. Security checks are finite userspace observations, not an atomic guarantee
against deliberate FD forwarding or an exec after the last check.

## Fixed verification

The standalone signed SDK probe includes the actual adapter C source. Only its
test translation unit intercepts libproc: the actual child replaces the original
FD after snapshot two and retains the original socket under a duplicate FD. The
new code must refuse that original witness, not rescan for the duplicate. Other
fixed cases are IPv4 close, IPv6 close, listener-only refusal, retained socket
across same-PID exec before send, valid queued response followed by exec, root
loss with a live server, and Shutdown ACK followed by clean captured-child exit.
Captured children/rights remain bounded and outer cleanup reserves the unreaped
probe process group. This is native SDK evidence, not actual Syncthing attack
behavior. Existing actual Syncthing/Desktop tests run separately on the candidate.

Five Rust tests exercise the same private HTTP path for zero-byte pre-send typed
refusals, lost generation after a valid body, normal close and Shutdown ACK.
The deadline regression calls the shared pre-send boundary with a known expired
Instant; it has no short sleep or scheduler-dependent setup window.

Local Python: 103 tests, 5 platform skips, zero failures. Local Rust execution
was refused by Application Control 4551 on the build script; rustfmt is also
blocked. No security setting, binary relocation or bypass is attempted. Require
exact-source native CI before acceptance. Source review performed no execution.

## Dispatch chronology

`10018b3` pushed the correction. Its new unquoted Rust filter contained YAML
colon-space syntax; GitHub rejected workflow dispatch with HTTP422 and created
zero runnable jobs in invalid-workflow run43436071645. `586a157` uses a YAML block.
Run43436173081 then stopped at pinned rustfmt; it establishes no native result.
`5cb553e` applies the exact formatting delta and is running in
[CI43436384743](https://github.com/gungortarik/cryptex/actions/runs/43436384743).

Independent source review of the correction found no remaining actionable issue
in witness retention, generation checks, normal close/Shutdown distinction,
probe containment or marker collector. It identified a 10ms scheduler assumption
in the first deadline test; that was removed before the code commit and its
replacement and terminal PeerChanged correction were re-reviewed. This is a
separate technical agent source review, not human certification.

Current native acceptance is pending. The old owner-Mac combined receipt remains
accepted for its original source; no unchanged owner rerun or untested package
is requested. Other protocol/product/platform and human acceptance gates remain
open; see the [M8 checklist](../../docs/M8_REMAINING_ACCEPTANCE.md).

Primary SDK cross-check: Apple's
[libproc wrapper source](https://github.com/apple-oss-distributions/xnu/blob/main/libsyscall/wrappers/libproc/libproc.c)
returns process-group membership as a count of PIDs and per-FD information as a
byte count. These declarations/source semantics support the probe seam; they do
not replace actual native execution.


The first correction `5cb553e` subsequently passed both selected jobs in
CI43436384743 (Mac131922485600, Windows131922486337). Its eight SDK markers,
five HTTP tests, actual Syncthing/native pairing, restart/mutation/loss checks
and exported launchers passed. Those results remain checkpoint-specific.

Before final acceptance, source inspection strengthened peer-loss independence:
`3a12bcb6d6d1d2705081d367e9b561b12082a452` makes both same-PID exec cases keep a
separate captured idle root live while only the retained serving peer execs.
Conversely root_loss retains a live server. At most two captured children remain
in the same private session; original bounds and cleanup are unchanged. This
prevents a test from passing solely because root and server were the same task.
Independent read-only review found no actionable finding in this test-only
correction. No runtime/product code changed after the first passing candidate.
Final exact-source [CI43438187261](https://github.com/gungortarik/cryptex/actions/runs/43438187261)
is pending; do not substitute the prior pass for this stronger inventory.


## Final strengthened-source acceptance

Exact source `3a12bcb6d6d1d2705081d367e9b561b12082a452` passed both selected jobs in
[CI43438187261](https://github.com/gungortarik/cryptex/actions/runs/43438187261):
Mac131928632465, Windows131928632071. All eight fixed SDK cases and their aggregate
marker passed with distinct live root/execing-server evidence. The five HTTP
boundary tests passed on each platform. All 40 markers from the accepted earlier
combined-loss CI inventory remain present, alongside the nine new SDK markers:
49 exact fixed markers in the Mac job log. Existing actual Syncthing, Desktop
pairing, eleven mutations, restart/replay, protected-content and single/combined
loss tests and all exported regression launchers passed. Exact native formatting
and strict lint passed. Other milestone jobs were not run by this scoped dispatch.

The [machine receipt](macos-endpoint-lifetime-native-3a12bcb-ci.json) records exact
source/run/jobs, marker/test inventory and limits. Artifact14044059858 contains
the existing regression packages; no new SDK owner package is delivered or
requested. New SDK negatives are native-CI evidence, not an owner-Mac execution
claim. The original accepted owner-Mac combined receipt is preserved unchanged
for its original source. Source review and execution remain separate evidence;
see the [review record](m8-macos-endpoint-independent-review-20261113.md).

The two bounded P2 findings and this fixed safety inventory are closed. This does
not establish atomic per-byte attribution or malicious voluntary FD-transfer
resistance, production orphan cleanup, normal product activation or M8 completion.
The [remaining checklist](../../docs/M8_REMAINING_ACCEPTANCE.md) records the two
broader areas: protocol/Mac product integration decisions and final integrated
platform/security/human acceptance. No speculative M16 stress campaign is added.
