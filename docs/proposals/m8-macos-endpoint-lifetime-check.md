# Disabled Mac endpoint/generation lifetime checks

Scope: the existing ADR-0021 default-off transport adapter, without a new runtime
role, renderer capability, publisher/OS-floor decision or product activation.
The combined-loss owner gate is already accepted and is not repeated here.

## Two bounded findings

1. Socket discovery double-read its FD before retaining the serving task. There
   was no post-retention tie between that retained generation and the original
   FD/socket object. Keep the exact FD, reverse tuple, socket/PCB identities and
   generation count; revalidate that same witness after retention. A changed
   witness is terminal `PeerChanged`, not retryable accept-installation delay.
2. HTTP held a passive guard. Revalidate retained root/server generation, image
   and original socket immediately before credential-bearing send, then recheck
   the existing absolute deadline. Before publishing a fully consumed
   observation, require the retained root/server generations and images again.

Normal HTTP Connection:close may remove the server FD before the body is fully
consumed; post-response validation must therefore check generations/images,
not require an open server FD. Explicit Shutdown ACK conveys no live identity
and retains its existing separate exact process-tree exit requirement. The
existing ordinary Windows guard policy is unchanged. No reconnect or new peer
is permitted to replace failed evidence on this request.

This is finite userspace revalidation, not atomic per-byte attribution. It does
not eliminate a malicious trusted process deliberately transferring an accepted
FD or executing immediately after the last check. Do not claim that stronger
property. SDK contract evidence is separate from actual Syncthing behavior.

## Fixed evidence inventory

The standalone signed SDK probe includes the actual C adapter. Its deterministic
libproc interception exists only in that test translation unit, never in Cargo
or the daemon. It uses captured same-image children in its own reserved session,
synthetic bytes, fixed controls and no OS-store credentials or canonical data.

| Native case | Required result |
| --- | --- |
| IPv4 normal close | Coherent exact endpoint; complete body; live generation accepted after FD closure |
| IPv6 normal close | Same result for the other loopback family |
| Listener only | Refuse before application bytes |
| Discovery FD reuse | Child replaces exact FD after the second snapshot, retains original as another FD; return terminal changed-witness refusal, no bytes |
| Pre-send same-PID exec | Retain fixture socket across exec; old guard refuses, no bytes |
| Queued body then exec | Complete valid HTTP bytes do not authorize an old generation |
| Root loss with live server | Refuse despite live retained serving child/socket |
| Shutdown then immediate exit | ACK is not live identity; exact captured child exits cleanly |

Private Rust boundary tests exercise the same connected HTTP implementation:
typed guard errors emit no HTTP bytes, queued valid bodies cannot authorize lost
generations, normal close works, Shutdown ACK remains distinct, and synchronous
checks do not reset an expired deadline. The latter invokes the shared private
pre-send boundary directly; it has no scheduler-dependent short sleep.

The original real Syncthing, native Desktop pairing, restart/replay, single and
combined loss, eleven mutation cases and exported launchers must still pass on
the exact candidate. Require native Mac/Windows CI and independent source review
before claiming acceptance. Do not deliver an untested owner package.
