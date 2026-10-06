# M0 P0-4 — managed Syncthing

Status: **COMPLETE for the synthetic M0 P0-4 gate**. The owner accepted the
scoped ADR-0004 implementation addendum: pinned **2.1.5** file transport,
isolated managed profile and authenticated backend-owned loopback REST.
`7ff485d` adds the isolated, test-only controller and physical Windows proof
runner; staged package follow-up `7845ae1` proves native-role resolution and
license/attribution preservation on both OSes. Mac parity passed at `501d11d`.
Production replication semantics and user-global configuration remain outside
this scope.

- [Windows scoped evidence](windows-managed-syncthing-evidence-20261005.md)
- [Mac parity task](macos-managed-syncthing-test-plan-20261005.md)
- [Mac scoped evidence](macos-managed-syncthing-evidence-20261005.md)
- [Windows changed-version evidence](windows-syncthing-upgrade-evidence-20261005.md)
- [Mac changed-version task](macos-syncthing-upgrade-test-plan-20261005.md)
- [Mac changed-version evidence](macos-syncthing-upgrade-evidence-20261005.md)
- [Notice failure and correction](notice-byte-portability-failure-and-fix-20261005.md)
- [Notice inventory](../../../third_party/syncthing/2.1.5/README.md)

Reference interfaces: [REST configuration](https://docs.syncthing.net/rest/config.html),
[folder status](https://docs.syncthing.net/rest/db-status-get.html),
[CLI/process options](https://docs.syncthing.net/users/syncthing.html).
The controller only creates fresh synthetic test profiles under application
data. It generates distinct native identities, authenticates local REST and
keeps discovery, relay, NAT, browser and automatic upgrades disabled.

The scoped M0 packaging/supervision choice is now accepted in
`docs/adr/ADR-0004-managed-syncthing-implementation-addendum.md`. Installer
notice inclusion and production secret-storage integration remain unverified
production gates. Completion of P0-4 does not authorize M1.

Windows actual 2.1.2 -> 2.1.5 fixture transition and controlled cold rollback
reported scoped PASS at `133428b`. Mac parity at `dec5fb1` passed 33/33 and both
live scenarios. The product pin remains 2.1.5. Production/live canonical rollback
and crash atomicity are not claimed.
