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

Pending. The Windows benchmark profile must be generated under:

```text
%LOCALAPPDATA%\Cryptex-Dev\syncthing-p0-1
```

The exact Windows Syncthing version and generated device ID must be recorded before pairing.

## Version note

The macOS benchmark binary is Syncthing v2.1.2. Previous Windows benchmark metadata recorded Syncthing v2.1.5. P0-1 will preserve the exact versions used as evidence. Version pinning and managed product packaging are P0-4 concerns and are not inferred from this setup record.

## Next evidence

After both isolated device identities are verified:

- pair only the benchmark devices
- use only synthetic benchmark folders under Cryptex-Dev
- record initial scan/sync timing for files and segments
- record incremental workload definition and sync timing
- preserve version/platform/folder metadata
- do not treat transport success as Cryptex semantic acceptance
