# Mac task — managed Syncthing parity

**Status:** completed scoped PASS at `501d11d` (22/22 plus live runner).
See `macos-managed-syncthing-evidence-20261005.md`. Instructions below are retained
for reproduction; no new execution is requested by this completed plan.

Use clean current `m0/p0-2-cursor` containing notice/observation fix `f17bcf7`;
report full tested SHA. It must contain `syncthing-bundle.mjs` and its tests.
Read handoff, accepted stack baseline, P0-4 contract and Windows scoped evidence.
Native interruption/Golden/Doctor already reported PASS at `502abf4`.
Rerun only this managed task; do not repeat the passed native plans.

No Actions, installer, startup, personal data, global Syncthing process/config
changes or retained P0-1 profile use. Do not stop an unrelated process. On PASS
report only; no file edits/commit/push. Stop/report failure, retaining diagnostics;
never weaken a failed assertion or substitute old Syncthing `2.1.2`.

## Pinned native input

Resolve an available **stable 2.1.5 arm64** native executable and report its
path, `--version` output and SHA-256. If unavailable, report that prerequisite;
do not upgrade the user's global installation. A separately extracted official
2.1.5 release under fresh Cryptex-Dev application data may be used; verify the
upstream release checksum and record archive provenance, executable architecture,
license/author files. Such a checksum is not publisher-authentication proof.
Do not print generated API keys, key/config contents or private certificates.

## Checks and actual proof

```sh
node --test tools/m0-lifecycle-compare/scripts/syncthing-pilot.test.mjs tools/m0-lifecycle-compare/scripts/syncthing-bundle.test.mjs
node --check tools/m0-lifecycle-compare/scripts/syncthing-pilot.mjs
node --check tools/m0-lifecycle-compare/scripts/syncthing-pilot.test.mjs
node --check tools/m0-lifecycle-compare/scripts/run-managed-syncthing-proof.mjs
node --check tools/m0-lifecycle-compare/scripts/syncthing-bundle.mjs
node --check tools/m0-lifecycle-compare/scripts/syncthing-bundle.test.mjs
git diff --check
```

Require **22/22 on Mac** (including POSIX executable mode and notice/Git/observation
regressions), no failures/skips, all exits 0. Windows has 21 because its
POSIX-only test is not registered. Confirm
`git ls-files --eol third_party/syncthing/2.1.5` reports LF index and LF working-tree notice files.
Do not normalize notices manually or accept an alternative hash.
Then run the repository runner
with the resolved absolute pinned binary; Codex should fill that variable.

```sh
managed_base="$HOME/Library/Application Support/Cryptex-Dev/p0-4"
mkdir -p "$managed_base"
managed_root="$managed_base/mac-managed-proof-20261005-$(uuidgen)"
node tools/m0-lifecycle-compare/scripts/run-managed-syncthing-proof.mjs --root "$managed_root" --binary "$managed_binary"
shasum -a 256 "$managed_root/managed-syncthing-report.json"
```

Require runner exit 0 and every actual report assertion: distinct native
identities/homes, incorrect-key 401/403 rejection, loopback isolation, pairing,
folder pause/resume, idle status, version history, retained identity/payload,
immediate restart clean stop, actual new transfer after another restart, and
native conflict visibility preserving **both exact divergent contents**.
Require `staged_native_package_verified=true` and
`staged_license_attribution_verified=true`: the native executable must run from
the fresh proof's `native-package`, alongside exact pinned LICENSE/AUTHORS files.
Its bundle membership/digests and POSIX modes are reverified after shutdown;
this is staged package proof, not installed distribution or code signing.
All four owned monitor exits must be 0 and PIDs absent; no forced cleanup.
Report PID/timing array and whether native database-stop failure was detected.
The 25-second shutdown budget covers pinned native 10-second connection close,
10-second database close and scheduling margin; do not shorten or extend it.
Global config's observed digest must be unchanged. No temporary child processes
remain; retained private proof profiles/report are expected outside Git.

Report full source SHA, platform/architecture, Node and Syncthing versions,
resolved binary/hash, 22/22 and syntax/diff exits, report path/hash, actual assertions
and cleanup. Do not paste REST bodies/config files/logs containing credentials.
This is two local native identities on one Mac, not cross-host replication,
actual version upgrade, production secret storage or installed distribution.
ADR-0004 acceptance/P0-4 completion remain separate.

## Re-run after the reported failure

The previous source `502abf4` failed **7 PASS / 8 FAIL** before live Syncthing
because notice hashes represented Windows CRLF checkout bytes. Retain that
failure and read `notice-byte-portability-failure-and-fix-20261005.md`.
After pulling the fix, perform the complete managed plan above, using the
already inventoried stable 2.1.5 arm64 input (recheck path/version/hash first).
No acquisition/global upgrade is needed if that input is unchanged.
Do not repeat the passed native recovery/Golden/Doctor or changed-package tests.
