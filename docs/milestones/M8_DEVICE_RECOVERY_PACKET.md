# Existing-device queued-work interruption check

Scope: the owner's November 17 request for a largely automatic, one-click
interruption/recovery test on the existing Mac and Windows VM. Not M8 closeout.

The ordinary application does not gain a new renderer capability. A separate
Desktop build selects `m8-device-recovery-check`, off by default, and accepts one
closed native command with a run UUID and expected Brain/device UUIDs. It accepts
no note body, filesystem override, PID, shell command, helper control or trust
override. Mac additionally requires the existing selected development product,
owner certificate, protected content and native ExistingProfile binding.
Windows retains the normal authenticated Desktop pipe and sibling daemon launch.
`cryptexd` remains the only canonical writer; the existing capture, resolve,
guarded search and connection-setting routes do all work.

## Sequence

1. Normal applications must already be closed. Verify the established signed
   ten-record/two-device canonical set against the accepted November 17 aggregate.
   Changed data, identity, unfinished imports or an active instance stop the run.
2. Build/install beside the old application. Keep existing launchers and keys.
   Each device starts its own exact retained daemon child. Require one expected
   trusted peer and saved LAN endpoints. Either initial pause preference is valid.
3. Pause transport through normal settings, commit one fixed Public run marker,
   and verify it through guarded local search. Re-enable the normal worker. On
   Windows, abruptly terminate only the retained daemon child. An already dead
   child or failed termination is not successful fault injection. On Mac, a
   retained Desktop child exits abruptly without running Rust drops; its daemon
   observes native owner loss and performs ordinary helper cleanup. The outer
   collector stays alive throughout cleanup and never kills/adopts that daemon.
4. Allow the normal owner-loss cleanup budget, then perform fresh ordinary
   authenticated startup. Never delete active markers, force-unlock, adopt an
   orphan, repair canonical data or change OS security settings.
5. Receive and search the peer's fixed run marker. Each daemon then commits one
   fixed Public acknowledgement marker. Search the peer acknowledgement, allow
   delivery grace, pause, close normally and restart offline. Search all four
   markers again. Restore the original pause preference and close normally.
6. Each stopped daemon independently verifies its final canonical set. Acceptance
   requires both run/source-bound reports, the full ordered stage receipts,
   successful cleanup, fourteen signed records from two devices, and equal final
   aggregates. One successful side is insufficient. A divergent or delayed peer
   produces incomplete evidence, never a remote durability claim.

The two notes per device are ordinary persistent Public test memory and remain
after the test. No existing note is edited or removed. A capture timeout is
resolved through the existing durable receipt; it is never blindly appended
again. This first packet deliberately refuses a changed baseline on rerun.
Failure receipts must be inspected before another attempt; no automatic repair.

## Exact evidentiary boundary

The note was definitely queued while disconnected before the transport restart.
The driver interrupts an owned process after the local worker reports ready.
The Mac report explicitly names Desktop loss; Windows names daemon loss. These
are not interchangeable. Abrupt Mac daemon loss would retain `transport.active`:
only the original live daemon owns the helper child/group and can prove cleanup.
ADR-0021's replacement refusal remains intact. Automatic recovery from that
stronger failure is neither claimed nor added.
It does **not** observe an unfinished network byte at the exact instant of that
termination: a small note might already have arrived. Therefore this proves only
queued-work process-interruption recovery when both actual device reports pass.
`midstream_bytes_interrupted`, power-loss and remote-protection claims stay false.
Planned pause alone, ReadyLocally alone, CI on one host, screenshots, and matching
pre-existing digests cannot substitute for these two final owner reports.

CI compile/unit/runner/package readback checks are not actual device acceptance.
The default-off driver is not a public distribution or a new recovery authority.
Final M8 review must assess whether a separately observed mid-transfer network
cut remains necessary; this packet must not silently close that stronger gate.
