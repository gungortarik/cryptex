# M0 P0-1 — Syncthing Direct-TCP Reachability Evidence

**Status:** VERIFIED REACHABILITY EVIDENCE  
**Scope:** P0-1 benchmark transport reachability only  
**Architecture effect:** none; this does not select a production storage layout or complete P0-1/P0-4.

## Preconditions

Both isolated P0-1 Syncthing instances were running in the foreground with the repository-controlled launchers and the following verified configuration:

```text
gui_address=127.0.0.1:18384
listen_address=tcp://0.0.0.0:22001
globalAnnounceEnabled=false
localAnnounceEnabled=false
relaysEnabled=false
natEnabled=false
autoUpgradeIntervalH=0
releasesURL=<blank>
launcher_no_upgrade=true
active_folder_count=0
transport_scope=direct_tcp_only
```

Selected LAN addresses for this physical test run:

```text
macos_lan_ipv4=192.168.50.104
windows_lan_ipv4=192.168.50.137
```

These addresses are run-specific environment evidence and are not product defaults.

## macOS to Windows

Command executed on the physical macOS machine while the isolated Windows benchmark instance was listening on TCP port 22001:

```bash
nc -vz 192.168.50.137 22001
```

Observed result: the connection test reported success.

```text
macos_to_windows_tcp_22001=SUCCEEDED
```

## Windows to macOS

Command executed on the physical Windows machine while the isolated macOS benchmark instance was listening on TCP port 22001:

```powershell
Test-NetConnection 192.168.50.104 -Port 22001
```

Observed result:

```text
TcpTestSucceeded : True
```

Normalized evidence:

```text
windows_to_macos_tcp_22001=true
```

## Result

Bidirectional direct TCP reachability on the selected LAN path is verified while discovery, relay and NAT mechanisms are disabled for the isolated P0-1 profiles.

No benchmark folder had been configured and no file synchronization run had started when this reachability evidence was collected.

## Next evidence

- add only the two isolated benchmark device identities to each other using explicit TCP addresses
- verify the peer connection is established using the explicit LAN addresses
- keep active benchmark folder count at zero until device pairing is verified
- then configure synthetic benchmark folders under Cryptex-Dev and measure initial and incremental sync behavior
