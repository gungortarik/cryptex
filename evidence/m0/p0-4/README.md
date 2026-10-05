# M0 P0-4 — managed Syncthing

Status: **IN PROGRESS**. `7ff485d` adds an isolated, test-only controller and
physical Windows proof runner against the accepted **2.1.5** baseline.
No production replication semantics or user-global configuration are changed.
Staged package follow-up `7845ae1` also proves native-role resolution and exact
license/attribution preservation on both OSes; Mac parity reported PASS at `501d11d`.

- [Windows scoped evidence](windows-managed-syncthing-evidence-20261005.md)
- [Mac parity task](macos-managed-syncthing-test-plan-20261005.md)
- [Mac scoped evidence](macos-managed-syncthing-evidence-20261005.md)
- [Notice failure and correction](notice-byte-portability-failure-and-fix-20261005.md)
- [Notice inventory](../../../third_party/syncthing/2.1.5/README.md)

Reference interfaces: [REST configuration](https://docs.syncthing.net/rest/config.html),
[folder status](https://docs.syncthing.net/rest/db-status-get.html),
[CLI/process options](https://docs.syncthing.net/users/syncthing.html).
The controller only creates fresh synthetic test profiles under application
data. It generates distinct native identities, authenticates local REST and
keeps discovery, relay, NAT, browser and automatic upgrades disabled.

The production packaging/supervision decision still needs an ADR-0004 addendum.
Actual version-changing compatibility, installer notice inclusion
and production secret-storage integration remain unverified. This spike is not
a completed P0-4 gate or permission to start M1.
