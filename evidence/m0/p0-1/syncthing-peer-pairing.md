# M0 P0-1 — Syncthing Peer Pairing Evidence

**Status:** IN PROGRESS  
**Scope:** P0-1 isolated benchmark peer pairing only  
**Architecture effect:** none; this does not select a production storage layout or complete P0-1/P0-4.

## Preconditions already verified

- both isolated P0-1 Syncthing instances run from Cryptex-Dev homes
- both use Syncthing v2.1.5 for the current transport run
- GUI address is `127.0.0.1:18384`
- sync listener is `tcp://0.0.0.0:22001`
- global discovery, local discovery, relays, NAT, and automatic upgrades are disabled
- bidirectional direct TCP reachability over the selected LAN addresses was verified
- no benchmark folders exist yet

Selected pairing path:

```text
macOS device ID=MHS264T-3YLCW4F-GL35TOB-DXZ6SUX-4MVY7YK-AZZFNZI-XFOXHN3-DYMMEQM
macOS address=tcp://192.168.50.104:22001
Windows device ID=G3IQV46-V6WYJ2U-WDLQQOC-FXMJBGA-QRIXPNV-GVARBCC-IHYTOBI-BOIG6QA
Windows address=tcp://192.168.50.137:22001
```

## Windows-side peer configuration

The guarded repository helper was run on the physical Windows benchmark machine:

```text
tools/m0-storage-bench/scripts/pair-syncthing-p0-1.mjs
```

Observed verified output:

```text
home=C:\Users\Tarik\AppData\Local\Cryptex-Dev\syncthing-p0-1
local_device_id=G3IQV46-V6WYJ2U-WDLQQOC-FXMJBGA-QRIXPNV-GVARBCC-IHYTOBI-BOIG6QA
peer_id=MHS264T-3YLCW4F-GL35TOB-DXZ6SUX-4MVY7YK-AZZFNZI-XFOXHN3-DYMMEQM
peer_name=P0-1-macOS
peer_address=tcp://192.168.50.104:22001
peer_action=added
peer_address_mode=explicit_direct_tcp_only
auto_accept_folders=false
introducer=false
paused=false
active_folder_count=0
restart_required=false
scope=isolated_p0_1_peer_pairing_only
```

This verifies the Windows isolated benchmark profile now knows only the intended macOS benchmark peer at the explicit direct TCP address. No benchmark folder has been configured and no storage sync run has started.

## macOS-side peer configuration

The same guarded repository helper was then run on the physical macOS benchmark machine.

Observed verified output:

```text
home=/Users/tarikgungor/Library/Application Support/Cryptex-Dev/syncthing-p0-1
local_device_id=MHS264T-3YLCW4F-GL35TOB-DXZ6SUX-4MVY7YK-AZZFNZI-XFOXHN3-DYMMEQM
peer_id=G3IQV46-V6WYJ2U-WDLQQOC-FXMJBGA-QRIXPNV-GVARBCC-IHYTOBI-BOIG6QA
peer_name=P0-1-Windows
peer_address=tcp://192.168.50.137:22001
peer_action=added
peer_address_mode=explicit_direct_tcp_only
auto_accept_folders=false
introducer=false
paused=false
active_folder_count=0
restart_required=false
scope=isolated_p0_1_peer_pairing_only
```

This verifies the macOS isolated benchmark profile now knows only the intended Windows benchmark peer at the explicit direct TCP address. Both sides are configured symmetrically, no benchmark folders exist, and no storage sync run has started.

## Pairing checkpoint

Pair configuration is now complete on both physical benchmark machines. Runtime connection establishment still requires explicit verification on both sides before any benchmark folder is created or shared.

## macOS runtime peer connection verification

The repository verifier was run against the live macOS isolated benchmark instance:

```text
tools/m0-storage-bench/scripts/verify-syncthing-p0-1-peer-connection.mjs
```

Observed verified output:

```text
home=/Users/tarikgungor/Library/Application Support/Cryptex-Dev/syncthing-p0-1
local_device_id=MHS264T-3YLCW4F-GL35TOB-DXZ6SUX-4MVY7YK-AZZFNZI-XFOXHN3-DYMMEQM
peer_id=G3IQV46-V6WYJ2U-WDLQQOC-FXMJBGA-QRIXPNV-GVARBCC-IHYTOBI-BOIG6QA
configured_peer_address=tcp://192.168.50.137:22001
peer_address_mode=explicit_direct_tcp_only
runtime_connected=true
runtime_type=tcp-client
runtime_address=192.168.50.137:22001
runtime_client_version=v2.1.5
runtime_peer_ip_matches=true
runtime_is_local=true
active_folder_count=0
scope=isolated_p0_1_peer_connection_verification_only
```

This physically verifies the macOS benchmark peer connection is established to the intended Windows benchmark device over the explicit direct TCP LAN path. No benchmark folder exists and no storage sync run has started.

## Windows runtime peer connection verification

Pending physical execution and verification.

## Next evidence

- verify the Windows isolated Syncthing instance reports the intended macOS peer connection as established at runtime
- verify the Windows runtime connection type is TCP and the observed remote address belongs to the selected macOS LAN address
- do not create or share any benchmark folder until the Windows-side runtime connection verification also passes
