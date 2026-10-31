# M5 Slice 3 -- deliberate-new capture acknowledgement dialog fix

Date: 2026-10-11. Product commit: `a172d4b` (full SHA recorded by Git).
Based on the native Mac archive UX attempt at source `62342d8`, which blocked
because the `window.confirm` acknowledgement did not appear or respond to
accessibility input.

## Change

The unresolved-save path now opens a Cryptex-owned HTML `<dialog>` with explicit
Cancel and Start new capture actions. It uses modal semantics and the platform
WebView's focus and Escape handling. Cancel closes the dialog without changing
pending capture/archive state. Confirm re-evaluates the current reducer state,
requires the explicit duplication acknowledgement, and then submits a new
capture ID through the existing daemon IPC path. No canonical writer or event
contract changed.

## Local verification

- `pnpm --filter @cryptex/desktop test:capture-policy`: 16/16 PASS
- `pnpm --filter @cryptex/desktop build`: PASS (`tsc --noEmit` + Vite)
- `git diff --check`: PASS

These checks do not establish native dialog behavior. Required next check is a
fresh macOS synthetic pilot at this commit: reproduce pending A, draft B, verify
the acknowledgement is visible/actionable, Cancel preserves A/B, then confirm
and verify A is tracked in the archive with Resolve. Continue through archive
Quit Cancel/Confirm only after the archive row is genuinely created by the
supported product path. Stop if that path fails; do not fabricate state.

Windows native verification of this new product commit is also still required.
No Actions, merge, Slice 4, or M5-complete claim.
