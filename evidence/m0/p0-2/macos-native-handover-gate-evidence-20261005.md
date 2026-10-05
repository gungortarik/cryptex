# Mac native handover OS gate parity — scoped PASS

Recorded from the Mac operator's returned plan result by the Windows implementer.
Source: `6a7d29481c19484bcacbdda17e8e3084c78b1af8`.
macOS 27.0.1 arm64; Rust/Cargo 1.98.1; Node v24.19.0.

- Rust **14/14**, Node **86/86 PASS**, no failures/skips, exit 0.
- Format, strict Clippy, locked build, two syntax and diff checks PASS.
- Seven gate cases verified contender refusal, exception release, SIGKILL lease
  invalidation, link refusal and reacquisition after abnormal EOF.
- No fixture remained; no subprocess cleanup or POSIX flush error reported.
- Mac changed no files and made no commit/push; branch reported clean.

No Mac commands were rerun on Windows. This closes the cooperating pilot gate
parity task only. Existing Desktop/startup/update entry points are not integrated
by these tests; actual native handover, signing and canonical safety remain open.
ADR-0012 remains PROPOSED. No Actions or physical lifecycle repeat was required.
