# Mac task — native handover OS gate parity

**Result:** scoped PASS at `6a7d29481c19484bcacbdda17e8e3084c78b1af8`.
See `macos-native-handover-gate-evidence-20261005.md`; instructions retained below.

Read handoff and approved native-package-handover pilot plan. Use clean current
`m0/p0-2-cursor` containing `80b727fdceab67ca4a9075371a07aca9cccf9654`;
report exact tested SHA. Keep pinned Rust and Node v24.19.0.
No GUI, installed updater, startup registration, retained experiment reuse,
canonical data or Actions. Do not edit source/evidence or commit/push on success.
Report failures without changing tests or declaring a partial run PASS.

```sh
cargo fmt --all -- --check
cargo test -p m0-lifecycle-compare --locked
cargo clippy -p m0-lifecycle-compare --all-targets --locked -- -D warnings
cargo build -p m0-lifecycle-compare --locked
node --test tools/m0-lifecycle-compare/scripts/package-transaction.test.mjs tools/m0-lifecycle-compare/scripts/package-daemon-health.test.mjs tools/m0-lifecycle-compare/scripts/package-native-recovery.test.mjs tools/m0-lifecycle-compare/scripts/package-native-tree.test.mjs tools/m0-lifecycle-compare/scripts/package-handover-gate.test.mjs
node --check tools/m0-lifecycle-compare/scripts/package-handover-gate.mjs
node --check tools/m0-lifecycle-compare/scripts/package-handover-gate.test.mjs
git diff --check
```

Expected: Rust **14/14**, Node **86/86** (51 + 4 + 6 + 18 + 7), zero failures/skips,
exit 0; no Clippy warnings. Seven new gate tests exercise live contender refusal,
operation-error release, native holder SIGKILL and lease invalidation, wrong
marker/existing and dangling link refusal, repository/input refusal, persistent
lock reuse/idempotent release, and abnormal stdin EOF with subsequent acquisition.
The old source-format problem was repaired on Windows; no formatter repair is
authorized by this plan. Report any failure rather than upgrading dependencies.

Return platform, tool versions, exact SHA, all command exits/counts and any
fixture/child cleanup or POSIX flush errors. Windows records evidence from your
report. This gate covers cooperating pilot callbacks only; actual Desktop startup
and update flows are not integrated yet. SIGKILL is not power-loss evidence.
ADR-0012 remains PROPOSED; signed/native `.app` handover and canonical safety are
still outside this parity task.
