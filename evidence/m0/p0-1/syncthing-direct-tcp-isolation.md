# M0 P0-1 — Syncthing Direct-TCP Transport Isolation Evidence

**Status:** IN PROGRESS  
**Scope:** P0-1 benchmark transport isolation only  
**Architecture effect:** none; this does not select a production storage layout or complete P0-1/P0-4.

## Reason for isolation

The first isolated Windows P0-1 Syncthing launch successfully bound the benchmark GUI and TCP listener, but its log also showed global discovery, local discovery, and NAT-related mechanisms enabled. The process was stopped before any benchmark folder existed or any sync run started. That launch is not treated as benchmark transport evidence.

For the controlled P0-1 transport measurements, both isolated benchmark profiles are configured to use only the explicit TCP listener and later explicit peer LAN addresses.

## Windows physical-machine verification

Repository-controlled helper:

```text
tools/m0-storage-bench/scripts/configure-syncthing-p0-1.mjs
```

Observed output on the physical Windows machine:

```text
home=C:\Users\Tarik\AppData\Local\Cryptex-Dev\syncthing-p0-1
config=C:\Users\Tarik\AppData\Local\Cryptex-Dev\syncthing-p0-1\config.xml
backup=C:\Users\Tarik\AppData\Local\Cryptex-Dev\syncthing-p0-1\config.xml.pre-p0-1.bak
previous_gui=127.0.0.1:18384
gui=127.0.0.1:18384
previous_listeners=tcp://0.0.0.0:22001
listen_address=tcp://0.0.0.0:22001
previous_globalAnnounceEnabled=true
globalAnnounceEnabled=false
previous_localAnnounceEnabled=true
localAnnounceEnabled=false
previous_relaysEnabled=true
relaysEnabled=false
previous_natEnabled=true
natEnabled=false
active_folder_count=0
transport_scope=direct_tcp_only
scope=isolated_p0_1_syncthing_config_only
```

## macOS physical-machine verification

Observed output on the physical macOS machine:

```text
home=/Users/tarikgungor/Library/Application Support/Cryptex-Dev/syncthing-p0-1
config=/Users/tarikgungor/Library/Application Support/Cryptex-Dev/syncthing-p0-1/config.xml
backup=/Users/tarikgungor/Library/Application Support/Cryptex-Dev/syncthing-p0-1/config.xml.pre-p0-1.bak
previous_gui=127.0.0.1:18384
gui=127.0.0.1:18384
previous_listeners=tcp://0.0.0.0:22001
listen_address=tcp://0.0.0.0:22001
previous_globalAnnounceEnabled=true
globalAnnounceEnabled=false
previous_localAnnounceEnabled=true
localAnnounceEnabled=false
previous_relaysEnabled=true
relaysEnabled=false
previous_natEnabled=true
natEnabled=false
active_folder_count=0
transport_scope=direct_tcp_only
scope=isolated_p0_1_syncthing_config_only
```

## Verified isolated transport configuration

Both physical benchmark machines have the following verified transport configuration:

```text
gui_address=127.0.0.1:18384
listen_address=tcp://0.0.0.0:22001
globalAnnounceEnabled=false
localAnnounceEnabled=false
relaysEnabled=false
natEnabled=false
active_folder_count=0
transport_scope=direct_tcp_only
```

The existing/default macOS Syncthing profile is separate from the Cryptex-Dev benchmark home and is outside the repository helper's permitted config path.

Direct LAN addresses already selected for the benchmark path:

```text
macos_sync_address=tcp://192.168.50.104:22001
windows_sync_address=tcp://192.168.50.137:22001
```

These LAN addresses are environment evidence for this run only, not product defaults.

## macOS first-launch auto-upgrade incident

The first macOS launch after transport isolation did not proceed to pairing or sync. The isolated profile started with no active folders, then Syncthing v2.1.2 reported an available v2.1.5 upgrade, performed an automatic upgrade, and exited:

```text
2026-09-29 22:22:52 INF syncthing v2.1.2 ...
2026-09-29 22:22:53 INF Upgrade available (current=v2.1.2 latest=v2.1.5 ...)
2026-09-29 22:22:54 INF Upgraded, should exit now (newVersion=v2.1.5 ...)
```

This launch is not benchmark transport evidence. No benchmark folder existed, no peer pairing had been configured, and no sync run started.

The macOS launcher referenced the Syncthing executable inside `/Applications/Syncthing.app`. Therefore the on-disk application binary may have been updated even though the benchmark config home was isolated. The existing/default macOS Syncthing configuration was not modified by the repository helper, but the application binary version must be re-verified before continuing.

Repository remediation has been added but is not yet physical-machine verified:

```text
autoUpgradeIntervalH=0
releasesURL=<blank>
launcher flag=--no-upgrade
```

The same upgrade guards are required on Windows for deterministic benchmark execution.

## Next evidence

- pull and apply the upgraded-isolation config on both benchmark homes
- verify `autoUpgradeIntervalH=0` and blank `releasesURL` on both physical machines
- verify the macOS application binary version after the observed auto-upgrade
- verify the existing/default macOS Syncthing process/config remains separate; do not claim its runtime health without a direct check
- relaunch only after the upgrade guards are physically verified
- verify no discovery/relay/NAT/upgrade mechanisms are active for the benchmark profiles
- verify GUI `127.0.0.1:18384` and TCP listener `22001` are live on both machines
- verify bidirectional direct TCP reachability over the selected LAN addresses
- pair only the two benchmark device identities using explicit TCP addresses
- do not configure a benchmark folder until transport pairing is verified
