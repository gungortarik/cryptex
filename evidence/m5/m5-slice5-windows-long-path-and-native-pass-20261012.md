# M5 Slice 5 — Windows long-path repair and packaged native pass

**Status:** Windows focused checks and packaged native Timeline/detail review PASS. M5 and Slice 5 remain open pending macOS parity, required CI, and final DoD/human review.

## Build identity

- Branch: `codex/m5-capture-timeline`
- Product commit: `ae6f5a64dbf2825080778462257068744197c501`
- Package source checkout HEAD before commit: `6169ba590075398043cae13c1c2dfcfa68faa1df`, plus the product changes now recorded in `ae6f5a6`. The tested executable hashes below identify the exact artifacts.
- Host: Windows 11 x64
- Desktop: `target/release/cryptex-desktop.exe` — SHA-256 `AB5787CE732D5FBC3AD51078B233F3B8424237E9F5CA3DEF77629D79189BB673`
- Daemon: `target/release/cryptexd.exe` — SHA-256 `A05C016D9CE258F6CDE02146BA7E1E765FBD2DAD620E03B2D882BC3E2806ACF6`
- NSIS: `target/release/bundle/nsis/Cryptex_0.0.0_x64-setup.exe` — SHA-256 `60E1A32456A522FFBBFD5912D60D92A7D7519FDBCB2459B18525ED6D04AFCDFF`

## Long-path failure and correction

The first real fixture seed reached receipt persistence but Windows returned
`ReceiptPersistFailed` / `The system cannot find the path specified`. The failed
runtime was preserved and not reused. The receipt path includes Brain, device,
and capture IDs and can exceed the legacy 260-character Win32 limit. Receipt
filesystem operations now use Windows extended-length paths. A round-trip test
creates, confirms, and reads a receipt whose logical path is over 260 characters.

After rebuilding, a fresh marker-only runtime seeded one synthetic `public`
v1 `memory.note` through `cryptexd pilot-fixture timeline-v1`; no canonical
bytes were inserted outside the daemon capture path.

## Native packaged review

- Fresh fixture runtime: `%LOCALAPPDATA%\Cryptex-Dev\m5-s5-timeline-f5d31550525a4aa1affe4e4f70da4cb2\runtime`
- Brain: `d1f3df78-0e92-4235-891d-5470ea02e4c7`
- Event: `748687fa-ace8-4ec1-b19c-77c0ab220a9c`
- First Timeline render briefly showed zero notes; asynchronous derived catch-up then exposed exactly one note.
- Timeline row showed timestamp, `Public`, and the synthetic note snippet.
- `Open full note` showed the complete synthetic text and provenance: Event ID, Brain ID, Device ID / sequence `1`, `memory.note`, signing-key ID, and content hash.
- `Source details` expanded with the same verified provenance fields.
- File → Quit showed the native Yes/No confirmation. Confirm exited Desktop PID `36000` and daemon PID `36028`.
- Post-quit evidence: `desktop-supervisor.state` = `clean_quit_from_app_menu`, daemon PID `none`, supervisor not alive; `lifecycle.status` = `pid=none`, `note=clean shutdown released instance lock`; `gui-proof.snapshot` = `window_visible=false`, daemon not running.
- No non-synthetic or personal memory was used. The isolated runtime remains preserved.

## Windows checks

| Check | Result |
|---|---:|
| `cargo fmt --all -- --check` | PASS |
| `cargo test --locked -p cryptexd --bin cryptexd timeline_fixture` | 3/3 PASS |
| `cargo test --locked -p cryptexd --lib capture::` | 20/20 PASS |
| `cargo test --locked -p cryptexd --lib capture_receipt::` | 1/1 PASS, including >260-character receipt path |
| Desktop `pilot_runtime` | 9/9 PASS |
| daemon Clippy, all targets, `-D warnings` | PASS |
| Desktop release-bin Clippy, `-D warnings` | PASS |
| `npm run tauri:package -- --bundles nsis` | PASS |
| `git diff --check` | PASS |

This does not establish macOS parity, paging beyond a single note, or the M5
completion gate. Required macOS Slice 5 checks/native review and final CI/DoD
acceptance remain open. No merge or M5-complete claim.
