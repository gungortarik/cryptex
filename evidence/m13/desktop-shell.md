# M13 desktop shell

Windows and macOS passed the native shell rehearsal on `f72a1d8`. Each job launches the ordinary Tauri main window with the frontend embedded, using a fresh temporary pilot profile. The test-only binary requires that profile before starting its checks. [Native CI](https://github.com/gungortarik/cryptex/actions/runs/44950188369)

| Interaction | Observation |
| --- | --- |
| Open the main window | The application registers its Window menu and tray icon and presents a resizable, decorated Cryptex window. |
| Change appearance | The embedded WebView follows the light and dark window theme. Its rendered background changes in both directions and body text uses the system font family. |
| Use global Search from Timeline | A platform-specific keyboard event returns to Search and focuses its input. An open modal retains the shortcut. |
| Close and reopen | A native close hides the window. The same restore routine used by the tray brings the existing WebView back with its input intact. |

The automated check drives the live WebView and native window APIs. Physical tray, Dock, quit-menu and draft-guard interactions have earlier owner receipts. The app menu, close, reopen and guarded quit routines match those receipts' revisions. The tray builder now registers a stable ID for lookup. [Windows receipt](../m5/m5-slice3-windows-owner-native-quit-pass-20261011.md) Â| [macOS receipt](../m5/m5-slice3-macos-menu-owner-native-pass-20261011.md)

The first Mac check ran before WebKit finished navigation. Waiting for the main page's finished event made the test observe the mounted window. The global Search change also fixes navigation from Timeline in the product. The shell rehearsal covers the prerequisite spike. Visual review of the completed product screens and final human acceptance belong to M13 closeout.

## Product interactions

| Area | What the check observes |
| --- | --- |
| Home and Devices | A service connection can recover while a note draft stays open. Device settings report local readiness and the configured peer count separately. If a settings change loses its acknowledgement, Home drops the old observation. |
| Search, Memory and Timeline | A completed search with zero results shows the submitted phrase. Opening a result uses the guarded reader again. A refusal clears the old excerpt. Closing an available note restores keyboard focus to its trigger. |
| Doctor | An owner can check the local service. The disposable native pilot can restart its owned child or stop it after confirmation. Cancel leaves the child running. The supervisor keeps an explicitly stopped child stopped. |
| AI access | Pausing public read requests cancels outstanding requests and revokes previously approved sockets. Resuming requires a fresh grant. The pause lasts for the current daemon session. |
| Backups | Primary navigation exposes the existing owner review for backup removal. The backend retains responsibility for eligibility and explicit confirmation. |

The rendered tests exercise a lost connection and its recovery, keyboard focus, light and dark contrast, and a narrow viewport. CI captures both appearances. The native check separately exercises the embedded WebView. [Home and native shell run](https://github.com/gungortarik/cryptex/actions/runs/44950756904) Â| [Rendered interaction tests](../../apps/desktop/scripts/connection-screen.test.mjs)

## Maintenance Mode and engine ownership

Maintenance Mode keeps local capture, guarded reads and search available. Entering the mode revokes AI grants, cancels pending requests and suspends managed replication. Devices reports attention if it cannot confirm transport shutdown. Leaving restores the saved connection settings and the owner's previous AI pause preference. A revoked grant always requires a new review. The mode lasts until exit or daemon restart.

The daemon test submits an unauthorized request, enters maintenance as the owner, saves and searches for a note in temporary storage, and checks both previous AI preferences after exit. The paired Windows test also checks the live transport and reader stop and resume. Mac owner commands use the existing authenticated Mach channel. [Daemon test](../../services/cryptexd/src/ipc/tests/maintenance_tests.rs) | [Cross-platform maintenance workflow](../../.github/workflows/m13-desktop-shell.yml)

Doctor can control a daemon started by this desktop in a validated pilot profile. It matches the retained child to the authenticated service PID, asks for a clean stop and reports an unfinished stop separately. It gives the renderer no PID selection, executable path or shell. Personal profiles keep their existing write and lifecycle restrictions. [Native engine control checks](https://github.com/gungortarik/cryptex/actions/runs/44952922903)

The milestone is awaiting final CI and owner review.
