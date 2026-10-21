# macOS embedded frontend and native visual revalidation — SCOPED PASS

Operator report received 2026-10-05. This supersedes the current FAIL state for
the specific blank-renderer gate while preserving the original failure record.
It does not mean the earlier raw Cargo executable passed.

## Source and environment

- Checkout: `e755601a136e0231cbbbef279112cad0597b6f7a`, including implementation
  `65ce7b91fe500cfe751aa3d13b39010f6dcc935e`.
- Branch: `m0/p0-2-cursor`.
- macOS 27.0.1 arm64; Rust/Cargo 1.98.1; Node 24.19.0; pnpm 11.28.0.
- New packaged artifact: `Cryptex.app`; the old `7f5ab2b` raw Cargo release was
  not used for visual review.

## Focused checks

All reported commands exited 0: format check; Desktop debug tests **16/16**;
strict debug Clippy; intentionally wrong-mode raw release build followed by a
negative pilot invocation (**exit 2**, expected
`RELEASE_FRONTEND_MODE_REJECTED: ReleaseUsesDevelopmentServer`, marker-only
root); fresh Tauri `.app` package; embedded frontend release tests **17/17**
(including `supported_release_suite_requires_embedded_assets`); strict release
Clippy with `tauri/custom-protocol`; pilot CLI **4/4**; diff check.

The first wrong-mode binary copy came from a stale workspace target. The Mac
operator identified this and reran the proof with the binary produced by the
intended isolated Cargo target. Its SHA-256 was
`b64efc9cabdd1411e76d4d2855bca4d17b8fd154191bf3ea1c79fbbc0e97e6f0`.

## Fresh package inventory

| Artifact | SHA-256 | Architecture / metadata |
| --- | --- | --- |
| Desktop | `b924e69c1f3a7205635a56f1422d1c3c6849ce80e4d5f44364a63fd6205857fc` | arm64 |
| `cryptexd` | `bfbfa26c6df1934312ba0d483f5351ac759c8f3f96458f8753d5b4a70157feb4` | arm64 |
| Info.plist | — | version `0.0.0`; bundle `com.cryptex.desktop` |
| Frontend | — | `index-LEGligur.js`, `index-BzFBO_Ey.css`, `index.html` |

## Native visual and cleanup

Owned pilot visual session 2 used the new `.app`. `show.request` produced
`window_visible=true`; Desktop PID 5288 and daemon PID 5314 reported protocol-1
`health.pong` and held lock. The operator's captures across visual sessions 2
and 3 showed the rendered eyebrow, Cryptex title, runtime path, sidecar,
wrapping controls and Mode B text, including a scrolled-bottom capture. Light
theme, logical size about 960 x 640 and Retina image size 2144 x 1504 were
recorded for the observed capture.

| Capture | SHA-256 |
| --- | --- |
| Visual 2 top | `aa22167afe54bcd1addd35bc3062ad9b30b0da554deaa4936445a226689c9725` |
| Visual 3 top/health | `b81fbad1d2e2450859cb11100dfcb79da4290d972dc10d7f5c7c9e9d1c8d9b29` |
| Visual 3 bottom | `c5ad8ce8a0c67c602d99c1f95fa394406776037089e7b4aff2ade52c5ac167b2` |

Clean exit 0; PIDs absent, daemon lock/IPC/native gate released, no
`pilot-quit.error`. Proof roots are under
`~/Library/Application Support/Cryptex-Dev/p0-2/mac-embedded-frontend-{reval,visual2,visual3}-20261005-*`.
This report derives from the operator's message; raw Mac report files and image
bytes were not copied into this Windows checkout for independent re-hashing.

## Scope limits

**SCOPED PASS:** embedded release guard/checks and Mac renderer visibly populated
in the fresh packaged app. The blank-renderer failure is resolved for this
package path. AX automation could not click “IPC health ping” or show a PID in
the renderer; backend health was independently reported. No theme/DPI matrix,
installer, signing or production acceptance is inferred. ADR-0012/ADR-0004 and
M0 human acceptance remain separate gates.
