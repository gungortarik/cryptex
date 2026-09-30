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

The macOS launcher referenced the Syncthing executable inside `/Applications/Syncthing.app`, so the shared application binary was affected by the upgrade even though the benchmark config home was isolated.

Physical re-verification after the incident showed:

```text
syncthing v2.1.5 "Hafnium Hornet" (go1.27.1 darwin-arm64) builder@github.syncthing.net 2026-09-08 06:57:55 UTC
```

The process list at the same check showed two Syncthing processes with the normal/default application command line and no P0-1 `--home` argument:

```text
1663 /Applications/Syncthing.app/Contents/Resources/syncthing/syncthing --no-browser --no-restart --logfile=default
2238 /Applications/Syncthing.app/Contents/Resources/syncthing/syncthing --no-browser --no-restart --logfile=default
```

These processes are not treated as P0-1 benchmark instances. Their personal/default runtime health has not been independently verified and they have not been stopped or modified by the benchmark procedure.

## Upgrade isolation remediation

The repository-controlled config helper and launchers now require upgrade isolation for P0-1:

```text
autoUpgradeIntervalH=0
releasesURL=<blank>
launcher flag=--no-upgrade
```

The macOS physical benchmark config was re-run with the remediation and verified as:

```text
previous_autoUpgradeIntervalH=12
autoUpgradeIntervalH=0
previous_releasesURL=https://upgrades.syncthing.net/meta.json
releasesURL=
active_folder_count=0
transport_scope=direct_tcp_only
upgrade_policy=disabled
scope=isolated_p0_1_syncthing_config_only
```

The Windows physical benchmark config was then re-run with the same remediation and verified as:

```text
previous_globalAnnounceEnabled=false
globalAnnounceEnabled=false
previous_localAnnounceEnabled=false
localAnnounceEnabled=false
previous_relaysEnabled=false
relaysEnabled=false
previous_natEnabled=false
natEnabled=false
previous_autoUpgradeIntervalH=12
autoUpgradeIntervalH=0
previous_releasesURL=https://upgrades.syncthing.net/meta.json
releasesURL=
active_folder_count=0
transport_scope=direct_tcp_only
upgrade_policy=disabled
scope=isolated_p0_1_syncthing_config_only
```

Both isolated benchmark configs now physically verify the same direct-TCP-only and upgrade-disabled policy.

## Windows isolated relaunch verification

The repository-controlled Windows launcher was then run on the physical Windows benchmark machine after the upgrade and transport guards were applied.

Observed launcher state:

```text
scope=isolated_p0_1_syncthing_only
home=C:\Users\Tarik\AppData\Local\Cryptex-Dev\syncthing-p0-1
gui=127.0.0.1:18384
listen_address=tcp://0.0.0.0:22001
global_announce=false
local_announce=false
relays=false
nat=false
auto_upgrade_interval_h=0
releases_url=blank
launch_no_upgrade=true
transport_scope=direct_tcp_only
active_folder_count=0
starting_foreground=true
```

Observed startup log included:

```text
2026-09-29 22:34:47 INF syncthing v2.1.5 "Hafnium Hornet" (go1.27.1 windows-amd64) ...
2026-09-29 22:34:47 INF No automatic upgrades; STNOUPGRADE environment variable defined
2026-09-29 22:34:47 INF GUI and API listening (address=127.0.0.1:18384 ...)
2026-09-29 22:34:47 INF TCP listener starting (address="[::]:22001" ...)
```

The supplied startup excerpt contained no global-discovery, local-discovery, relay, NAT, or upgrade-attempt activation lines. The isolated Windows process remained running in the foreground with zero active benchmark folders. No peer pairing or sync had started at this point.

## Next evidence

- relaunch the macOS isolated P0-1 Syncthing instance with the repository-controlled launcher and verify the same guarded startup behavior
- verify GUI `127.0.0.1:18384` and TCP listener `22001` are live on both machines
- verify bidirectional direct TCP reachability over the selected LAN addresses
- leave the existing/default macOS Syncthing processes/config untouched; do not claim their runtime health without a direct check
- pair only the two benchmark device identities using explicit TCP addresses
- do not configure a benchmark folder until transport pairing is verified
