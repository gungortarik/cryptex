# M17 signing for the owner's Mac and Windows VM

Status: proposed release profile. The owner chose these two devices as the audience. The disposable signing tests exercise native package formats. Enrollment of a long-lived signing key and use of personal data require separate owner action.

## First installation

Choose the per-user NSIS installer on Windows and the application bundle on macOS. Each platform uses its own code-signing identity. Generate the private key on an owner-controlled machine and keep it in the platform's protected key store. Release signing is separate from note signing and Recovery Kit approval. No service, installer or updater receives the private key.

Record the public certificate's SHA-256 fingerprint on the machine that created it. Verify the first installer or bundle against that fingerprint through an independent local channel before opening it. A certificate supplied beside a download cannot enroll itself. Preserve normal OS security policy. A local signer does not confer Apple notarization or a public Windows publisher identity. The Mac daemon and Desktop still require the live role and image checks of the [existing image policy](m8-macos-release-image-policy.md).

For first-install verification, use the verifier from a trusted checkout. Keep two SHA-256 values from the signing machine: the public certificate's DER fingerprint and the release hash. On Windows the release hash covers the signed NSIS executable. Run `verify_first_install_windows.ps1 -Installer PATH -SignerSha256 PIN -ReleaseSha256 HASH` in PowerShell. On Mac the release hash covers the signed `Contents/MacOS/cryptex-desktop`. Run `python3 verify_first_install_mac.py PATH.app PIN HASH`. This checks the bundle seal and the signer of each architecture of Desktop and its daemon. Both [verifiers](../../scripts/m17/) perform inspection before installation. They leave certificate trust and user profiles untouched.

## Updates and rollback

Use the previously enrolled public certificate to authenticate the candidate and the retained installation. Bind the platform, architecture, release identity and package contents to the approved candidate before executing any update probe. The existing protocol check then tests compatibility. If activation fails, verify the retained installation before reopening it. A replacement certificate requires explicit owner verification through the same independent channel as the first install. A missing key or unverifiable retained image keeps the package inactive.

Keep signed application binaries separate from the Brain, OS credentials, backups and Recovery Kit. An ordinary uninstall removes the application and its startup registration. Erasure of personal data remains a distinct owner operation under the existing purge rules. Disposable signed update and reinstall tests preserved a signed note on both systems. The Windows NSIS test also verified removal of the matching login entry. The Mac removal path and the owner-controlled installations still require verification.

The standalone updater takes the independently approved public certificate at build time through `CRYPTEX_UPDATE_SIGNER_DER`. A build without that certificate refuses admission. The `fingerprint` operation measures the installed directory. An attached CMS statement binds the full directory digest, OS and CPU architecture. The `admit` operation validates that statement and the two native executables before saving a receipt. The normal `apply` operation rechecks the selected and retained images against their receipts. The owner-selected digest identifies the requested update. It grants no signing authority. [Installed update evidence](../../evidence/m17/personal-packaging.md#signed-updates)

## macOS startup after installation

The controlled [development packager](../../scripts/m8/mac_lan_setup.py) signs two executables, copies the web assets into `Contents/Resources/frontend`, and writes the signed startup inventory into Resources. The ordinary [Tauri package](../../apps/desktop/src-tauri/tauri.bundle.conf.json) seals the complete bundle and embeds the frontend. Copying the development inventory into the finished application changes sealed resources. Generating it before the seal would record the desktop image before its final signature.

The sealed installation rehearsal generates a signed inventory after signing the bundle. A protected copy of the frontend is included before signing. Desktop and the daemon verify the detached inventory against the independently provisioned certificate, source revision and architecture, and authenticate each live peer against the [verified snapshot and role hashes](../../services/cryptexd/src/m8_owner_channel/release_inventory.rs). The [ordinary window launch](https://github.com/gungortarik/cryptex/actions/runs/45276597822) used a fresh profile and verified its synthetic note on exit. The feature refuses an existing-profile launch.

The release updater must make the matching signed inventory available for each image before probing it and keep the retained image's inventory available through rollback. The owner installation and release review remain outstanding.

### Preparing and receiving a Mac inventory

On the signing Mac, identify the final signed application, the reviewed build commit, the public certificate's DER SHA-256, and the approved signed Desktop executable SHA-256. With the reviewed Keychain helper on this machine, run the following from the checkout using existing output directories.

```sh
python3 scripts/m17/prepare_mac_inventory.py prepare APP CERTIFICATE_SHA256 IMAGE_SHA256 OUTPUT SOURCE_COMMIT --mac-helper HELPER
```

Transfer the sealed application and the resulting `cryptex-release-IMAGE_SHA256` directory together. Keep the approved fingerprints available through the independent owner channel. On the receiving Mac, place the metadata into the per-user application directory with the following command before applying the separately signed package update.

```sh
python3 scripts/m17/prepare_mac_inventory.py stage APP CERTIFICATE_SHA256 IMAGE_SHA256 "$HOME/Applications" DELIVERED_INVENTORY_DIRECTORY
```

The receiving tool verifies a private copy of the package and asks its read-only entry point to authenticate the exact inventory bytes and protected frontend. It then publishes both files together. Repeating the operation accepts an identical pair and preserves any conflicting existing slot. This path is available in the sealed rehearsal build. Personal-profile activation still requires the release decision.

## Scope and release decision

Private CI jobs use disposable identities and synthetic data. Their certificates expire or are discarded. The release decision requires a verified owner-controlled installation on both target systems, the beta checklist and explicit approval. The [transport hold](../../evidence/m16/hardening.md#personal-use-release-hold) remains in force independently of package signing.
