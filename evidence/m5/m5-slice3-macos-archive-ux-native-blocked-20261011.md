# M5 Slice 3 -- macOS archive UX native attempt blocked

Date: 2026-10-11. Owner-reported native attempt at source HEAD
`62342d88865d0ac0fe54d53b805c316daeea662c`, product ancestor
`eba337fcf2e81da8163254ff2092690f2a695ce3`, macOS 27.0.1 (26A434) arm64.
Working tree was clean. This is a blocked attempt, not a native PASS.

## Pilot and supported flow attempted

Fresh synthetic pilot:
`…/Cryptex-Dev/m5/macos-slice3-archive-ux-20261011T190436Z-1f188673-7de4-49f2-8c25-ee58e24f0de6`

Desktop PID 34358; daemon PID 34376. Synthetic public capture A
`bb7e8b38-475c-49c7-8226-4a15264e19ea` was rejected with pending state retained
and Resolve guidance displayed. The owner then drafted B and attempted Save.
The unresolved-capture notice appeared; `window.confirm` returned false.

The deliberate-new acknowledgement UI did not appear or become actionable via
System Events, Swift HID click, or AXPress. Consequently B was not authorized,
the archive list was never created, and no Quit Cancel/Confirm, tray, or
close-to-tray archive checks were attempted. Stop at this boundary; do not
fabricate daemon/committed state or add a product fault hook to force the path.

The pilot was reported alive at the time of the result. The owner-provided
screenshots are in the pilot `proof` directory: `08-after-save-A-crop.png`,
`09-after-save-B-crop.png`, `11-dialog-burst-*`,
`12-after-swift-save-crop.png`, `13-after-axpress-crop.png`, and
`99-blocker-final-crop.png`. They are not present in this repository checkout.

## Status and next step

Native archive UX remains blocked and unverified. Automated tests cover policy
and guard logic but are not native UX evidence. Windows native generic Quit
passed separately at product `eba337f`; Windows archive UX also remains
unverified. Mac pilot portability checks passed at the source HEAD above.

The next safe action is a focused UI investigation of why the supported
duplication-acknowledgement dialog is not presented or actionable, using the
existing synthetic pilot and diagnostics. Do not change production behavior
until the UI failure is understood and scoped. No Actions, merge, Slice 4, or
M5-complete claim.
