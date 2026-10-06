# macOS native foundation UI attempt — NOT VERIFIED

> **Historical attempt:** This pilot could not produce a visual capture. It was
> superseded for the scoped M0 visual gate by the fresh Tauri `.app` PASS in
> `evidence/m0/macos-embedded-frontend-visual-evidence-20261005.md`. The failure
> below remains valid evidence for this specific raw Cargo/pilot attempt; Mac
> artifacts were not independently re-hashed on Windows.

Checkout: `9752798d577e937f784b8ff21c546850aee58193`.
Retained build: `7f5ab2bdaa72e213f5cb16312bca0292c436db18`.
Mac executor reports relevant application source identical to current checkout.
Platform: macOS arm64. This is owner-supplied provenance; Windows did not
inspect/recompute the Mac-local artifacts.

Computer Use did not recognize the running pilot executable: `Invalid app`.
No screenshot/layout/scrolling proof produced. **Visual gate NOT VERIFIED**.
Tool failure is neither visual PASS nor an established application defect.

Reported native session: Desktop **94503**, daemon **94513**; isolated pilot,
correct parent/executable chain, protocol-1 `health.pong` and held lock verified.
Clean exit **0**, daemon absent, daemon lock/gate released, IPC empty and no
`pilot-quit.error`. No old tests, rebuild, Actions, source edit or commit/push.

Mac-local report:
`/Users/tarikgungor/Library/Application Support/Cryptex-Dev/p0-2/mac-native-ui-20261005-4625386B-226D-46BA-9DFA-E9A4F1BD6B35/proof/ui-session-report.json`.

Owner-supplied SHA-256:
`b7fdc01699f93d3b86dbf22c7e86cdb4bca0888e2272edff88f836303448862f`.

Continuation: operator-screenshot fallback in
`m0-native-foundation-ui-review-plan-20261005.md`. Use a new isolated owned
session; never reuse the completed pilot runtime. No security-policy changes,
fabricated views or old suite repetitions. Windows visual proof remains scoped
PASS; Mac visual/human ADR/completion gates remain open.
