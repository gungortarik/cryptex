# Mac task — complete native package tree parity

Read handoff and the approved native-package-handover plan. Use clean current
`m0/p0-2-cursor` containing implementation
`e5423f201543ff6a11cea0274b4a69230b545f23`; report the full tested SHA.
Use pinned Node v24.19.0 and existing Rust toolchain/dependencies.

This task verifies tree inventory/staging only. No GUI, installers, login,
production data, retained corpora or Actions. Do not launch Desktop or claim
real `.app` handover from synthetic fixture tests. No source/format repair is
authorized here; report a failure without weakening tests.

```sh
cargo build -p m0-lifecycle-compare --locked
node --test tools/m0-lifecycle-compare/scripts/package-transaction.test.mjs tools/m0-lifecycle-compare/scripts/package-daemon-health.test.mjs tools/m0-lifecycle-compare/scripts/package-native-recovery.test.mjs tools/m0-lifecycle-compare/scripts/package-native-tree.test.mjs
node --check tools/m0-lifecycle-compare/scripts/package-native-tree.mjs
node --check tools/m0-lifecycle-compare/scripts/package-native-tree.test.mjs
node --check tools/m0-lifecycle-compare/scripts/run-native-tree-proof.mjs
git diff --check
```

Expected: **79/79 Node tests** (51 + 4 + 6 + 18), no failure/skip, exit 0.
The 18 tree tests must include actual POSIX executable mode preservation and
non-executable role refusal, ordinary symlink refusal, resource/empty-directory
membership, injected copy failure with retained diagnostics, source mutation
refusal and runner root protection. Existing native recovery tests require the
newly built target-matched helper. Fixtures use the real temporary directory
and cleanup only their own generated trees/children.

Return OS/architecture, Node/Rust versions, full source SHA, build/test exit
codes, counts, three syntax and diff results and any cleanup/flush errors.
Do not edit evidence or commit/push on success; Windows records the returned
result. No broad stress, signatures, machine power loss, real Desktop replacement
or canonical-data acceptance is established by this task. ADR-0012 stays PROPOSED.
