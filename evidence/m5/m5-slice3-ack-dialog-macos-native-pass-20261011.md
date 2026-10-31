# M5 Slice 3 -- macOS native acknowledgement and archive-path PASS

Date: 2026-10-11. Owner-reported native verification at source HEAD
`c292d0902b3f387e0f3547fa7d165e18587fdcd0`, product
`a172d4bc45a2bd041a4e28ba6342a9ab9e62dd44` is an ancestor. Host: macOS
27.0.1 arm64.

## Old pilot cleanup

The prior blocked synthetic pilot (Desktop 34358 / daemon 34376) was closed
through Cmd+Q -> Discard and Quit. Both processes exited and recorded
`clean_quit_from_app_menu`. Proof count remained 32; its proof directory was
preserved.

## Fresh synthetic pilot

Pilot: `…/macos-slice3-ack-dialog-20261011T200243Z-f72c31bd-f96d-4fb1-8ab0-3195007d6086`
Desktop PID 45454; daemon PID 45464.

| Step | Owner-reported result |
|---|---|
| Pending capture A | `1e8f1742-b724-4690-8e01-8a8f6602c80c` |
| Deliberate-new acknowledgement | Visible: “Start another capture?” / “Start new capture” |
| Cancel | Draft B and pending A preserved |
| Confirm | A appeared under Earlier unresolved captures with Resolve |
| Quit Cancel | Both PIDs remained alive |
| Quit Confirm | Clean shutdown |

This verifies the acknowledgement and archive path on macOS through the
supported synthetic product flow. The result is owner-reported evidence; no
automated rerun is claimed here.

## Remaining checks

- Optional close-to-tray/reopen with archive state was not run.
- Windows native verification at product `a172d4b` remains pending.
- No Actions, merge, Slice 4, or M5-complete claim.
