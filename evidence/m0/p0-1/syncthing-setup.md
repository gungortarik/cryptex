# M0 P0-1 — Syncthing Benchmark Setup Evidence

**Status:** IN PROGRESS  
**Scope:** P0-1 benchmark transport setup only  
**Architecture effect:** none; this file does not select a production storage layout or complete P0-1/P0-4.

## macOS benchmark identity

Verified on the physical macOS machine.

```text
syncthing_binary=/Applications/Syncthing.app/Contents/Resources/syncthing/syncthing
syncthing_version=syncthing v2.1.2 "Hafnium Hornet" (go1.26.5 darwin-arm64) builder@github.syncthing.net 2026-06-26 05:22:40 UTC
isolated_home=/Users/tarikgungor/Library/Application Support/Cryptex-Dev/syncthing-p0-1
device_id=MHS264T-3YLCW4F-GL35TOB-DXZ6SUX-4MVY7YK-AZZFNZI-XFOXHN3-DYMMEQM
```

Observed setup command sequence:

```bash
ST="/Applications/Syncthing.app/Contents/Resources/syncthing/syncthing"
STHOME="$HOME/Library/Application Support/Cryptex-Dev/syncthing-p0-1"
mkdir -p "$STHOME"
"$ST" generate --home="$STHOME"
"$ST" device-id --home="$STHOME"
```

The macOS benchmark identity is isolated under the Cryptex-Dev runtime root. No global Syncthing profile is used by this setup.

## Windows benchmark identity

Verified on the physical Windows machine.

```text
syncthing_binary=syncthing (resolved from PATH)
syncthing_version=syncthing v2.1.5 "Hafnium Hornet" (go1.27.1 windows-amd64) builder@github.syncthing.net 2026-09-08 06:57:55 UTC
isolated_home=C:\Users\Tarik\AppData\Local\Cryptex-Dev\syncthing-p0-1
device_id=G3IQV46-V6WYJ2U-WDLQQOC-FXMJBGA-QRIXPNV-GVARBCC-IHYTOBI-BOIG6QA
```

Observed setup command sequence:

```powershell
$STHOME="$env:LOCALAPPDATA\Cryptex-Dev\syncthing-p0-1"
New-Item -ItemType Directory -Force -Path $STHOME | Out-Null
syncthing generate --home="$STHOME"
syncthing device-id --home="$STHOME"
```

The generated Windows device ID was observed in the `generate` output. The benchmark identity is isolated under the Cryptex-Dev runtime root. No global Syncthing profile is used by this setup.

## Pre-pairing safety check

Both generated isolated configs contain the generated empty folder-template element (`id=""`, `label=""`, `path=""`) inside the `<defaults>` section. This is a defaults template, not an active benchmark folder. No Cryptex benchmark folder has been configured yet.

Windows process check showed no running `syncthing` process.

After a macOS restart, a separate Syncthing application process was observed running as:

```text
/Applications/Syncthing.app/Contents/Resources/syncthing/syncthing --no-browser --no-restart --logfile=default
```

The standard macOS Syncthing config also exists at:

```text
/Users/tarikgungor/Library/Application Support/Syncthing/config.xml
```

The isolated P0-1 config is separately located at:

```text
/Users/tarikgungor/Library/Application Support/Cryptex-Dev/syncthing-p0-1/config.xml
```

The observed running process has no P0-1 `--home` argument, while the benchmark identity/config is under the separate Cryptex-Dev home. This supports treating the running application process as the existing/default Syncthing profile, not the P0-1 benchmark instance. The existing process has not been stopped or modified.

On macOS, the following candidate benchmark listener ports were checked with `lsof` and produced no listening process:

```text
gui_candidate=127.0.0.1:18384
sync_candidate=tcp://0.0.0.0:22001
observed_status=free_at_check_time
```

On Windows, the same candidate ports were checked with `Get-NetTCPConnection -State Listen` and also produced no listening process:

```text
gui_candidate=127.0.0.1:18384
sync_candidate=tcp://0.0.0.0:22001
observed_status=free_at_check_time
```

The candidate ports are therefore free on both physical benchmark machines at the observed check times.

