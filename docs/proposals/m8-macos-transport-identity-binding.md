# Mac managed transport identity binding — proposed disabled extension

Status: **SELECTED FOR DISABLED DEVELOPMENT**, recorded in
[ADR-0021](../adr/ADR-0021-disabled-macos-transport-helper.md).
Date: 2026-11-12. This does not enable a product route.

Karar özeti: Öneri, mevcut imzalı `cryptexd` ikilisini ayrı bir taşıma yardımcısı
rolünde çalıştırmak. Yardımcı yalnızca izole Syncthing süreçlerini ve yerel API
kimliğini yönetir; kalıcı hafıza veya eşleştirme onayı yazmaz. Önce kapalı test
kapsamında süreç kaybı/temizlik ve gerçek socket sahipliği kanıtlanır. Bu yeni
süreç rolü, kullanıcının uygun seçeneği uygulama yönlendirmesiyle seçildi;
yayınlama veya üründe etkinleştirme istenmiyor.

## Concrete finding

The native Desktop pairing commands are wired and their bounded refusal checks
passed at `6ee4893`. The missing Mac identity source is not an unwired field:

- `managed_profile::worker`, its private-directory adapter, native configuration
  implementation and package/process adapters currently support Windows only.
- `owned_process::endpoint::connect_owned_loopback` explicitly refuses other OSes.
- Windows attaches the engine to a private Job at process creation and selects
  `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`. Merely starting a Mac child and adding a
  `Drop` implementation would not reproduce cleanup after daemon SIGKILL/crash.
