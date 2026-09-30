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

## Version note

The macOS benchmark binary is Syncthing v2.1.2 and the Windows benchmark binary is Syncthing v2.1.5. P0-1 preserves the exact versions used as evidence. Version pinning and managed product packaging are P0-4 concerns and are not inferred from this setup record.

## Next evidence

Both isolated device identities are now verified. Next steps:

- pair only the benchmark devices
- use only synthetic benchmark folders under Cryptex-Dev
- record initial scan/sync timing for files and segments
- record incremental workload definition and sync timing
- preserve version/platform/folder metadata
- do not treat transport success as Cryptex semantic acceptance
