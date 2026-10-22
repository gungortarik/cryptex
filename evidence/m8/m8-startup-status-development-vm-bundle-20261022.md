# M8 startup/status development VM verification handoff

Status: staged, **guest execution NOT RUN**. Date: 2026-10-22.
Host runtime tests remain blocked by Application Control 4551; no bypass or
host policy change was made. See adjacent startup/status implementation report.

Package: `D:/VMs/Win11/Cryptex-M8-StartupStatus-20261022.zip`
Size: 13,734,246 bytes. SHA-256:
`1c9ba03b53801ce4b0a5cbc9a49fbdb82a3bb12d12339bb10ffc8bd27b814016`.

Contains current tracked source plus scoped untracked M8 daemon/scripts/evidence
and ADR-0019, the exact pinned unmodified Syncthing native executable, README
and `Run-Windows-Review.cmd`. No Git metadata, untracked personal PDF, runtime
profile, API credential or memory data was included. Native executable SHA-256
remains `36a0f7bc372f64fa7cc4f5654fa324c0dd9f7fef2e07565e00c6e1cf73f50344`.

All 33 normalized source hashes were verified against actual ZIP entries and
ZIP integrity checked. Text was normalized to UTF-8 without BOM and LF. An
initial BOM-preserving attempt failed the Cargo.toml manifest check; that
diagnostic archive was retained with `-encoding-invalid.zip`, not distributed
as the verification candidate. No source behavior was changed for packaging.

In the authorized Windows development VM, extract the full candidate archive
and run `Run-Windows-Review.cmd`. Its PowerShell process uses RemoteSigned only
for that process and verifies the exact two runner-script hashes before
removing their download marks. It does not disable Application Control or
alter machine policy. The runner verifies the 33-file source inventory, builds
and tests, then passes its own freshly built daemon into the synthetic native
startup fixture. No peer/trust activation or Actions. The launcher PowerShell
syntax was parsed without execution on the host; guest policy/toolchain/native
success is not assumed. Cargo may download its normal dependencies.

Retain the printed result and report folder. A failure remains a failure; no
manual script/source edits, exclusions or policy bypass are requested.
Metadata receipt: `vm-bundle.json` in the host proof directory documented by
`m8-startup-status-windows-20261022.md`. No guest native PASS/M8 completion.
