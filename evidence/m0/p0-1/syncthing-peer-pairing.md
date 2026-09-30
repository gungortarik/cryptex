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

Pending physical execution and verification.

Expected peer:

```text
peer_id=G3IQV46-V6WYJ2U-WDLQQOC-FXMJBGA-QRIXPNV-GVARBCC-IHYTOBI-BOIG6QA
peer_name=P0-1-Windows
peer_address=tcp://192.168.50.137:22001
peer_address_mode=explicit_direct_tcp_only
auto_accept_folders=false
introducer=false
paused=false
active_folder_count=0
```

## Next evidence

- add the Windows benchmark peer to the macOS isolated benchmark profile with the same guarded helper
- verify both Syncthing instances report the intended peer connection established over the explicit direct TCP path
- do not create or share any benchmark folder until peer connection verification is complete
