# Windows M0 release build

Date: 2026-10-02 EDT. Source commit: `51fc063f181d814534badf4f6f3def1de101943e`.

## Build result

The following commands completed successfully on the physical Windows host:

```powershell
pnpm --filter @cryptex/desktop tauri build --no-bundle
cargo build --workspace --release --locked
pnpm --filter @cryptex/desktop tauri build --no-bundle
```

The final Tauri invocation produces the Desktop executable with the embedded
frontend after the plain workspace build. TypeScript and the Vite production
build passed. All five workspace executables were built in the release profile.

| Artifact under `target/release` | Bytes | SHA-256 |
|---|---:|---|
| `cryptex-desktop.exe` | 8652288 | `974045534113AB0B7A65A9B5642DBDF82A25F9C9121DDB41EACC58E5C9B54676` |
| `cryptexd.exe` | 370176 | `8A43D226562C2D928F831718F7FC8F0E5B14E1FC638F60ECB108290A00A7DC9F` |
| `m0-ipc-spike.exe` | 348672 | `4D452A2578C908FEE2B32A2220DA8315330DFEEDAFC831288594091AD277AFA1` |
| `m0-lifecycle-compare.exe` | 471040 | `1D8F469569781A3FE239C99AB12ED60718E85748C23E7122B86905A9F58815AA` |
| `m0-storage-bench.exe` | 252928 | `2360319B5CEF4EB4861C025DE759EB18A1F41BD903A8F024B0ADD24701FD0AA1` |

## Release runtime checks

The release `cryptexd.exe` foundation command exited successfully and printed
`cryptexd M0 foundation`.

The Windows process-level IPC script passed against the release binary:

```powershell
powershell.exe -NoProfile -NonInteractive -File tools/m0-ipc-spike/scripts/test-windows-ipc-spike.ps1 -Binary D:\github_repos\cryptex\target\release\m0-ipc-spike.exe
```

Runtime evidence was retained under
`%LOCALAPPDATA%\Cryptex-Dev\p0-3\ipc-windows-20261003T004952260Z`.
Role/version/capability checks, malformed/oversized rejection and actual
process-restart reconnect passed. The reconnect reached the new server PID
`34228`. Second-account denial remains untested; the user currently has no
accessible second Windows account.

## CI and remaining gates

The IPC spike test was added to both CI platform jobs. CI was dispatched for
the source commit: [CI run](https://github.com/gungortarik/cryptex/actions/runs/37083546094).

macOS CI passed. Windows CI found one `needless_borrows_for_generic_args`
warning in the Windows-only IPC security-descriptor construction. The redundant
borrow was removed without changing the descriptor. The full local Windows
workspace then passed `cargo clippy --locked --workspace --all-targets -- -D warnings`.
Combined-source CI revalidation is pending.

These outputs are M0 foundation executables. Installer bundling is disabled in
the repository configuration. This build does not accept ADR-0012/0007 or
complete the unresolved Windows login, cross-account ACL, Desktop reconnect,
managed Syncthing, packaging, and human-review gates.
