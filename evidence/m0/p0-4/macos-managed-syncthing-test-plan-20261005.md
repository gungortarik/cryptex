# Mac task — managed Syncthing parity

Use clean current `m0/p0-2-cursor` containing `7ff485d`; report full tested SHA.
Read handoff, accepted stack baseline, P0-4 contract and Windows scoped evidence.
This is a separate task from the pending native interruption/Golden/Doctor plan.
Both plans are required. Do not replace one with the other.

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
node --test tools/m0-lifecycle-compare/scripts/syncthing-pilot.test.mjs
node --check tools/m0-lifecycle-compare/scripts/syncthing-pilot.mjs
node --check tools/m0-lifecycle-compare/scripts/syncthing-pilot.test.mjs
node --check tools/m0-lifecycle-compare/scripts/run-managed-syncthing-proof.mjs
git diff --check
```

Require **7/7**, no failures/skips, all exits 0. Then run the repository runner
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
All four owned monitor exits must be 0 and PIDs absent; no forced cleanup.
Report PID/timing array and whether native database-stop failure was detected.
The 25-second shutdown budget covers pinned native 10-second connection close,
10-second database close and scheduling margin; do not shorten or extend it.
Global config's observed digest must be unchanged. No temporary child processes
remain; retained private proof profiles/report are expected outside Git.

Report full source SHA, platform/architecture, Node and Syncthing versions,
resolved binary/hash, 7/7 and syntax/diff exits, report path/hash, actual assertions
and cleanup. Do not paste REST bodies/config files/logs containing credentials.
This is two local native identities on one Mac, not cross-host replication,
actual version upgrade, production secret storage or installed distribution.
ADR-0004 acceptance/P0-4 completion remain separate.
