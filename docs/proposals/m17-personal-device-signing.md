# M17 signing for the owner's Mac and Windows VM

Status: proposed release profile. The owner chose these two devices as the audience. The disposable signing tests exercise native package formats. Enrollment of a long-lived signing key and use of personal data require separate owner action.

## First installation

Choose the per-user NSIS installer on Windows and the application bundle on macOS. Each platform uses its own code-signing identity. Generate the private key on an owner-controlled machine and keep it in the platform's protected key store. Release signing is separate from note signing and Recovery Kit approval. No service, installer or updater receives the private key.

Record the public certificate's SHA-256 fingerprint on the machine that created it. Verify the first installer or bundle against that fingerprint through an independent local channel before opening it. A certificate supplied beside a download cannot enroll itself. Preserve normal OS security policy. A local signer does not confer Apple notarization or a public Windows publisher identity. The Mac daemon and Desktop still require the live role and image checks of the [existing image policy](m8-macos-release-image-policy.md).

## Updates and rollback

Use the previously enrolled public certificate to authenticate the candidate and the retained installation. Bind the platform, architecture, release identity and package contents to the approved candidate before executing any update probe. The existing protocol check then tests compatibility. If activation fails, verify the retained installation before reopening it. A replacement certificate requires explicit owner verification through the same independent channel as the first install. A missing key or unverifiable retained image keeps the package inactive.

Keep signed application binaries separate from the Brain, OS credentials, backups and Recovery Kit. An ordinary uninstall removes the application and its startup registration. Erasure of personal data remains a distinct owner operation under the existing purge rules. Release tests still need to exercise the complete signed update and uninstall paths.

The standalone updater takes the independently approved public certificate at build time through `CRYPTEX_UPDATE_SIGNER_DER`. A build without that certificate refuses admission. The `fingerprint` operation measures the installed directory. An attached CMS statement binds the full directory digest, OS and CPU architecture. The `admit` operation validates that statement and the two native executables before saving a receipt. The normal `apply` operation rechecks the selected and retained images against their receipts. The owner-selected digest identifies the requested update. It grants no signing authority. [Installed update evidence](../../evidence/m17/personal-packaging.md#signed-updates)

## Scope and release decision

Private CI jobs use disposable identities and synthetic data. Their certificates expire or are discarded. The release decision requires a verified owner-controlled installation on both target systems, the beta checklist and explicit approval. The [transport hold](../../evidence/m16/hardening.md#personal-use-release-hold) remains in force independently of package signing.
