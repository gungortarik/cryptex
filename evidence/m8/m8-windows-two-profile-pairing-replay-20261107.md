# Windows two-profile pairing and replay — 2026-11-07

## Result

Owner-operated native Windows pilot on application build `19d94b1`.
One Windows VM hosted two isolated temporary runtime profiles, opened in turn.
Both roles completed the same four-message signed exchange and received the
Desktop message “Pairing recorded in canonical local history.”

The owner read both journals and supplied the PowerShell output showing four
messages and state `anchored` on each profile. The exact saved transcript was
then submitted again on each reopened profile through **Resume a signed
pairing**. Both completion calls reported success.

| Profile | Event files unchanged after reopen/replay | Journal state | Append attempts |
| --- | --- | --- | --- |
| Initiator | `True` | `anchored` | 1 |
| Responder | `True` | `anchored` | 1 |

## Method and provenance

Before opening each profile for the replay test, the owner saved its canonical
event-file paths and SHA-256 hashes outside the runtime. The comparison excluded
lock files and the derived `writer_tip.json`. After replay, PowerShell read the
same file set and compared path/hash pairs using `Compare-Object`. It separately
read the matching transaction state and `append_attempts.Count` from
`bootstrap/pending.json`. The owner supplied the before/after command outputs
and Desktop screenshots in the engineering chat.

This evidence supports unchanged canonical event files and a single journaled
append attempt across successful replay for both pilot roles. The application
completed through the normal authenticated Desktop-to-daemon route.

## Observed limitations and next work

The responder initially had only `writer.lock` in its local stream directory.
Completion had stopped at `transcript_ready` with zero append attempts. After
a public synthetic test note was saved by the daemon, pairing completion
succeeded. Source inspection identified the trust-history scan's empty-local-
history rejection. Empty-profile first pairing still requires an implementation
review and regression evidence.

A lingering daemon after Desktop closure also delayed package switching.
The scoped daemon stop-file request allowed a normal stop. Process checks then
confirmed the intended application and sidecar paths before further testing.

Next prepare native approved-peer restriction review at a nonzero exact cutoff.
The reviewing profile must have the peer's verified canonical event through a
supported daemon import/replication path before the cutoff can be selected.
Production replication, independent-device operation, append-unknown crash
recovery, macOS parity, and independent security review remain open.

Raw signed messages, device identifiers, runtime directories, and memory bodies
are retained outside Git.
