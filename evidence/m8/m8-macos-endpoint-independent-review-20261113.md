# Disabled Mac endpoint lifetime: independent source review

The owner previously selected independent M8 review and delegated agent versus
external reviewer choice. The existing independent technical review agent
reviewed this bounded correction read-only. It performed no edits, tests or CI
dispatch. This is source review, not external human certification or device
execution. Product activation and final M8 review remain separate gates.

## Initial findings at 2114cd9

Two P2 code gaps were identified, without a remote exploit claim:

1. Socket discovery ended before task retention, without tying the retained
   generation back to the exact original FD/socket object.
2. HTTP retained a passive guard but did not revalidate before credential send
   or before publishing a fully consumed observation.

## Corrections reviewed

The final source is `3a12bcb6d6d1d2705081d367e9b561b12082a452`:

- Pin exact FD, reverse tuple, socket/PCB identities and generation count.
  Revalidate after retention, bracketed by root/peer generation/image checks.
- Map changed discovery evidence to terminal PeerChanged. Only an ordinary
  undiscovered PeerNotOwned uses the preexisting accept-installation retry;
  a failed original witness cannot be replaced by a rescan.
- Revalidate immediately before credential-bearing send and recheck the original
  absolute deadline after synchronous native work. Revalidate root/peer
  generation/image before accepting the complete observation body.
- Permit normal server-FD closure after a valid HTTP response. Shutdown ACK
  conveys no live identity and retains separate captured-tree exit verification.
- Keep five connected HTTP/private boundary tests and the eight-case standalone
  native SDK inventory. Test interception exists only in that translation unit,
  not in Cargo or the daemon. Only captured unreaped children are signalled;
  outer cleanup retains the private group leader before group cleanup/reaping.

The reviewer found a scheduler-dependent 10ms deadline-test setup assumption.
It was removed before the implementation commit: the regression now calls the
same private pre-send boundary with an already-expired Instant, without sleeps,
and requires callback then Deadline. The correction was re-reviewed.

Before final acceptance, both exec probes were strengthened to keep a distinct
captured root alive while only the serving peer re-execs. The root-loss case
proves the converse with a live serving peer. Read-only review of that final
test delta found no actionable issue; existing cleanup, worker count and
12-second alarm/15-second outer case bounds remain unchanged.

No remaining actionable source finding was reported in this bounded correction.
No runtime/product behavior changed between passing preliminary source5cb553e
and the final strengthened test source3a12bcb. Native results are recorded
separately in the [execution record](m8-macos-endpoint-lifetime-20261113.md).

## Limits

This remains finite userspace revalidation, not atomic per-byte attribution
against deliberate accepted-FD forwarding or exec immediately after a last
check. The SDK probe is not evidence that actual Syncthing performs those
fixture transitions; actual Syncthing/Desktop regressions are separate tests.
The previous owner-Mac receipt is preserved for its original exact source and
is not relabelled as execution of this new probe. No new owner rerun is requested
here; final accepted product paths/platforms still require their own review.
