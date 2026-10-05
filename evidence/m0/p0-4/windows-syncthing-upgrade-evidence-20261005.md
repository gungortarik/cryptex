# Windows Syncthing changed-version compatibility — scoped PASS

Source: `133428b222e0549c7ee4ccd1e37b9b069513dd61` (the exact implementation
bytes used in the final run). Windows x64, Node **v24.19.0**.
This is an isolated M0 test tool; the product baseline remains **2.1.5**.
Historical **2.1.2** is only the old-version fixture.

## Native input provenance

Official baseline archive:
`https://github.com/syncthing/syncthing/releases/download/v2.1.2/syncthing-windows-amd64-v2.1.2.zip`.
Its SHA-256 matched the unique archive entry in upstream `sha256sum.txt.asc`:
`4626c13012e9620ece2393bfc3300aeafead654695d5dc096a873c27a7543c96`.
This verifies archive correspondence, not the detached signature or publisher.
Retained input: `%LOCALAPPDATA%\Cryptex-Dev\p0-4\upgrade-input-20261005-f50cdbe22efc4fa9a007ed0069afb796`.
The extracted archive, checksum file, LICENSE and AUTHORS remain outside Git.
Baseline version: `syncthing v2.1.2`, Go **1.26.5 windows-amd64**.
Baseline binary SHA-256:
`6bff9bbf005ea28f103da7a6508f2677bdd15bd344faf4b7f908872c83d76e3e`.
Upstream LICENSE/AUTHORS fingerprints respectively:
`9221c2f936159b8446d329249fb4c0f25be510f447383a0f13336ac7985668a3`,
`b9f67f73de07542f1126393579e64f8e095f02d2f7e547f1062c8df9626e7008`.

Candidate is the retained **2.1.5** binary copied into a fresh exact-role bundle
with the repository's pinned LF notices. Binary SHA-256:
`36a0f7bc372f64fa7cc4f5654fa324c0dd9f7fef2e07565e00c6e1cf73f50344`.
Both `--version` and authenticated native REST version are checked against the
explicit fixture pin before readiness. The ordinary managed launcher still
rejects 2.1.2; upgrade fixtures use a separate ownership scope.

## Strategy and live results

Pair two fresh 2.1.2 loopback profiles; transfer synthetic bytes. Pause both
folders, cleanly stop the changing peer, verify its PID absent, and copy its
complete cold profile (config, identity, database and synthetic payload).
Start 2.1.5 against that owned profile, validating isolation/pairing/versioning,
identity key bytes and original payload. No credentials or private fingerprints
are printed or written in the public report.

- Compatible: 2.1.5 selected, real transfer from the 2.1.2 peer, 2.1.5 restart
  and another new transfer PASS. Candidate PID **3236**, restart **13692**.
- Rejection: real 2.1.5 candidate **30312** runs/authenticates. An explicitly
  injected expected health version `v2.1.999` triggers typed refusal. Stop it,
  verify/copy the cold baseline, retain rejected profile in quarantine and
  restore 2.1.2 **36512**. Original identity/key/payload and new paired transfer
  PASS. The candidate's possibly migrated database is never opened by 2.1.2.
- All owned processes clean exit **0**, absent PIDs; global config observed
  digest unchanged. Native 25-second shutdown budget remains unchanged.

| Scenario | Monitor PID | Clean shutdown ms |
| --- | ---: | ---: |
| Compatible baseline | 28592 | 64 |
| Compatible candidate | 3236 | 66 |
| Compatible restart | 13692 | 63 |
| Compatible peer | 17224 | 892 |
| Rejection baseline | 5520 | 64 |
| Rejected candidate | 30312 | 10065 |
| Restored baseline | 36512 | 900 |
| Rejection peer | 26204 | 1001 |

Final root:
`%LOCALAPPDATA%\Cryptex-Dev\p0-4\version-upgrade-final-20261005-7d9a1dc998e14b459b45578e7270ecaf`.
`syncthing-upgrade-report.json` SHA-256:
`10e5e09bcfafbc317cf9bbd8c84b4afbf560ca4eb5b364abc973baac3f971bcd`.
Initial development run also passed; its retained root is
`version-upgrade-proof-20261005-2f20ecf8ce164a34a775da5e994131f4`.

## Regression checks

Combined Node **160/160** plus storage capture **5/5**, no failure/skip, exits 0.
New upgrade tests **10/10**; managed/bundle/upgrade subset **31/31** on Windows.
Four changed-script syntax checks and diff check PASS. Tests cover exact pins,
health rejection, cold config/database/mode copy, live/unclean/wrong receipts,
ownership/corruption, overlapping/existing destinations, exact rollback,
tampered backup refusal before mutation and rejected-profile preservation.

Existing managed 2.1.5 actual runner also PASS after controller refactor:
root `managed-upgrade-regression-20261005-cd98222d1cf74e418b49e5c9348be7c5`;
report SHA-256 `d8850fc05011f2e62dfdc647f0d44b5ac160b89c7c42b38cb03860c12c6581c1`.
Four clean absent monitors **26128/9508/34804/21936**, shutdown
**65/10064/851/125 ms**. All previous actual assertions and global digest PASS.
No Actions, global installation/startup change or unrelated process termination.

## Scope limits

This proves this real version pair and a controlled pre-acceptance rejection.
It does not prove a native migration failure, arbitrary downgrade, interruption
atomicity, power loss, installed updater, signing or production secret storage.
The rollback window accepts no new payload writes; restoring a cold payload is
not a strategy for rolling back live canonical writes. All data here is synthetic.
Mac parity task: `macos-syncthing-upgrade-test-plan-20261005.md`.
ADR-0004 remains PROPOSED and M0 IN PROGRESS.