## Generated listener defaults before benchmark reconfiguration

Windows isolated profile:

```text
gui_address=127.0.0.1:8384
listen_address=default
```

macOS isolated profile:

```text
gui_address=127.0.0.1:52743
listen_address=tcp://0.0.0.0:52746
listen_address=dynamic+https://relays.syncthing.net/endpoint
listen_address=quic://0.0.0.0:52746
```

These are generated/default isolated-profile values only. They are evidence of the pre-benchmark state and are not accepted benchmark listener settings.

## macOS benchmark listener configuration

The repository-controlled helper `tools/m0-storage-bench/scripts/configure-syncthing-p0-1.mjs` was run against the isolated macOS P0-1 home. It created a pre-change backup, found no active folders, and rewrote only the isolated benchmark config.

Observed output:

```text
home=/Users/tarikgungor/Library/Application Support/Cryptex-Dev/syncthing-p0-1
config=/Users/tarikgungor/Library/Application Support/Cryptex-Dev/syncthing-p0-1/config.xml
backup=/Users/tarikgungor/Library/Application Support/Cryptex-Dev/syncthing-p0-1/config.xml.pre-p0-1.bak
previous_gui=127.0.0.1:52743
gui=127.0.0.1:18384
previous_listeners=tcp://0.0.0.0:52746,dynamic+https://relays.syncthing.net/endpoint,quic://0.0.0.0:52746
listen_address=tcp://0.0.0.0:22001
active_folder_count=0
scope=isolated_p0_1_syncthing_config_only
```

This verifies the macOS isolated benchmark profile is now configured for GUI `127.0.0.1:18384` and a single TCP sync listener `tcp://0.0.0.0:22001`. The existing/default macOS Syncthing profile was outside the helper's permitted path and was not modified.

## Windows benchmark listener configuration

The same repository-controlled helper was run against the isolated Windows P0-1 home. It created a pre-change backup, found no active folders, and rewrote only the isolated benchmark config.

Observed output:

```text
home=C:\Users\Tarik\AppData\Local\Cryptex-Dev\syncthing-p0-1
config=C:\Users\Tarik\AppData\Local\Cryptex-Dev\syncthing-p0-1\config.xml
backup=C:\Users\Tarik\AppData\Local\Cryptex-Dev\syncthing-p0-1\config.xml.pre-p0-1.bak
previous_gui=127.0.0.1:8384
gui=127.0.0.1:18384
previous_listeners=default
listen_address=tcp://0.0.0.0:22001
active_folder_count=0
scope=isolated_p0_1_syncthing_config_only
```

This verifies the Windows isolated benchmark profile is now configured for GUI `127.0.0.1:18384` and a single TCP sync listener `tcp://0.0.0.0:22001`.

## Final read-only config verification

After both repository-controlled configuration runs, each physical machine was checked again without modifying the config.

Windows observed:

```text
gui_address=127.0.0.1:18384
listen_address=tcp://0.0.0.0:22001
active_folder_count=0
```

macOS observed:

```text
gui_address=127.0.0.1:18384
listen_address=tcp://0.0.0.0:22001
active_folder_count=0
```

Both isolated benchmark profiles therefore match the intended P0-1 listener setup and still contain no active benchmark folders. No sync transport run has started from this setup evidence.

## Version note

The macOS benchmark binary is Syncthing v2.1.2 and the Windows benchmark binary is Syncthing v2.1.5. P0-1 preserves the exact versions used as evidence. Version pinning and managed product packaging are P0-4 concerns and are not inferred from this setup record.

## Next evidence

Both isolated device identities, candidate listener ports, benchmark listener configurations, and final read-only config checks are verified. Next steps:

- record the direct LAN addresses that will be used for benchmark device pairing
- leave the existing/default macOS Syncthing profile untouched
- pair only the benchmark devices using the isolated identities and direct TCP listener
- use only synthetic benchmark folders under Cryptex-Dev
- record initial scan/sync timing for files and segments
- record incremental workload definition and sync timing
- preserve version/platform/folder metadata
- do not treat transport success as Cryptex semantic acceptance
