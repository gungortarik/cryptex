# M8 Windows importer and IPC source audit

**Date:** 2026-10-22  
**Branch / HEAD:** `codex/m8-device-trust` @ `50f56396997694f4e26a24d016c6eec95414c310`  
**Scope:** Static source review for the Windows-first M8 continuation. No runtime, protocol activation, or race proof.

## Findings

### Syncthing importer / canonical publication

- `services/cryptexd/src/managed_profile/native_xml.rs` (`inspect`, lines 228–261) strictly parses the pinned profile and admits exactly one local device. Its contract says no active folder/peer is admitted at this bootstrap stage; unexpected folder/peer XML is rejected by the tests around lines 376–382.
- No Syncthing import actor or canonical publication API was found in `services/cryptexd/src/managed_profile/` or the daemon IPC surface. `knowledge_materialization.rs` still identifies a not-yet-integrated sync importer as a source of uncoordinated changes.
- The canonical event writer in `services/cryptexd/src/canonical_writer.rs` appends local events under its existing writer lock and reads verified streams. It has no multi-device import transaction API matching the M8 proposal's inventory-plus-sorted-device lock contract.
- Result: there is no current Syncthing-to-canonical event path to race. The future M8 import path must stage transport data outside canonical storage, validate it in the daemon, then publish the complete accepted update while holding the required locks. Syncthing must not directly mutate canonical packs. The path and its tests are not implemented.

### Existing Windows Desktop IPC process check

- `create_listener` (`services/cryptexd/src/ipc.rs:1013–1034`) creates a local named pipe with a protected DACL granting full access to the current user SID.
- `peer_process_id` (`ipc.rs:2035–2041`) obtains the connected peer PID from `interprocess::Stream::peer_creds()`; the server checks this as each connection is accepted (`ipc.rs:598–605`).
- `desktop_process_identity` (`ipc.rs:1850–1894`) finds the daemon's parent PID, opens and retains a process handle, checks that process image path is the expected sibling Desktop executable, and later requires the connection PID to match while the retained process handle is still live (`ipc.rs:567–584`).
- This rejects an unrelated same-user process that merely connects to the pipe while the expected parent Desktop process is alive. It does not by itself establish the M8 owner-approval contract: the design still needs to define what OS identity evidence is captured and bound to the approval connection, executable replacement/reuse assumptions, daemon restart behavior, and adversarial process-replacement tests. The static review does not claim native Windows verification.

## Next implementation boundary

1. Keep canonical event storage daemon-owned; transport sync is not authority.
2. Design the incoming path as untrusted staging followed by daemon validation and atomic publication under inventory and affected-device locks.
3. Specify the M8 approval IPC credential contract for Windows and macOS, including connection binding and process replacement/reuse behavior.
4. Implement and test only after the protocol/security candidate passes its required independent review and explicit acceptance gates.

No code, tests, trust state, Actions, or runtime data changed by this audit.
