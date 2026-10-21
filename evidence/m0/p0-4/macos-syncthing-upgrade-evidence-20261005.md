# Mac Syncthing changed-version compatibility — scoped PASS

> **Historical status note (2026-10-05):** Status statements below reflect the state when this dated plan/result was recorded, before M0 closeout. Current M0 status is COMPLETE for the synthetic foundation scope; scoped ADR-0012/0004 are ACCEPTED. This note does not expand this artifact's evidence, satisfy production residuals, or authorize M1. See `evidence/m0/m0-closure-audit-20261005.md`.

The project owner supplied the Mac executor's report. The Windows executor did
not open the Mac-local artifact or independently recompute its digest.
Tested SHA: `dec5fb19fb37b1199ac0f192c929a7113388cc0b`.
macOS **27.0.1 arm64**, Node **v24.19.0**. CLI and authenticated REST confirmed
native **2.1.2 -> 2.1.5**. Product pin remains 2.1.5.

## Inputs

Official baseline archive:
`https://github.com/syncthing/syncthing/releases/download/v2.1.2/syncthing-macos-arm64-v2.1.2.zip`.
SHA-256 matched the unique upstream archive checksum entry:
`fa2002f320170b64f226ff65831ccec516b59d7bba365acb51731dd079e52d1a`.
This is checksum correspondence, not detached-signature/publisher verification.
Baseline binary SHA-256:
`0325655f864024e0eecd78df07bdedbad46cefc91f480503ff615b6211081869`.
Candidate binary SHA-256:
`6de41132c8a3fc78fc57486df32f9ac69d6f53db1e1910bb46f548ee7f29725e`.
Baseline upstream LICENSE/AUTHORS fingerprints:
`9221c2f936159b8446d329249fb4c0f25be510f447383a0f13336ac7985668a3`,
`b9f67f73de07542f1126393579e64f8e095f02d2f7e547f1062c8df9626e7008`.

## Verification

- **33/33** relevant tests, no failure/skip; four syntax and diff checks PASS.
- Both live scenarios and runner exit **0**; previous plans not repeated.
- Compatible selection `candidate`: actual upgrade, paired transfer, upgraded
  restart and new transfer PASS.
- Health-reject: injected expected `v2.1.999` rejected; selection `baseline`.
  Cold 2.1.2 restore preserved original identity/key/payload and enabled a new
  transfer. Rejected candidate profile retained; no in-place database downgrade.
- Staged candidate notices/hash/POSIX mode checks PASS; observed global config
  digest unchanged. No forced cleanup or native database-stop error.

| Scenario / monitor | PID | Clean shutdown ms |
| --- | ---: | ---: |
| Compatible baseline | 85878 | 18 |
| Candidate | 85889 | 17 |
| Candidate restart | 85894 | 18 |
| Compatible peer | 85874 | 897 |
| Rejection baseline | 85906 | 19 |
| Rejected candidate | 85912 | 10020 |
| Restored baseline | 85922 | 883 |
| Rejection peer | 85902 | 1002 |

All eight monitors exit **0**; PIDs absent. Mac repo clean, no source/commit/push,
Actions or global installation/startup changes.
Retained report:
`/Users/tarikgungor/Library/Application Support/Cryptex-Dev/p0-4/mac-syncthing-upgrade-20261005-76534980-E8DD-42AF-91F9-120AC6D5FD3D/syncthing-upgrade-report.json`.
Reported SHA-256:
`92a5a46cc2b6f79f094f3af1d6c786763cf10e490398a37d14c73d922962fc21`.

## Acceptance limits

This closes the Mac parity task for this exact version pair and cold
maintenance-window rollback. It does not establish live canonical rollback,
native migration failure, installed updater, crash atomicity, power loss,
signing or production secret storage. ADR-0004 remains PROPOSED; P0-4/M0 human
acceptance and remaining boundary/notice reviews are separate gates.