- The accepted pinned upstream engine has a monitor and a serving child. Its
  `--no-restart` flag does not remove that monitor. The exact tagged
  [main source](https://github.com/syncthing/syncthing/blob/v2.1.5/cmd/syncthing/main.go)
  and [monitor source](https://github.com/syncthing/syncthing/blob/v2.1.5/cmd/syncthing/monitor.go)
  must govern process ownership; do not adopt a server just because its PID or
  response claims the expected identity.

The existing Mac gate must stay `transport_identity_unavailable` until actual
instance/socket ownership and the lifecycle below have passing evidence.

## Recommended process role

Reuse the **existing signed cryptexd executable** in a separate, explicit
transport-helper mode. It runs as an actual supervised child of the canonical
daemon and owns only a single isolated Syncthing monitor/server tree. This
avoids choosing a new release publisher, executable signing identity, launchd
registration policy or supported OS floor as part of identity reading.

```mermaid
flowchart LR
  UI[Desktop renderer] --> DB[Verified Desktop backend]
  DB -->|existing native owner channel| D[Canonical daemon]
  D -->|separate purpose-bound native channel| H[cryptexd transport helper]
  H --> M[Pinned Syncthing monitor]
  M --> S[Syncthing serving child]
  H -->|attest the actual connected socket before HTTP| S
```

This is a new process role and integration strategy, rather than just exposing
the Windows status cache on Mac. Repository `AGENTS.md` requires clarification
before changing an ambiguous process/workflow/integration direction. The user
selected this proposal by delegating the suitable option; ADR-0021 records the
bounded development decision before the role and launch path are added.

An OS-managed transient service is an alternative if helper supervision cannot
meet the failure criteria. That would need its own launchd/service lifecycle
decision and evidence; it is not silently selected here.

## Required boundaries

1. Keep this extension behind the existing default-off Mac development feature.
   Ordinary Unix M8 refusals, Windows behavior and canonical formats remain intact.
2. Authenticate the daemon and its exact captured helper child in both directions
   using independently verified same-release images, live task/audit generation
   pins and a fresh native channel. Distinguish helper-purpose messages from
   Desktop owner messages; a helper must never supply an owner capability.
3. The helper enters before ordinary daemon lifecycle/IPC/canonical initialization.
   It cannot open a canonical writer, event signing key, approval registry or
   bootstrap journal. Its only OS-store scope is its isolated managed transport
   API credential. Reusing executable bytes does not confer canonical authority.
4. Use a fresh private process session/group for the helper and its transport
   children. The canonical daemon retains the helper as an unreaped child until
   group cleanup finishes. The helper owns/reaps the engine children. Verify the
   actual pinned upstream fork behavior rather than assuming group membership.
5. Helper loss while the daemon survives, and daemon loss while the helper
   survives, must each invalidate reads and clean the owned group within the
   tested bound. Do not rely solely on Rust destructors or a cached PID. Cover
   combined supervisor loss explicitly: if the mechanism cannot prove cleanup,
   retain an unresolved state, refuse a replacement, and leave that acceptance
   gate open rather than claiming Windows Job equivalence.
6. Reuse managed-profile policy, immutable readiness/credential binding and
   strict XML validation through Mac platform adapters. Use descriptor-relative
   no-follow file access, owner/mode/ACL checks, link checks, exclusion and
   identity continuity checks. Never repair or regenerate an existing partial
   profile/key to make a restart work. Preserve user-global Syncthing state.
7. Package only the exact target's upstream 2.1.5 native executable and notices.
   No PATH lookup, system-install adoption, shell launcher, self-update, monitor
   bypass through internal environment variables, or modified upstream binary.
8. Connect to a backend-selected numeric loopback endpoint. Before sending an
   API key, attest the **established connection's reverse tuple**, its server FD
   and the retained serving process in our owned tree. A listener-table result,
   process name, UID or matching REST JSON is insufficient. Retain the process
   generation proof through the exact HTTP stream; no reconnect/proxy/pool path.
9. Use the public SDK declarations for `proc_pidinfo`/`proc_pidfdinfo` and socket
   structures/state constants. Their availability and ownership semantics are
   an implementation probe, not established by browsing a header. No copied
   private numeric ABI. The primary
   [Apple libproc declarations](https://github.com/apple-oss-distributions/xnu/blob/main/libsyscall/wrappers/libproc/libproc.h)
   and [socket-info declarations](https://github.com/apple-oss-distributions/xnu/blob/main/bsd/sys/proc_info.h)
   identify the candidate public seam.
10. Read pinned version and native ID over those owned sockets, then match ID to
    the private readiness/config receipt. A native identity read must perform a
    fresh bounded helper observation, not just return an earlier LocalReady cache.
    No profile path, port, expected ID, arbitrary command or API key comes from
    the renderer. Imported offers cannot choose the ownership policy.
11. Limit helper commands to bounded observation and stop, with typed diagnostics
    and absolute request/cancellation/cleanup deadlines. Peer loss must interrupt
    generation, observation and shutdown; an idle receive loop alone is not a
    parent-death watcher. Credentials, paths and raw native logs stay backend-only.
12. Feed this reader into `current_transport_device_id` for native read, offer and
    proof, preserving the existing domain checks. A lost helper/engine, changed
    package/profile/key, stale session, unowned socket or timeout must refuse.

## Implementation sequence after the role decision

1. Public-SDK endpoint/process-generation probe with real child sockets: correct
   server, foreign server, listener-only row, exit/exec/restart and PID continuity.
2. Default-off supervised helper and private transport adapters; abrupt-loss
   cleanup and exclusion tests before publishing any ready identity.
3. Exact pinned real Syncthing profile/credential provisioning and reopen,
   owned REST version/ID, native reader binding, stop and missing-key refusals.
4. Native Desktop Brain/transport reads and real offer/proof for both roles,
   review/cancel/confirm/complete, restart and exact transcript/grant replays.
5. Exact-source Mac/Windows CI, independent source review and then a single
   one-click owner Mac package. Do not send a package that can only repeat the
   currently expected unavailable-transport refusal.

## Package provenance inspected for this proposal

The official [2.1.5 release](https://github.com/syncthing/syncthing/releases/tag/v2.1.5)
archives were downloaded and their archive SHA-256 values compared to that
release's `sha256sum.txt.asc` contents. The PGP signature itself was not verified;
this is a reproducible package-pin input, not publisher authentication or Mac
execution evidence. Only the exact top-level archive members were hashed.

| Target | Archive SHA-256 | Native executable SHA-256 | Bytes |
| --- | --- | --- | ---: |
| macos-arm64 | `4a3153cce6f3bdc6290824e409b1622c3b78ad8b8a8b2fc846c0061b61e44d4a` | `6de41132c8a3fc78fc57486df32f9ac69d6f53db1e1910bb46f548ee7f29725e` | 27811792 |
| macos-amd64 | `f4535e479472a1ae43d3e6ffc4bcfc7651179e948387c08f3696847d142b62c7` | `277466c540abc5898c03b3e5e6d6349001483ba40b3ffa5e3782f5ba23f12f7e` | 29591776 |

Both Mac archives contain the same notice hashes: LICENSE.txt
`9221c2f936159b8446d329249fb4c0f25be510f447383a0f13336ac7985668a3`
(17099 bytes), AUTHORS.txt
`2c197afd6113ec13ae134a20ce8f101d9812b078c3ef32c42c3bb4cdaf2b96a2`
(17156 bytes). These are not the existing Windows notice-byte pins. Never select
an executable by basename suffix: the archive also contains non-executable
configuration examples named `syncthing` in nested directories.

## Completion bar

Preparing this proposal does not complete the requested connection. No process
role, native platform implementation, release policy or enabled route changed.
The existing passing source/CI checkpoint remains applicable only to the earlier
wiring/refusal work. Keep M8 and real native Mac pairing open until the evidence
above exists; product activation remains a separate decision under ADR-0020.
