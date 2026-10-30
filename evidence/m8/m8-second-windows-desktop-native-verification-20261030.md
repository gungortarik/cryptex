# M8 second-Windows Desktop verification — 2026-10-30

## Source and environment

Owner-operated second Windows/VM: 26200.9711 / 25H2, standard-user session.
Source archive remains pinned to `a5f859a34e3b9946527d97d1187c2c5ac72381a8`.
Node 24.19.0 and `npx.cmd --yes pnpm@11.28.0` were used; dependency installation
with `--frozen-lockfile` completed. The repository pins Rust 1.98.1.

## Package smoke — PASS, owner-supplied console evidence

```powershell
npx.cmd --yes pnpm@11.28.0 --dir apps/desktop run tauri:package-smoke
```

The supplied log shows:

- Release daemon sidecar build completed in 2m 15s and the sidecar was staged
  for `x86_64-pc-windows-msvc`.
- `beforeBuildCommand pnpm build` ran TypeScript checking and Vite 8.3.1.
- The Desktop release build completed in 7m 11s and reported
  `target/release/cryptex-desktop.exe` as the built application.
- The shell returned without a reported error. This command used `--no-bundle`;
  it is not an installer/bundle verification.

This supplements the owner-supplied 520/0/4 daemon library and reported 12
binary passes recorded in
[`m8-windows-tcp-state-diagnostic-20261029.md`](m8-windows-tcp-state-diagnostic-20261029.md).
Executable hashes and an interactive native owner flow are not yet supplied.

## Next native checkpoint — pending

Create a fresh marked pilot runtime outside the source tree. Seed one public
synthetic note only through `cryptexd pilot-fixture timeline-v1`, then launch
the matching release Desktop with `--cryptex-pilot-runtime`. Its pilot window
can be opened with the supported `show.request` control file.

First verify native startup and health. Then exercise self-retirement review
and cancellation before a separate explicit confirmation on this disposable
synthetic identity. Record the displayed exact sequence/hash boundary,
journal/event outcome, and clean shutdown/restart behavior. Peer restriction,
nonzero peer cutoff cases, and denial/retry cases remain separate checks.

No interactive owner confirmation, native UI acceptance, transport activation,
macOS parity, elevated-launch support, or M8 completion is claimed by the
package smoke. The development host's TCP-state anomaly remains open.
