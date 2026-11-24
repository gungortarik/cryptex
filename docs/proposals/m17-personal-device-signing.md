# M17 signing for the owner's Mac and Windows VM

Status: proposed release profile. The owner chose these two devices as the audience. The disposable signing tests exercise native package formats. Enrollment of a long-lived signing key and use of personal data require separate owner action.

## Audience and cost

For the owner's Mac and Windows VM, use an independently verified local signing certificate on each platform. This path has no certificate subscription fee. The Windows package is a per-user installer and the Mac delivery contains the sealed application. The hosted rehearsals use disposable identities. A certificate created for a CI job must never be enrolled on either personal device.

Public distribution would require a separate decision about publisher identity and OS trust. Apple's [Developer Program](https://developer.apple.com/programs/enroll/) and Microsoft's [Artifact Signing](https://azure.microsoft.com/en-us/products/artifact-signing/) are optional services for that decision. Neither is a prerequisite for this two-device rehearsal.

The Windows first-install verifier requires Authenticode to report a valid signature as well as a matching pinned certificate. The owner will need to establish trust in their own signing certificate on the receiving Windows account before that verification can succeed. The hosted test's machine-wide trust enrollment belongs to its disposable runner. Personal-device enrollment and Mac first-launch approval are separate steps for the owner after the release gates pass.

## First installation

Choose the per-user NSIS installer on Windows and the application bundle on macOS. Each platform uses its own code-signing identity. Generate the private key on an owner-controlled machine and keep it in the platform's protected key store. Release signing is separate from note signing and Recovery Kit approval. No service, installer or updater receives the private key.

Record the public certificate's SHA-256 fingerprint on the machine that created it. Verify the first installer or bundle against that fingerprint through an independent local channel before opening it. A certificate supplied beside a download cannot enroll itself. Preserve normal OS security policy. A local signer does not confer Apple notarization or a public Windows publisher identity. The Mac daemon and Desktop still require the live role and image checks of the [existing image policy](m8-macos-release-image-policy.md).

For first-install verification, use the verifier from a trusted checkout. Keep two SHA-256 values from the signing machine: the public certificate's DER fingerprint and the release hash. On Windows the release hash covers the signed NSIS executable. Run `verify_first_install_windows.ps1 -Installer PATH -SignerSha256 PIN -ReleaseSha256 HASH` in PowerShell. On Mac the release hash covers the signed `Contents/MacOS/cryptex-desktop`. Run `python3 verify_first_install_mac.py PATH.app PIN HASH`. This checks the bundle seal and the signer of each architecture of Desktop and its daemon. Both [verifiers](../../scripts/m17/) perform inspection before installation. They leave certificate trust and user profiles untouched.

### Packaging Windows images with the owner's certificate

For the first-install exercise in the local Windows VM account `cryptexm14`, extract the trial ZIP and open `KUR.exe` as an ordinary user. The launcher carries the separate per-user NSIS installer and the reviewed unsigned image hashes. It requests trust for a seven-day, non-exportable code-signing key through the current user's Windows certificate stores. Check the displayed trial certificate name in the Windows confirmation dialog.

The launcher signs and verifies the installer before running it. It checks the installed Desktop and daemon against the unsigned build hashes before signing them in place and verifying both signatures. The test installs under `%LOCALAPPDATA%\Cryptex-M17-Ilk-Kurulum\Application`. A second run verifies the already signed images. The launcher writes `M17-IMZA-SONUC.json` beside itself. The receipt describes installation and file signatures. Application launch, personal profile enrollment and activation of sync are separate operations.

Build the reviewed checkout with `pnpm --filter @cryptex/desktop tauri:package-smoke`. Record the SHA-256 hashes of the approved release Desktop executable and staged daemon. Use PowerShell 7 on the signing machine with the code-signing identity available in the current user's personal certificate store.

```powershell
./scripts/m17/build_owner_windows_installer.ps1 CERTIFICATE_SHA256 DESKTOP_SHA256 DAEMON_SHA256 OUTPUT_INSTALLER.exe
```

The command makes private copies of both reviewed build products and uses the selected identity for signing. It verifies the resulting NSIS before publishing to the new output path and returns its SHA-256. The signing command leaves certificate enrollment to the owner. Confirm the release fingerprint independently on the receiving device using the first-install verifier above.

## Updates and rollback

Use the previously enrolled public certificate to authenticate the candidate and the retained installation. Bind the platform, architecture, release identity and package contents to the approved candidate before executing any update probe. The existing protocol check then tests compatibility. If activation fails, verify the retained installation before reopening it. A replacement certificate requires explicit owner verification through the same independent channel as the first install. A missing key or unverifiable retained image keeps the package inactive.

Keep signed application binaries separate from the Brain, OS credentials, backups and Recovery Kit. An ordinary uninstall removes the application and its startup registration. Erasure of personal data remains a distinct owner operation under the existing purge rules. Disposable signed update and reinstall tests preserved a signed note on both systems. The Windows NSIS test also verified removal of the matching login entry. First installation with the owner's own keys remains open.

The standalone updater takes the independently approved public certificate at build time through `CRYPTEX_UPDATE_SIGNER_DER`. A build without that certificate refuses admission. The `fingerprint` operation measures the installed directory. An attached CMS statement binds the full directory digest, OS and CPU architecture. The `admit` operation validates that statement and the two native executables before saving a receipt. The normal `apply` operation rechecks the selected and retained images against their receipts. The owner-selected digest identifies the requested update. It grants no signing authority. [Installed update evidence](../../evidence/m17/personal-packaging.md#signed-updates)

### Delivering the standalone updater

On each signing machine, set `CRYPTEX_UPDATE_SIGNER_DER` to the absolute path of the owner's public DER certificate for that OS. Build the reviewed checkout with `cargo build --release --locked -p cryptex-updater`. Record the SHA-256 of that unsigned executable. From the same checkout, package it with the matching OS signing identity.

```sh
python scripts/m17/package_owner_updater.py UPDATER UNSIGNED_SHA256 PUBLIC_DER CERTIFICATE_SHA256 NEW_ARCHIVE.tar.gz
```

The command signs a copy, verifies its native signature, and outputs the SHA-256 of an archive containing the signed program and its dependency notices. Compare that archive hash with the value recorded independently on the signing machine before extracting it on the receiving device. The updater's `signer-fingerprint` command reports the SHA-256 of the public certificate it uses to authenticate future packages. Packaging stops if the build lacks this pin or contains a different one. Run the updater outside the installed Desktop directory.

### Removing the installed application on Mac

Quit Cryptex first. From outside the application, use the standalone updater built with the reviewed public certificate. Supply the approved full-package digest from the signing machine.

```sh
cryptex-updater remove-installed-app APPROVED_PACKAGE_SHA256
```

The updater checks the signed receipt, application and daemon before asking macOS to move the installed bundle to Trash. If a Cryptex LaunchAgent file exists, its label, executable, arguments and run-at-login setting must exactly match this installation. The updater moves that file to Trash first. A different entry, an active Desktop or an unfinished update stops removal. If moving the application fails after the login file was moved, rerun the same command after resolving the reported OS error. This removes the registration for future logins. The command leaves the current launchd session alone.

The profile, keys, backups and inactive update packages stay in place. To reinstall, verify the signed application and its detached metadata using the first-install procedure. The owner can enable login startup again separately.

## macOS startup after installation

The controlled [development packager](../../scripts/m8/mac_lan_setup.py) signs two executables, copies the web assets into `Contents/Resources/frontend`, and writes the signed startup inventory into Resources. The ordinary [Tauri package](../../apps/desktop/src-tauri/tauri.bundle.conf.json) seals the complete bundle and embeds the frontend. Copying the development inventory into the finished application changes sealed resources. Generating it before the seal would record the desktop image before its final signature.

The sealed installation rehearsal generates a signed inventory after signing the bundle. A protected copy of the frontend is included before signing. Desktop and the daemon verify the detached inventory against the independently provisioned certificate, source revision and architecture, and authenticate each live peer against the [verified snapshot and role hashes](../../services/cryptexd/src/m8_owner_channel/release_inventory.rs). The [ordinary window launch](https://github.com/gungortarik/cryptex/actions/runs/45276597822) used a fresh profile and verified its synthetic note on exit. The feature refuses an existing-profile launch.

The receiving machine stages the signed inventory for each image. The updater probes the selected image against its own inventory and reconnects to the retained image on rollback. [Installed transaction results](../../evidence/m17/personal-packaging.md#sealed-macos-update-and-rollback). The owner installation and release review remain outstanding.

### Building the Mac delivery

Use the approved build, the public DER certificate and its independently recorded SHA-256. The trusted updater's `fingerprint` command measures the entire input application. On the signing Mac the following command writes a new delivery directory using the corresponding Keychain identity.

```sh
python3 scripts/m17/build_owner_mac_package.py APP APPROVED_INPUT_SHA256 SOURCE_COMMIT PUBLIC_DER CERTIFICATE_SHA256 NEW_DELIVERY_DIRECTORY --updater TRUSTED_UPDATER --mac-helper HELPER
```

The directory contains the signed `Cryptex.app`, its signed startup inventory, the updater receipt `update.cms`, and `release.json` with the resulting fingerprints to record on the signing machine. The receiving device verifies the image and stages the inventory before update. The current sealed build opens temporary profiles.

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

### Proposed delivery and version policy for the two personal devices

The [release contract](../27_RELEASE_DISTRIBUTION_AND_SIGNING.md#17-explicitly-unresolved) leaves the channel, formats, signing, update hosting, version numbers and uninstall experience to the release decision. The owner has confirmed the audience. The choices below are a proposal for that decision.

| Owner action | Proposed behavior |
| --- | --- |
| Bring the first build to each device | Transfer the Windows per-user NSIS installer or the Mac delivery directory over an owner-controlled local transfer. The Mac directory includes the sealed app, detached inventory and receipts. Obtain the expected certificate and release fingerprints independently from the signing device and follow the first-install procedure above. |
| Establish publisher trust | Use the local signing identity for each platform. On Windows enroll the public certificate for the receiving user. On macOS verify the independently pinned signer and signed inventory for Desktop and its daemon. This audience uses local signing without a paid publisher or notarization service. |
| Receive an update | Use the same local transfer for the signed update and its receipt. The owner initiates application of that package. This delivery method works offline and needs no update host. The standalone updater checks the approved digest, signer, platform and compatibility, and preserves the previous working installation for rollback. |
| Identify an update | Use the same three-part numeric release version for both platforms. Assign the first owner candidate `1.0.1`; increment the final component for each subsequent candidate in that series, including a rebuild for changed release inputs. Record the source revision, platform-specific package hash and signing certificate fingerprint alongside that number. A version label alone grants no update authority. |
| Remove Cryptex | Use the per-user uninstaller on Windows or the verified Mac removal command above. The application and its matching login registration are removed. Keep the Brain, keys and backups available for reinstall. Erasing memory requires the existing separate purge process. |

The current [Desktop configuration](../../apps/desktop/src-tauri/tauri.conf.json) still declares `0.0.0`. When the owner accepts a release numbering policy, check the version shown by each installed package against the approved release record. Leave the existing M16 personal-use hold in effect until the corrected official transport passes verification. Approval of a disposable Windows signing exercise authorizes only that exercise.

Private CI jobs use disposable identities and synthetic data. Their certificates expire or are discarded. The release decision requires a verified owner-controlled installation on both target systems, the beta checklist and explicit approval. The [transport hold](../../evidence/m16/hardening.md#personal-use-release-hold) remains in force independently of package signing.
