# Mac native package tree parity — scoped PASS

Recorded by the Windows implementer from the Mac operator's returned plan result.
Source: `8296e66154b7e123519ca08398e9e4bbccc2f7e8`.
Platform: macOS 27.0.1 arm64; Node v24.19.0; Rust 1.98.1.

- **79/79 Node tests PASS**, zero failures/skips, exit 0.
- Locked helper build, three syntax checks and diff check PASS.
- The 18 tree tests verified POSIX executable mode preservation,
  non-executable/symlink refusal, resource/empty-directory membership,
  injected copy diagnostics and source mutation refusal.
- No flush or owned-child cleanup error reported; no fixture remained.
- Mac changed no files, made no commit/push and reported a clean branch.

These Mac checks were not rerun on Windows. They close requested synthetic
complete-tree parity only. No actual `.app` handover, publisher authentication,
power-loss durability or canonical-data acceptance is established. ADR-0012
remains PROPOSED. No Actions or physical lifecycle repeat was required.
