# Windows Tauri release build — 2026-10-03

**Host:** Windows 11 Pro, build 26300, x86_64  
**Source:** `11c65a8ba56696b7d6d200bf0fde56608299fadb`  
**Build command:** `pnpm --filter @cryptex/desktop tauri build --no-bundle`

## Result

The Tauri CLI ran the configured production frontend build and completed the
optimized Rust release build. It produced:

```text
D:\github_repos\cryptex\target\release\cryptex-desktop.exe
```

| Artifact | Size | SHA-256 |
|---|---:|---|
| `cryptex-desktop.exe` | 8,747,520 bytes | `A6E53E78441767697075BF95E8DDD8AF83DD4B51D28D5BDD261470200FC1A586` |

The release executable loaded its embedded renderer at `http://tauri.localhost/`
and passed the Windows Tauri renderer IPC health/replacement flow. Exact UI
observations are in
[`windows-tauri-renderer-ipc-reconnect-20261003.md`](p0-2/windows-tauri-renderer-ipc-reconnect-20261003.md).

`--no-bundle` builds the executable without an installer. Installer packaging
and signing remain open; this does not accept ADR-0012 or complete M0.
