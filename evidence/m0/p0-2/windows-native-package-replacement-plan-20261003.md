# Windows native package replacement candidate — prepared, not verified

Scope: P0-2 native installer install/replace/operator rollback experiment.
This does not select a production updater or accept ADR-0012.

Execution update: baseline, candidate replacement and operator rollback now
passed their scoped installed identity/status/IPC/clean-exit checks. See
`windows-native-package-baseline-20261003.md`,
`windows-native-package-update-20261003.md` and
`windows-native-package-rollback-20261003.md`. Preparation observations below
are historical; full atomic-update and canonical-data acceptance remains open.

## Prepared artifacts

Both unsigned NSIS builds passed locally at source `5015ff1`:

- Baseline `0.0.0`: installer SHA-256
  `7198BAC5639A4D8AE1E4F3AA76B89CFE219A9A9E986CB6C85BFFEBFC9758F425`.
- Candidate `0.0.1`: installer SHA-256
  `F05F7D9EC3534007EF84C834616DA000065154A410E6281BF2CACFB07CAB03E2`.

They are retained outside Git under
`%LOCALAPPDATA%\Cryptex-Dev\p0-2\native-package-proof-1003\baseline` and
`candidate`, with Desktop/daemon copies and `package-manifest.json` containing
all sizes/hashes. The candidate version came from an external JSON override;
repository versions and dependencies were unchanged. These are test labels,
not a production version-policy decision. The daemon is protocol 1 in both
packages and has the same hash; this is a compatible Desktop replacement with
the daemon shipped as part of both packages, not proof of daemon protocol upgrade.

## Installer audit

The generated NSIS script uses `currentUser` / `RequestExecutionLevel user`.
Its running-app check can force shutdown of the main executable through Windows
Restart Manager. It then copies Desktop and daemon separately. No Cryptex
pre-install hook, package compatibility preflight, transaction journal or
automatic rollback is currently configured. A successful ordinary installer
run cannot establish interrupted-update safety or incompatible-pair rejection.

Before installation, actual-user WMI and direct-HKCU reads found no existing
Cryptex installer `InstallLocation`. No product startup entry or test process
was active. Installation has not yet been executed.

## Operator and agent procedure

1. Run the baseline installer from ordinary Explorer so its registration is
   written in the operator's actual session. Select this isolated install folder:
   `%LOCALAPPDATA%\Cryptex-Dev\p0-2\native-package-proof-1003\installed`.
   Keep login startup disabled. Stop and report any security barrier or unexpected
   existing-install prompt; do not bypass it or accept forced shutdown.
2. Verify installed Desktop and sibling daemon hashes against the baseline
   manifest and installed version. Launch the installed Desktop and confirm
   one daemon, product runtime path, renderer status/IPC protocol 1 and the
   actual installed sidecar path. Record PIDs, then Quit clean and verify exit
   and released lock before replacing any package files.
3. Preserve read-only runtime inventory/hash evidence before replacement.
   Do not touch P0-1 corpora. Runtime metadata is derived; it is not canonical
   event data. Synthetic canaries, if used, must be labelled fixtures and cannot
   establish a production canonical-event migration pass.
4. Run the candidate installer into exactly the same install folder with no
   Desktop/daemon processes running. Verify the installed candidate Desktop
   hash, daemon hash, version and renderer status/IPC. Quit clean before rollback.
5. Reinstall the retained baseline as an operator-controlled rollback. Verify
   the old Desktop hash/version, matching daemon, one instance and IPC, and
   compare preserved data observations. Quit clean afterward. Do not select
   any app-data deletion option during eventual uninstallation.
6. Record actual results and limits. Keep automatic rollback, interrupted pair
   replacement, pre-install incompatible-pair rejection, signing and canonical
   data acceptance open until implemented and independently proved. Do not
   simulate a broken installed pair against the shared runtime without an
   isolated failure-path plan and a retained recovery package.

No GitHub workflow is required. Current package artifacts must not be overwritten
by another build before their manifest and copies are retained. The next decision
after this experiment is the scoped P0-2 update orchestration proposal; M14
productization and network auto-update services remain outside this work.
