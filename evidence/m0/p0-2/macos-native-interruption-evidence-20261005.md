# macOS native recovery and Golden/Doctor — 2026-10-05

**Result: scoped PASS**, supplied by the Mac validation executor through the
project owner. Tested SHA: `502abf409925b368ff1377380c719069207f0d61`.
The Windows implementer records that report; the Mac-local artifact was not
opened from Windows. macOS **27.0.1 arm64**, Node **v24.19.0**, Rust/Cargo
**1.98.1**, pnpm **11.28.0**.

Reported Rust **71/71**: Desktop 10, daemon 34, IPC 6, helper 14, storage 7.
Node **123/123** and storage capture **5/5**, no failure/skip. Format, strict
workspace Clippy, locked build, arm64 `.app`, six syntax checks and diff PASS;
all command exits **0**. These counts are the supplied Mac per-binary counts,
not the Windows workspace total.

## Actual native boundary and recovery

- Updater **82701** received SIGKILL at `selected-before-candidate-launch`.
- Prior Desktop/daemon **82711 / 82719** had cleanly stopped before the kill.
- Recovered baseline Desktop/daemon **82733 / 82740** used correct baseline
  executable paths with verified parent ownership.
- Actual protocol **1**, `health.pong`, matching PID, held lock; Doctor **healthy**.
- Final exit **0**, PIDs absent, daemon lock/native gate released, IPC absent,
  no native intent or recovery claim.
- Exact synthetic sealed-pack/manifest inventory was preserved; no POSIX flush
  or owned-child cleanup error.

Report path:
`/Users/tarikgungor/Library/Application Support/Cryptex-Dev/p0-2/mac-native-interruption-20261005-B8D95248-110E-49AC-8D84-AE2FE2D8726B/proof/native-recovery-report.json`.
Supplied SHA-256:
`7fb789012223c460927bdbf3b90adaff1fa1fa00d0c482a71f51a4a836e5c136`.

Repo was reported clean; no source/evidence edit, commit/push, Actions,
startup/global config change. Physical flash/focus was not separately observed.
This proves this pre-successor-launch interruption and synthetic foundation;
not live-child interruption, interrupted recovery reclamation, machine power
loss, installed/signed updater or production canonical-schema acceptance.
Doctor canonical-store/Syncthing checks retain their unimplemented status.
ADR-0012 stays **PROPOSED**. Managed Syncthing is a separate failed/pending gate.
