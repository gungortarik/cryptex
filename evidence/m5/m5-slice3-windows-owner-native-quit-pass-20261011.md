# M5 Slice 3 -- Windows owner-observed native Quit checks

Date: 2026-10-11. Product: `eba337fcf2e81da8163254ff2092690f2a695ce3`.
Source documentation tip: `62342d88865d0ac0fe54d53b805c316daeea662c`.

## Evidence boundary

The owner performed these interactions inside the Windows 11 VMware guest.
Results are owner reports and supplied screenshots/status output, not automated
interaction or tests executed by the reviewing agent. Synthetic unsaved text
`M5 synthetic draft - preserve me`, classification `public`; no Save requested.
Package provenance/hashes: `m5-slice3-pilot-runtime-portability-fix-20261011.md`.

## Results

- App-menu Quit -> No: owner reports Desktop stayed open without a problem.
- Initial session subsequent exit: status and GUI snapshot show
  `clean_quit_from_app_menu`, Desktop PID 5528, recorded daemon PID 8112,
  daemon not running, supervisor not alive, and clean shutdown released the
  instance lock. Exact final UI action is ambiguous; do not claim a separately
  observed app-menu Yes dialog from these records.
- Fresh session Desktop PID 5660, recorded daemon PID 3288: tray menu contained
  Open Cryptex / Quit Cryptex.
- Window X -> tray Open: owner confirms synthetic draft preserved.
- Tray Quit -> No, followed by later Quit -> No: owner confirms one dialog per
  attempt and draft preserved.
- Tray Quit -> Yes: supplied process query returns no Cryptex processes;
  supervisor state is `clean_quit_from_tray`, `daemon_pid=none`,
  `supervisor_alive=false`. Lifecycle records `pid=none` and
  `note=clean shutdown released instance lock`.

First runtime: `%LOCALAPPDATA%\Cryptex-Dev\m5-native-7e3dbcd6d2024ba8b51e298d8a70c712\runtime`.
Tray runtime: same parent, `runtime-93b31f2cbf7142a0815e12a6f69002fd`.

## Remaining gates

- macOS focused verification of the subsequent portability fix at `eba337f`
  passed at source HEAD `62342d8`; see
  `m5-slice3-macos-portability-and-archive-gap-20261011.md`. Earlier native Mac
  Quit PASS at `b9b3e4c` remains scoped to that product.
- Archive / unresolved Quit UX is NOT VERIFIED natively on either platform.
  The Mac attempt is blocked at the duplication acknowledgement; see
  `m5-slice3-macos-archive-ux-native-blocked-20261011.md`. No waiver or
  synthetic fault production hook is authorized.
- No Actions, merge, Slice 4, or M5 completion is claimed.
