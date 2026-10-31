# Mac task — native interrupted-recovery cleanup and format verification

**Result:** scoped PASS at `7011fc7b316c8abcaf3f761663f42d0e04bb5a68`.
Initial source `486d86c55ed5bd7e41ba23707619f7a129511124` required the authorized
formatting repair. See `macos-package-native-recovery-evidence-20261004.md` for
the returned checks and remaining scope limits. Instructions below are retained.

Read current handoff and ADR-0012 candidate proposal. Use the clean current
`m0/p0-2-cursor` source containing this plan; report full source SHA.
Preserve local changes and branch strategy. Use pinned Rust and Node v24.19.0.
No GUI, installer, physical lifecycle repeat or GitHub Actions required.

Windows Application Control blocked cargo-fmt (4551); formatting is pending.
Run `cargo fmt --all -- --check` first. If it fails, run
`cargo fmt -p m0-lifecycle-compare`, review the diff, and commit/push only the
formatter's changes to the lifecycle tool. Do not refactor or modify behavior.
If formatting touches unrelated files, stop and report that diff. Record both
the original SHA and the formatting commit, then test the formatted source.
This narrowly authorizes formatting repair; Windows records host evidence.

Required checks after formatting:

```sh
cargo fmt --all -- --check
cargo test -p m0-lifecycle-compare --locked
cargo clippy -p m0-lifecycle-compare --all-targets --locked -- -D warnings
cargo build -p m0-lifecycle-compare --locked
node --test tools/m0-lifecycle-compare/scripts/package-transaction.test.mjs tools/m0-lifecycle-compare/scripts/package-daemon-health.test.mjs tools/m0-lifecycle-compare/scripts/package-native-recovery.test.mjs
node --check tools/m0-lifecycle-compare/scripts/package-native-recovery.mjs
node --check tools/m0-lifecycle-compare/scripts/package-native-recovery.test.mjs
git diff --check
```

Expect Rust **13/13**, Node **61/61** (51 + 4 + 6), zero failures/skips and no
Clippy warnings. The native tests need the newly built debug helper and verify
stale owner cleanup, live/malformed claim refusal, competing cleaners and OS lock
release after native-holder SIGKILL. Core recovery/I/O/race tests also pass.
Use ordinary test fixture allocation; do not modify retained experiments or
canonical data. POSIX directory flush errors must be reported, never swallowed.

Return OS/architecture, tool versions, tested SHA(s), test totals/exit codes,
format/lint/build/syntax/diff results and fixture/child cleanup errors. Apart from
the scoped formatting repair, do not edit files or commit/push evidence.
Native cleanup is test-only and locks claim removal only. Production handover,
signing, broad stress, power loss and canonical-data acceptance remain open.
