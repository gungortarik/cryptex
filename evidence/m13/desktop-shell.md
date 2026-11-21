# M13 desktop shell

Windows and macOS passed the native shell rehearsal on `f72a1d8`. Each job launches the ordinary Tauri main window with the frontend embedded, using a fresh temporary pilot profile. The test-only binary requires that profile before starting its checks. [Native CI](https://github.com/gungortarik/cryptex/actions/runs/44950188369)

| Interaction | Observation |
| --- | --- |
| Open the main window | The application registers its Window menu and tray icon and presents a resizable, decorated Cryptex window. |
| Change appearance | The embedded WebView follows the light and dark window theme. Its rendered background changes in both directions and body text uses the system font family. |
| Use global Search from Timeline | A platform-specific keyboard event returns to Search and focuses its input. An open modal retains the shortcut. |
| Close and reopen | A native close hides the window. The same restore routine used by the tray brings the existing WebView back with its input intact. |

The automated check drives the live WebView and native window APIs. Physical tray, Dock, quit-menu and draft-guard interactions have earlier owner receipts. The app menu, close, reopen and guarded quit routines match those receipts' revisions. The tray builder now registers a stable ID for lookup. [Windows receipt](../m5/m5-slice3-windows-owner-native-quit-pass-20261011.md) · [macOS receipt](../m5/m5-slice3-macos-menu-owner-native-pass-20261011.md)

The first Mac check ran before WebKit finished navigation. Waiting for the main page's finished event made the test observe the mounted window. The global Search change also fixes navigation from Timeline in the product. The shell rehearsal covers the prerequisite spike. Visual review of the completed product screens and final human acceptance belong to M13 closeout.
