# Mac task — actual Syncthing version transition and cold rollback

Use clean current `m0/p0-2-cursor` containing code `133428b`; report full tested
SHA. Read handoff, Master Spec, accepted stack baseline, ADR-0004 addendum and
`windows-syncthing-upgrade-evidence-20261005.md`.
This is a new isolated test of **2.1.2 -> 2.1.5**. Product pin remains **2.1.5**.
Do not repeat Desktop/native recovery/Golden/Doctor or the old managed live plan.
No Actions, installer, global upgrade/startup/profile change, personal data or
retained P0-1 profile. Do not stop unrelated Syncthing processes. On PASS report
only; no file edits/commit/push. Stop at failure; retain typed diagnostics and
owned cleanup result. Never relax pins, hash checks or timing budgets.

## Inputs — resolve without user command copying

Codex should fill the absolute `baseline_binary` and `candidate_binary` variables.
Candidate: existing inventoried 2.1.5 arm64 native executable at
`/Applications/Syncthing.app/Contents/Resources/syncthing/syncthing`;
recheck version, `file` architecture and SHA-256. Previous reported digest:
`6de41132c8a3fc78fc57486df32f9ac69d6f53db1e1910bb46f548ee7f29725e`.
Stop/report if input correspondence changes unexpectedly.

Baseline: obtain the official **2.1.2 macos-arm64** archive in a fresh input
directory below `~/Library/Application Support/Cryptex-Dev/p0-4`:
`https://github.com/syncthing/syncthing/releases/download/v2.1.2/syncthing-macos-arm64-v2.1.2.zip`.
Also obtain `sha256sum.txt.asc` from the same release. Require one exact archive
filename/checksum entry and matching archive SHA-256 **before extracting or
executing**. Retain the archive/checksum/LICENSE/AUTHORS; report provenance and
notice fingerprints. Check the extracted baseline `--version` is stable 2.1.2,
architecture arm64 and report binary SHA-256. Do not install it globally or
normalize its notices. Archive checksum matching is not signature verification.
If unavailable/mismatched, stop and report prerequisite; do not substitute a version.

## Checks

```sh
node --test tools/m0-lifecycle-compare/scripts/syncthing-pilot.test.mjs tools/m0-lifecycle-compare/scripts/syncthing-bundle.test.mjs tools/m0-lifecycle-compare/scripts/syncthing-upgrade.test.mjs
node --check tools/m0-lifecycle-compare/scripts/syncthing-pilot.mjs
node --check tools/m0-lifecycle-compare/scripts/syncthing-upgrade.mjs
node --check tools/m0-lifecycle-compare/scripts/syncthing-upgrade.test.mjs
node --check tools/m0-lifecycle-compare/scripts/run-syncthing-upgrade-proof.mjs
git diff --check
```

Require **33/33** on Mac, no failure/skip, all exits 0. Windows has **31/31**;
Mac registers two additional POSIX tests (bundle mode and linked-profile refusal).
Test fixture directories must be cleaned; retained live proofs intentionally stay.

## Actual native proof

Create only the parent if needed; the runner must create its fresh child root.
Codex fills the input variables from verified acquisition above.

```sh
upgrade_base="$HOME/Library/Application Support/Cryptex-Dev/p0-4"
mkdir -p "$upgrade_base"
upgrade_root="$upgrade_base/mac-syncthing-upgrade-20261005-$(uuidgen)"
node tools/m0-lifecycle-compare/scripts/run-syncthing-upgrade-proof.mjs --root "$upgrade_root" --baseline "$baseline_binary" --candidate "$candidate_binary"
shasum -a 256 "$upgrade_root/syncthing-upgrade-report.json"
```

Require exit 0 and report:

- exact distinct 2.1.2/2.1.5 native hashes and authenticated REST versions;
- compatible selection `candidate`, actual upgrade, paired transfer, upgraded
  restart and another transfer; identity/key/original payload retained;
- injected health rejection (expected `v2.1.999`, not a native failure claim),
  selection `baseline`, verified cold baseline restored, rejected profile retained,
  original identity/key/payload and new transfer with restored 2.1.2;
- eight clean monitor exits 0, absent PIDs, no forced cleanup/database-stop error;
- target-matched staged candidate notices/digests/POSIX modes verified and
  observed global config digest unchanged.

Report full source/platform/Node/native versions, input/archive/binary hashes,
33-test/syntax/diff exits, root/report hash, scenario selections and PID/shutdown
array. Do not paste config/logs/private keys/API keys or their private fingerprints.
Native shutdown uses the existing 25-second bound; do not extend it.
No surviving owned child processes. Keep private proof directories under app data.

This proves one version pair and cold maintenance-window rollback. It does not
prove live canonical rollback, actual native migration failure, installed updater,
crash atomicity, power loss, signing or production secret storage. ADR-0004 and M0
acceptance remain separate. Report these limitations explicitly.
