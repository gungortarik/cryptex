# CI existing-profile Keychain prerequisite

Status: PROPOSED; not enabled. November 15, 2026.

## Evidence and boundary

Native CI43757075897 at560f175 passes Windows. On Mac, actual product daemon
enrollment and subsequent key_store read complete without an interactive prompt.
The physical-path and empty-stream fixture setup also succeed. Startup then
correctly fails slot_scan: trust_reducer requires at least one verifiable local
record (MissingOrUnverifiableHistory). An empty enrolled profile is not a valid
positive ExistingProfile fixture. Do not relax that rule or mark this CI passed.

The earlier diagnostic43752041981 establishes that the external fixture's
self-signed code partition does not authorize the different product daemon image.
The successful owner4692ce47 interactive receipt remains separate evidence.
No supported external capture CLI can initialize the product-owned key's history:
ordinary capture requires an attributed Desktop and native ExistingProfile
readiness already requires the history. Do not impersonate Desktop or expose a
fixture operation in the product to escape this prerequisite.

## Recommended bounded CI workflow

Use only an ephemeral GitHub-hosted macOS runner and a new dedicated disposable
credential Keychain, distinct from the signing Keychain and the owner's login
Keychain. This changes test credential provisioning, not product authorization.

1. Record the runner's original default Keychain and search list. Create a fresh
   private temporary credential Keychain with a random password. Temporarily
   select it for this fixture only; retain the actual HOME.
2. Use the existing external daemon signed-transfer fixture to create nonempty
   signed synthetic history and its OS credential. Never export/read the seed
   into Python, logs, environment variables or artifacts.
3. Freeze the generated device UUID and exact service/account. Before any access
   edit, verify exactly one matching item belongs to the dedicated Keychain.
   Select only this item and this explicitly named Keychain; authorize only the
   exact creator and product daemon CDHash partitions. No wildcard, all-app,
   Apple-wide, certificate-wide or login-keychain grant. Verify the resulting
   metadata with the value-free probe. Fail on ambiguity or additional entries.
4. Execute the unchanged actual signed product ExistingProfile startup/restart
   and tampered-inventory refusal. Compare all canonical bytes and identity
   metadata. Keep the original ungranted cross-image refusal receipt/test separate;
   this positive case must say that test key access was explicitly provisioned.
5. Require reaped owned process groups, unchanged canonical/identity state, exact
   generated-item cleanup, original default/search-list restoration and deletion
   of the disposable credential Keychain. A restoration/cleanup failure fails CI.
   No keychain, private material or password is exported in an artifact.

Apple's security tool supports an explicit Keychain plus service/account filters
for generic-password partition edits and a separate default-Keychain command:
[primary command definitions](https://raw.githubusercontent.com/apple-oss-distributions/Security/main/SecurityTool/macOS/security.c).
Implementation must validate actual tool behavior on the disposable runner and
redact provisioning argv/output; that verification has not yet run.

## Acceptance and limits

Add tests for missing/duplicate/foreign items, unexpected partitions, scoped
selection, failed creation/enrollment, unresolved child cleanup and restoration
after every failure. Retain product fixture refusal, nonempty-history requirement,
image/session authentication, Keychain failure behavior and startup deadlines.
Run independent source review and both native CI jobs before claiming success.

This does not prove unattended access across future self-signed app upgrades,
owner permission denial, first-run onboarding, normal default-locator acceptance
or full M8 completion. The already delivered normal owner package is unchanged.

Alternative: leave headless existing-profile acceptance open and use interactive
owner receipts until product onboarding/upgrade permissions receive their own
design and acceptance. Changing product history policy merely to make this CI
fixture pass is not an option.
