# M5 Slice 3 -- acknowledgement dialog recheck blocked by existing pilot

Date: 2026-10-11. Owner-reported at HEAD
`be7d76d02ebe8984e0c9d866cce7a599a125f02e`, product ancestor
`a172d4bc45a2bd041a4e28ba6342a9ab9e62dd44`, macOS 27.0.1 arm64.

Automated checks passed: capture-policy 16/16, Desktop build, fmt, and release
strict Clippy. The app was packaged. Native acknowledgement behavior was not
tested in this attempt.

The earlier synthetic pilot was preserved: Desktop 34358, daemon 34376, under
`…/Cryptex-Dev/m5/macos-slice3-archive-ux-20261011T190436Z-1f188673-7de4-49f2-8c25-ee58e24f0de6`.
Launching the candidate with a fresh pilot runtime activated
`tauri_plugin_single_instance`; it restored the existing Desktop and wrote
`window_restored_via_single_instance_launch` there. The fresh runtime never
reached `ready`. No new dialog or archive state was exercised.

The preserved pilot contains synthetic data and has already yielded the
previously recorded acknowledgement blocker. Preserve its evidence directory,
then close it through the product's supported Quit -> Confirm flow before
launching the new isolated candidate. Do not kill PIDs or delete runtime data.

No Actions, merge, Slice 4, or M5-complete claim.
