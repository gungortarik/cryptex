# M8 actual native worker transfer pilot — 2026-11-08

Status: harness/package preparation; host-to-VM native receipt pending.

## Scope and boundaries

Owner chose the main Windows computer and existing Windows VM. Fresh isolated
profiles only. The host is an **untrusted relay**, not an owner enrollment or a
second canonical writer: the VM creates synthetic source/receiver canonical
identities and genuinely signs reciprocal approval records using test-store
keys. Public signed segments travel VM request ZIP → host send-only native
folder → actual VM native receiver incoming folder → daemon publisher.

Production `ManagedTransportWorker` and `IncomingController` publication checks
are used. A narrowly pinned peer/folder native XML exception, crash injection,
fixture provisioning and queue feeding exist only in the test build. Product
reopen still rejects extended native configuration. Numeric private IPv4 only,
loopback REST API, discovery/relay/NAT/upgrade disabled. No firewall/AV changes.

Assertions required before `result.json` can say passed:

- Real owned native REST readiness and identity, exact delivered size/SHA-256.
- No peer canonical segment present before publication.
- Acknowledged owned-job termination after delivery and before publication;
  actual next attempt, changed PID, same native identity; no publication while
  transport is restarting.
- Shared runtime gate defers incoming work. Real named-pipe search reports busy
  while publication has moved canonical bytes but has not rebuilt derived state.
- Publication through the worker bridge, unchanged existing canonical bytes,
  exact imported hashes, verified search result after commit/rebuild.
- Clean native shutdown/reopen and reacquired runtime instance, replay writes
  zero segments, all canonical `.seg`/`.open` hashes and inventory unchanged.
- Final clean native shutdown and successful deletion of synthetic API credential.

Named-pipe helper uses the existing connected-peer test seam: no separate native
Desktop authorization claim. No physical power cut or mid-network-write fault.
Private fixture directories remain intentionally. Only allowlisted receipt,
checkpoint, public relay descriptor and build/test logs enter the result ZIP.

## Local checks

- Strict all-target daemon Clippy and Rust formatting pass.
- Managed-profile selection: 57 passed, zero failed, 2 intentionally ignored
  native opt-in tests (including this new live test).
- Trusted import fixtures: 3 passed; real publication/IPC admission integration:
  1 passed. These do not substitute for the new actual native receipt.
- Windows PowerShell 5.1 runner regressions pass: script parsing, exact one-test
  summary/exit requirements, generated Windows ZIP interoperability, duplicate
  and traversal rejection, digest refusal and public/wildcard listener refusal.
- Python package generator compiles; final package exact-member/source binding
  is checked when generated.
- One actual host native preflight reached managed startup and refused
  `Controller(Native(Rest(Endpoint(PeerNotEstablished))))`. Native cleanup was
  forced on that failure. No result/pass JSON was emitted; no guard was weakened.

Independent source review found and corrected: native hello filling an empty
peer name (now fixed nonempty name plus remote-name overwrite disabled), and a
restart observation that did not acknowledge the requested crash (now explicit
post-cleanup acknowledgement and exact attempt/PID ordering). Additional runner
checks cover bounded ZIP metadata and fixed private relay endpoints. Source
review is technical agent review, not human certification or native execution.

## Exact continuation

1. Owner extracts the delivered package **inside the Windows VM**, opens
   `NATIVE-AKTAR.cmd` as a normal user and leaves its window open.
2. After build/native readiness it exports `DENETLEYICI-ISTEK-<session>-<run>.zip`
   to Downloads. Owner returns that ZIP. The VM test waits at most 30 minutes.
3. Agent validates the ZIP with `start-native-controller-relay.ps1 -ValidateOnly`,
   using the retained host session below, then runs the same script without
   `-ValidateOnly`. Keep the relay alive across receiver crash/reopen; its own
   completion receipt alone is not import success.
4. Owner returns `DENETLEYICI-SONUC-<session>-<run>.zip`, including any failure.
   Check exact source digest/commit, test summary/exit, checkpoints, JSON scopes
   and host relay receipt before recording a native pass.

Retained host session (fresh synthetic, no native listener currently started):
`C:\Users\Tarik\AppData\Local\Cryptex-Dev\native-controller-relay\d1e5c535ae0e459fb699be8f7324c149`.
Its `relay.json` binds the native ID and private endpoint on `192.168.177.1`;
`sender.json` points to the fresh retained relay profile. Do not substitute old
pairing profiles or manually alter signed segments.

## Delivered candidate binding

- Source commit: `e1b2aedcf5f94461f15e02ba49e2662bd77d62ab`.
- Source ZIP SHA-256: `a720e511f72bd93c31bb382abaa3abbd802605f375336f6956fed31bd16c7227`.
- Package: `C:\Users\Tarik\Downloads\cryptex-denetleyici-aktarim-20261108.zip`.
- Package SHA-256: `da93da58712220cf5f0bd202360fcf7c158ca3b6da654f80326f48e9c3870a40`.
- Size: 14,282,177 bytes. Session: `d1e5c535ae0e459fb699be8f7324c149`.
- Exact ten ZIP members verified; embedded launcher equals the candidate source;
  source digest/size bounds and pinned native executable/notices verified.
- Retained host session also contains `package-binding.json`; no sender launched.

Independent reviewer completed the final Rust/scripts source pass with no
remaining concrete correctness/safety/false-PASS finding. Reviewer ran no
executables. Final script bytes were bound before commit; subsequent changes
are documentation only. Native receiver receipt remains pending.

M8 remains open: native receipt, remaining cross-device restriction/offline gates,
macOS production publication/endpoint parity, physical durability, required CI
and human review are separate completion gates.
