# M17 personal-device packages

The owner selected their own Mac and Windows VM as the audience. The packaging rehearsal uses a macOS app bundle and a Windows per-user NSIS installer. Apple notarization and public publisher credentials would be separate distribution decisions. The [release requirements](../../docs/27_RELEASE_DISTRIBUTION_AND_SIGNING.md) and the [current two-device transport decision](../../docs/proposals/m17-personal-device-signing.md#owner-acceptance-of-syncthing-217) apply.

## Package inventory

At `207dd15`, both native installations started Desktop and the packaged daemon and compared the frontend and Unicode notice bytes with the inputs. [Installed release smoke](https://github.com/gungortarik/cryptex/actions/runs/45249384781)

The Rust inventory includes the locked normal and build dependencies for Desktop with Tauri's custom protocol and the packaged daemon. It records package names, versions, declared licenses and document hashes. Each document is included once. For registry packages missing their repository-level terms, supplemental notices are associated with the exact package version and a source URL in [the inventory](../../third_party/rust-notices/sources.json). Sources include repository-level terms for objc2's Apple SDK bindings. Embedded third-party code and the managed transport require their own notice review.

The native inventory includes the source notice from the bundled SQLite 3.54.1 header in `libsqlite3-sys 0.39.1`. On Windows it also includes both notice files from Microsoft's WebView2 SDK 1.0.3800.47. The three static loader libraries in `webview2-com-sys 0.39.1` matched the corresponding x64, x86 and Arm64 entries in the upstream NuGet archive byte for byte. The archive SHA-256 was `56c9f26bdd07916a2d1949fb58a5c7e434dfa1173577dca879206050c4e718db`. [Upstream SDK package](https://www.nuget.org/packages/Microsoft.Web.WebView2/1.0.3800.47)

## Signing rehearsal

Disposable hosted accounts create temporary code-signing identities. The Mac test signs the daemon and then the app, copies the app to an installation directory, checks both roles against the provisioned public certificate and checks the bundle seal. The Windows test requests Authenticode for the actual installer, verifies the signer before installation and checks the installed Desktop and daemon. Both tests include wrong-signer and altered-content controls. The workflow exposes test results only.

At `784cfba`, both native jobs passed. The Windows runner verified the NSIS installer before execution and checked the installed Desktop and daemon. The Mac runner verified both roles against its independent DER pin after copying the signed bundle. Both compared the installed Rust notice inventory to its input, refused the wrong signer and detected a changed file. Mac also checked cleanup of the disposable signing identity. [Windows and macOS test results](https://github.com/gungortarik/cryptex/actions/runs/45253534985)

Before a personal install, the owner needs to verify the release signer's fingerprint through an independent local channel. The final packaging and updater review must cover the installed payload and the previously working version used for rollback. CI fixture identities are discarded at the end of each job. No real-profile installation or key enrollment has taken place in these rehearsals.

At `f86b598` both native signing jobs checked a first installation against an independently supplied public certificate fingerprint and release hash before launch. Windows verified the signed NSIS executable. Mac verified the signed Desktop executable and the sealed application bundle. Both rejected a different signer, a wrong release hash and changed package contents. [First-install verification](https://github.com/gungortarik/cryptex/actions/runs/45260525337)

At `8e59c63` both native jobs also opened the provenance resource from the installed application and matched it against an independent checkout. Checks cover the clean source commit, target triple, tool versions and hashes of the five locked build inputs. The CI identities and synthetic installation directories are disposable. [Installed build provenance on Windows and Mac](https://github.com/gungortarik/cryptex/actions/runs/45261329297)

## Signed updates

At `aae6626` the installed updater on Windows x64 and Mac arm64 admitted three independently signed fixture images. It checked a CMS statement covering the OS, architecture and SHA-256 digest of the complete installed image, then checked the platform signatures of Desktop and its daemon against the certificate compiled into the updater. A receipt from another signer and a receipt for a different image failed admission. A substituted receipt blocked both a fresh update and reopening the retained image after interruption.

Both native reports recorded a compatible update, rollback on protocol mismatch, recovery after interruption and rollback on timeout. Every reopened Desktop connected to its own daemon and the synthetic signed note still verified. The private CI keys belonged to disposable signing accounts. [Signed update and rollback reports](https://github.com/gungortarik/cryptex/actions/runs/45256804724)

At `53dbe05` both platforms repeated those four cases and returned to the working image when the candidate exited before reporting health. All five transactions verified the same signed note. On Windows the standard account ran the NSIS uninstaller and reinstalled the working version. On Mac the temporary account moved the installed application bundle out of Applications and reinstalled the working bundle. On both systems the reinstalled image passed admission and verified the original note and its unchanged signed pack. [Update and reinstall reports](https://github.com/gungortarik/cryptex/actions/runs/45257477174)

Both [signed installation jobs at `55517f1`](https://github.com/gungortarik/cryptex/actions/runs/45257927925) passed with the updated embedded dependency notices.

The standalone updater archives at `45d2855` each contain the signed executable and its own native dependency notice inventory. Reading the two uploaded archives confirmed the SHA-256 of every referenced notice for 44 locked Windows packages and 37 Mac packages. Both installed update rehearsals passed. [Native updater archives and tests](https://github.com/gungortarik/cryptex/actions/runs/45264154857)

At `03857d4` the real per-user Windows NSIS uninstaller removed its binaries and the matching seeded Cryptex startup registration, preserving an adjacent unrelated value in a disposable account. M14 separately verified startup registration at login. [Windows removal result](https://github.com/gungortarik/cryptex/actions/runs/45264593092/job/138008025382)

## Update probe shutdown

In the Windows install at `2f6e8c9`, the candidate's service took longer than the interactive three-second stop window. Desktop force-stopped it and returned a probe failure in place of the expected protocol-mismatch reply. The updater restored the working image. The log does not identify which internal service operation consumed those three seconds. [Windows failure](https://github.com/gungortarik/cryptex/actions/runs/45260956476/job/137995247047)

At `3b0fd56` the updater still enforces one 30-second deadline on the probe and its process tree. The update path allows the owned service to drain within that deadline and requires a successful exit and release of the instance lock before reporting health. A regression test exercised a child that completed cleanup four seconds after receiving its stop request. Both native jobs passed it. On both platforms the five installed update/recovery transactions verified the original signed note, including after reinstallation. [Windows and macOS receipts](https://github.com/gungortarik/cryptex/actions/runs/45263639901)

## Official transport inspection

The [two-platform binary inspection](https://github.com/gungortarik/cryptex/actions/runs/45252879619) at `bc24732` matched the published archive digests for official version 2.1.7. Both binaries report Go 1.27.3 and the same four in-range findings recorded in M16. The report also contains the historical GO-2022-0888 finding, whose affected range predates this version. Inspection downloads the executables to temporary storage, performs static analysis and leaves the installed transport untouched. The job can be dispatched again when upstream publishes a stable release. Its success records a completed inspection. Decisions about personal use remain with the owner under the M16 hold.

The [repeat inspection at `0c94466`](https://github.com/gungortarik/cryptex/actions/runs/45258102534) recorded the exact upstream license and author file digests for both archives and returned the same four in-range findings. Verification of a corrected release, its dependency notices, native compatibility and rollback remains outstanding.

Inspection at `2512599` matched 60 linked Go modules per executable, with 62 distinct module/version pairs across Windows x64 and Mac arm64. The independently inspected artifacts contained 62 distinct notice documents after deduplication. All document SHA-256 digests matched the decoded bytes. Each module had a notice, both archive hashes matched the binary inspection, and the set included the pinned Go license, Go patent grant and original Syncthing About attribution. Repeat inventory against the corrected release before distribution. [Linked notices and executable inspection](https://github.com/gungortarik/cryptex/actions/runs/45262814860)

## Signing an owner-approved update receipt

On the signing machine, run `python scripts/m17/sign_owner_update.py UPDATER PACKAGE APPROVED_SHA256 CERTIFICATE_SHA256 OUTPUT` from a reviewed checkout. Use the trusted local updater. The approved digest comes from the reviewed, complete installed package, and the certificate digest is the independently pinned SHA-256 of the public DER certificate. The updater recomputes the package digest before it emits the exact platform-bound bytes used by release admission. The signer publishes a new receipt on the same local filesystem only after signing succeeds.

Windows uses PowerShell 7 and a matching private signing identity in the current user's certificate store. On macOS pass `--mac-helper PATH` for the reviewed Keychain CMS helper built from `scripts/m8/mac_owner_cms.c` and the daemon's native inventory verifier. Its key access belongs to the owner. The receiving updater verifies the receipt and the signed Desktop and daemon roles again before admission. Disposable CI signing identities are scoped to rehearsal.

The first Windows rehearsal at `7e93596` exposed a file-permission error when a second account read the valid receipt. A hard link preserved the private signing directory's ACL. The signer now stages the public receipt in its destination directory and publishes without replacing an existing file. The standard account could then read the valid receipts and reject foreign ones. [Original run](https://github.com/gungortarik/cryptex/actions/runs/45269266820)

One Mac rehearsal at `3eaf341` hit the 110-second outer test deadline during the deliberate timeout and rollback. The unchanged test completed on retry. That failure establishes an elapsed-time overrun; its log does not identify a single slow operation. The rehearsal had been packaging a debug updater. At `28c92b8` it packages, signs, and exercises the optimized release executable with the same deadlines. [Earlier run and retry](https://github.com/gungortarik/cryptex/actions/runs/45270653931)

Both native jobs at `28c92b8` passed all five installed update and recovery transactions. The complete timeout/rollback operation took 33.48 seconds on Windows and 34.77 seconds on macOS. The timed-out probe and its service exited, the unrelated process survived, and the signed note was verified after reinstall. The negative signing controls rejected a different approved digest, an absent signer and overwriting an existing receipt. The downloaded updater archives contained the signed executable and notices covering 44 Windows and 37 Mac packages. All 30 and 35 distinct notice document hashes respectively matched their contents. [Native jobs and artifacts](https://github.com/gungortarik/cryptex/actions/runs/45273508734)

At `8d2bc80` Windows returned OS error 5 (access denied) from `apply` during the installed update suite. The first attempt did not identify the filesystem operation. The same installed update and rollback step passed on the second attempt at the unchanged revision. The Mac job passed on its first attempt. The intermittent Windows failure remains unexplained. [Windows attempts and Mac result](https://github.com/gungortarik/cryptex/actions/runs/45276597754)

## Installed macOS owner session

The rehearsal at `857593b` installed a sealed Tauri bundle in a disposable hosted Mac account. Desktop verified its bundled daemon over the native channel at startup and again after restarting the daemon. The old channel was refused. The protected frontend opened, and the service verified the synthetic signed note after closing the window. A stale release inventory and a re-signed daemon carrying an unrelated identifier were both refused. The latter used the same certificate as the approved daemon. [Native test](https://github.com/gungortarik/cryptex/actions/runs/45276000666)

At `8d3134e` the runner opened the installed application with no arguments, verified its signed fixture note on window exit and checked temporary profile cleanup. It installed two sealed builds from the same revision and signing identity. A signed resource change selected a different detached inventory. The candidate refused the old inventory, connected with its own, and the restored original again connected with its original inventory. The script moved the package slots directly for this check. [Native test and build](https://github.com/gungortarik/cryptex/actions/runs/45277923377)

The rehearsal feature accepts temporary profiles. The following native updater checks extend the earlier direct slot replacement. Owner installation and authorization for personal data remain open.

## Sealed macOS update and rollback

At `577ee0f`, the installed Mac updater admitted valid owner receipts for two sealed packages in a disposable account. The staged package had its own valid receipt but its detached inventory was withheld. The candidate probe failed, the updater verified the previous package and reconnected to its service, and AppKit showed the original executable's window. After the signed inventory became available, the same updater promoted the candidate and reopened its window. The test checked the selected full-package SHA-256, the updater's child PID and the visible window's owning process on both paths. [Mac updater run](https://github.com/gungortarik/cryptex/actions/runs/45282276550)

The sealed probe uses the authenticated Mach peer and the retained child PID, verifies the instance lock in a fresh temporary runtime, then checks the IPC health response against that same process. The Windows host's Application Control blocked a local Tauri build script (OS error 4551). The unchanged clean-stop test and the existing installed update suite ran successfully in both hosted native jobs. [Windows and Mac update checks](https://github.com/gungortarik/cryptex/actions/runs/45282276507)

The signer at `6019699` prepared the detached inventory from a private copy of the approved app with a disposable Keychain identity. It refused an existing slot and refused a daemon carrying a different signed identifier. The exercise then independently checked receive-side refusal of a deliberately signed wrong-role fixture. [Mac signing test](https://github.com/gungortarik/cryptex/actions/runs/45282769101)

At `9feca3b` the receiving Mac accepted the same valid inventory twice and refused both a damaged CMS signature and an authentic inventory for a different build. Neither refusal occupied the slot. The signing machine also refused a mismatched source revision and an output path inside the sealed application. The subsequent native updater transactions reopened the original and candidate windows as expected. [Mac preparation, delivery and update](https://github.com/gungortarik/cryptex/actions/runs/45284390051)

At `bf98cb7` the [Mac delivery command](../../scripts/m17/build_owner_mac_package.py) used a disposable Keychain identity to sign private copies of the approved executable images, generated the detached startup inventory, and signed the full installed-package receipt. It refused the wrong input hash and preserved the unsigned input. The receiving rehearsal used the delivered application, signed inventory and update receipt for the original installation and rollback. [Native packaging and update](https://github.com/gungortarik/cryptex/actions/runs/45285474274)

## Packaging the Windows installer with an existing identity

At `018f3f7`, Tauri lost the drive prefix of an absolute sidecar path. The Windows signing step failed before installation when the runner checked out the project on a different drive from its temporary files. At `b890699` the [packaging command](../../scripts/m17/build_owner_windows_installer.ps1) used a unique path relative to the Tauri manifest for the signed daemon and removed it after bundling. The original reviewed executable inputs retained their hashes. [Failing Windows run](https://github.com/gungortarik/cryptex/actions/runs/45284332989)

The corrected run built a per-user NSIS installer with a disposable certificate. It rejected a mismatched input digest and missing signing identity before publication. The installed Desktop and daemon matched the certificate pin. A modified daemon and a foreign signer were refused. The ordinary uninstaller removed both binaries and its startup entry while preserving an unrelated entry. The companion Mac packaging job passed as well. These jobs used hosted accounts. [Native Windows and Mac jobs](https://github.com/gungortarik/cryptex/actions/runs/45286355369)

## Dependency and renderer review at 40dea0f

On November 23, the [registry query](../../scripts/m16/check_rust_advisories.py) compared 727 public Rust crate/version pairs with OSV. None of the affected versions were in the normal or build dependency trees for the Desktop, daemon or updater on Windows x64 or Mac arm64. The separate Windows codec trial still resolves an affected `crypto-bigint 0.5.5`. Seven other affected entries were outside the inspected product and codec graphs. The release diff from M16 adds the existing `cc` build dependency to the updater in the workspace lockfile.

The production pnpm workspace audit and npm audits of the local MCP and backup tools reported zero known advisories. The 27 capture, connection and status policy tests passed on Windows with the pinned Node 24.19.0. The Desktop capability files and CSP are unchanged from M16. The new removal operation is a standalone native updater command with a signed-package check and no renderer binding.

These checks cover the listed code and dependency graphs at this revision. The [official transport hold](../m16/hardening.md#personal-use-release-hold) still applies separately to the bundled Go executable.

## Removing an approved Mac installation

The [installed updater run](https://github.com/gungortarik/cryptex/actions/runs/45289049570) at `65d4043` exercised the Mac removal command in a disposable account. It refused a wrong approved digest, an unrelated login plist, a busy Desktop session lock, and a pending update journal. For the matching installation macOS moved the application and matching LaunchAgent file to Trash. The adjacent unrelated plist retained its original bytes. The test reinstalled the signed application and verified the original signed note and its recorded hashes. It repeated removal and reinstall with login startup already disabled. The Windows standard-user update and reinstall suite passed at the same revision.

The Mac assertions cover the persistent login file and binaries. They do not measure a new OS login or unload an already registered launchd job in the current session. [Owner removal procedure](../../docs/proposals/m17-personal-device-signing.md#removing-the-installed-application-on-mac)

## Standalone updater delivery from the owner's signing machine

At `40dea0f` the [packaging command](../../scripts/m17/package_owner_updater.py) ran on native Windows and macOS with disposable certificate-store and Keychain identities. It refused a mismatched public certificate, unapproved executable hash and an updater built to trust a different signer. Both native suites extracted the signed updater from the resulting archive and used that executable for the five update and rollback scenarios. The signed test note remained verifiable after reinstall. The [sealed Mac run](https://github.com/gungortarik/cryptex/actions/runs/45289817066) also passed for this revision. The [companion first-install jobs](https://github.com/gungortarik/cryptex/actions/runs/45289817231) verified the installed signatures on both platforms.

The two archived executables and notices were downloaded from the [native run](https://github.com/gungortarik/cryptex/actions/runs/45289817134). Both archive digests matched the signing-job logs. Every embedded notice document matched its recorded SHA-256.

| Native target | Archive SHA-256 | Packages | Distinct notice documents |
| --- | --- | ---: | ---: |
| Windows x64 | `93fa8c1b6ce6a69af3f883ade727925083da1239e93ae86a5d9e1dd37dbe8723` | 44 | 30 |
| macOS arm64 | `a76ac5d88bb609004efffccdbf87d5be393851a92a88e3860da2e2c4bb5c190d` | 37 | 35 |

The owner package procedure produces fresh archives with independently enrolled local keys. The hashes above identify only these disposable CI artifacts.

## Search after a normal Mac quit and reopen

At `109490a`, the [persistent-profile job](https://github.com/gungortarik/cryptex/actions/runs/45292862528/job/138105777718) signed and installed the Mac application and independently verified its startup inventory. The installed daemon seeded one public synthetic note in a fresh directory using a disposable Keychain. Each ordinary Desktop launch selected the same certificate-specific temporary directory. The protected WebView submitted the normal Search form and observed the note text in a visible result before AppKit requested quit.

The harness compared the canonical file and public identity hashes and the signed-probe descriptor after both exits. A missing profile was refused without creating a replacement. With the descriptor removed, startup was refused and the canonical baseline was preserved. The [fresh-profile job](https://github.com/gungortarik/cryptex/actions/runs/45292862528/job/138105777389) also passed. Existing-profile selection in this build is limited to the disposable trial feature. Owner enrollment, persistent personal profile selection and authorization to import personal data remain open.

## Distribution notices for the inspected transport

At `04df1d5` the [official archive inspection](https://github.com/gungortarik/cryptex/actions/runs/45297008924) produced two UTF-8 notice files for Syncthing v2.1.7. An independent read of the original ZIP members confirmed the archive and executable digests and the exact LICENSE and AUTHORS text in each notice. The module inventories matched the linked dependencies per executable. Every linked module supplied notice text. Each file preserved the inspected document bytes and included the upstream credits and Go license and patent grant.

| Target | Linked modules | Executable SHA-256 | Notice SHA-256 |
| --- | ---: | --- | --- |
| Windows x64 | 60 | `cd9aea20afda2b9bdba5aff2e6ac964ed5c2e741ed4ceb48ad9e1be096ca7891` | `f795a39e93046c3c8cc625432da6c902e12dfad0971527155c923576fbd34275` |
| macOS arm64 | 60 | `38aab77073e47a717ec59eab082670b4fccf28c256fa0f7935aebb3f4665bb01` | `9aa871c76c7744728ad6d6ae9f422a68e712ad297e2b4ae89e5a9d7cb6bd8200` |

The four in-range advisories still hold personal use. The final installer needs verification against the selected corrected official binary and its corresponding notice.

At `43147e4` the [official inspection](https://github.com/gungortarik/cryptex/actions/runs/45501128250) also collected module author credits and patent grants. The generator refuses distribution if a linked module lacks a primary notice, and the workflow retains the diagnostic inventory. Both downloaded v2.1.7 archive digests matched the official release. Independent inspection found all 62 distinct linked-module documents for each target in its own rendered file, with their exact bytes and module labels. Each target still had 60 linked modules. The resulting notice SHA-256 was `8603658416888cc528e67681f60aa570cf8d85a5edc81b74be683c2bfd61569e` for Windows x64 and `e69e8838b92f33b3e6b6e140104bd84e410b72a93e773246be5eedc36345bc4b` for macOS arm64.

## Windows VM first-install handoff

The [Windows job](https://github.com/gungortarik/cryptex/actions/runs/45336980708) delivered the launcher from `e89e4b7` with NSIS input from `f4d7159`. Before embedding the Desktop hash it verified that normalization of the single NSIS bundle marker gave the original reviewed build digest. The daemon matched its original build hash directly.

A disposable local standard user named `cryptexm14` signed the installer, installed Desktop and the daemon into a separate application directory, signed the two verified installed images, and validated all three signatures. Running the same launcher again preserved both installed file hashes. The CI runner pretrusted the test user's public leaf to avoid an interactive Windows dialog. In the owner's VM the launcher requests trust through the current-user store and Windows can ask the user to confirm. Only the VM receipt can establish that this step and installation succeeded on the owner's device.

The ZIP from the successful run contains only `KUR.exe`. Its SHA-256 is `1420174bee3215c0459330f8a054ed412266de5c0ec7ccf9d48bb9f37e9648d2`. The executable SHA-256 is `8017cd3cb173a09f1fda84e6bc8ef505d69134ad6109ef7d94c4df1c7ecafca0`. The trial covers first installation and file verification. Personal profile access and the transport hold remain separate release decisions.

### Owner VM result

On November 24, 2026 the owner supplied the Windows trial receipt `M17-IMZA-SONUC.json` (file SHA-256 `876fde8fee588798f6585a749f9f7cc851affe4619345e777a1114f1dff35bde`). Its revision matched the pinned installer input `f4d715978c1a074c99292773028df8b513fd1e2d`, its result was `passed: true`, and it contained four distinct, correctly formatted SHA-256 values for the certificate, signed installer, installed Desktop and installed daemon. The launcher checks the native Authenticode signatures against the selected certificate before reporting success. This owner-supplied JSON records the local result; independent inspection of the VM binaries, launching the application, and personal profile access were outside this exercise.

## Mac owner first-install kit

The [macOS 26 arm64 build](https://github.com/gungortarik/cryptex/actions/runs/45485652498) verified source revision `e90c2abb597af8c78f7643d6641d82f6f86f26b5`. The application archive SHA-256 is `5d691d19942f9fd9a676fcff98d49ad8e77d0d8e8c73bacb713cda370f9a1f09`. The [kit test](https://github.com/gungortarik/cryptex/actions/runs/45488028688) produced delivery ZIP SHA-256 `13f393c0e6beca4597d0964c3d65f15beeb0e890c89fb4bc8a8b18c5f8fe71a4` at `742ba21`.

In a hosted standard account named `cryptexm14`, the test enrolled a seven-day signing identity in a disposable Keychain and authorized the native signing tool for that test key. It installed the app, independently checked both installed signatures, and compared first and repeat receipts. The launcher refused another account, a changed input archive and a modified installed resource. On the owner's Mac the same enrollment uses the account's default Keychain and macOS can prompt for signing permission. The installed app is under `~/Applications/Cryptex-M17-Ilk-Kurulum/`. The installer verifies the files without launching the app.

### Owner Mac result

On November 24, 2026 the owner supplied `M17-IMZA-SONUC-Darwin.json` (file SHA-256 `8778f1b4ea752736776048104ab6b238fca0598fafefef75763983833678172f`). It reported `passed: true`. Rehashing the delivered ZIP and its embedded application archive and reading the packaged installer's source revision confirmed the receipt matched this kit. The local certificate, Desktop and daemon fields contained three distinct SHA-256 fingerprints. The launcher verified the installed signatures on the Mac before issuing the receipt. This review examined the receipt and delivery ZIP. Application launch and personal-profile use fall outside this trial.

## First numbered owner candidate

On November 24, 2026 the owner approved version `1.0.1` and the [local delivery and removal policy](../../docs/proposals/m17-personal-device-signing.md#accepted-delivery-and-version-policy-for-the-two-personal-devices). The physical-device receipts above apply to the earlier first-install kits.

At `8d393d0`, the OSV check covered 727 public Rust versions and found zero affected versions in the Desktop, daemon and updater normal/build graphs on either target. The separate Windows backup codec trial still resolves [affected crypto-bigint 0.5.5](https://rustsec.org/advisories/RUSTSEC-2026-0387.html) through Sequoia 2.4.2 and cx448. The production pnpm audit and both standalone npm audits (local MCP and backup trial) returned zero known findings. The renderer source and capability files match M16 and the only change to the base Tauri configuration since M16 is the version number.

The upstream [latest stable release](https://github.com/syncthing/syncthing/releases/tag/v2.1.7) still supplied identical archive SHA-256 values to the [previous binary inspection](https://github.com/gungortarik/cryptex/actions/runs/45258102534) on November 24. Windows x64 matched `3ce7da770f546a8d02b65b2e7ee31f2a3b8b7c3f990ede045590cca4172ecfe1` and Mac arm64 matched `2df075b463231a307c91da657157beaed06383a941ba7ea04d33d536e3c4af38`. The personal-use hold continues. The [Obsidian API request](https://forum.obsidian.md/t/api-request-physically-contained-vault-operations-for-managed-paths/119512) still showed only the original post on the same date. The API repository's HEAD matched the previously reviewed revision `737629a51f6b4d30f8336fa3d38f3df006e322d4`.

The [first version check](https://github.com/gungortarik/cryptex/actions/runs/45496987716) observed `1.0.1` in the installed Windows executable, its signed installer, and the installed Mac bundle. The [sealed Mac delivery](https://github.com/gungortarik/cryptex/actions/runs/45496987996) checked the signed bundle's release record in ephemeral and persistent disposable profiles.

At `8d393d0` the [signed startup run](https://github.com/gungortarik/cryptex/actions/runs/45497413994) installed and started the ordinary signed builds on both platforms. The Windows installer ran in a fresh standard account. Both startup receipts recorded distinct Desktop and daemon PIDs, a live daemon across window hide/show, refusal of an unexpected sidecar from both inputs, and release of the daemon lock on normal exit. The installed notice bytes matched their reviewed build inputs. The local Windows run passed all 29 renderer-policy tests with Node 24.19.0 and all eight M17 transport inspection/notice tests. These exercises used disposable identities. Release signer enrollment and the corrected transport remain open.

### Source verification before signing

At `a3711ec` the [Windows packaging job](https://github.com/gungortarik/cryptex/actions/runs/45501813060/job/138826762564) refused three build records before publishing an installer. The records named a different revision, declared a dirty source tree, or named a Mac build target. Restoring the original bytes allowed installation and startup. The package command held the verified provenance file open while NSIS bundled it. The [Mac packaging tests](https://github.com/gungortarik/cryptex/actions/runs/45501812934) refused the correct application fingerprint paired with a different source revision before signing. Both temporary profile scenarios passed with the matching revision. The persistent-profile test found the original synthetic note after reopening.

Both [signed startup receipts](https://github.com/gungortarik/cryptex/actions/runs/45501813060) recorded distinct live Desktop and daemon processes, a restored window, refusal of foreign daemon selections and release of the daemon lock after quit. A [separate native shell and display run](https://github.com/gungortarik/cryptex/actions/runs/45502020093) passed the native WebView checks and produced six passing keyboard results per platform, covering light and dark appearance at 100%, 150% and 200% simulated density. The Windows light and macOS dark 200% captures both showed the search field focus and the full Search button. All three runs used disposable profiles at the same source revision.

## Installed Syncthing in owner candidate 1.0.2

At `f7f3050`, the owner release builds stage the unchanged executable from the pinned Windows x64 or Mac arm64 Syncthing 2.1.7 archive, its LICENSE and AUTHORS files, and the matching linked dependency notices. Installed-file verification compares all four digests and executes the installed binary to check its reported version. The release daemon requires the corresponding executable identity and REST version.

The [sealed Mac startup jobs](https://github.com/gungortarik/cryptex/actions/runs/45556329126) exercised both ephemeral and persistent test profiles. The enrolled profile started the managed transport through the installed service and found the signed synthetic note after reopening. The existing [signed updater regression tests](https://github.com/gungortarik/cryptex/actions/runs/45556329120) also passed on Windows and Mac.

The [first signed installer run](https://github.com/gungortarik/cryptex/actions/runs/45556329053) passed on Mac. Windows installed the signed package, enrolled the transport and started Desktop, then failed while cleaning up the protected fixture. Revision `954d7f1` moved that fixture under the disposable account's local app data so the existing account cleanup owns its lifecycle.

The [signed 1.0.2 installer run at `954d7f1`](https://github.com/gungortarik/cryptex/actions/runs/45557419491) passed on both platforms. Windows verified the installed executable and notices, enrolled the bundled transport under a standard account, started Desktop and removed the temporary account profile. Mac verified the installed files and completed signed startup.

These jobs used disposable profiles and signing identities. An installed upgrade from 1.0.1 with an existing 2.1.5 enrollment still needs a separate trial that preserves the device identity and signed note. The prior two-peer transport trial exercised the upstream executables directly.

## Renderer review for 1.0.2

At `bceab35` the renderer source tree is byte-identical to the reviewed M16 revision `d06eaf5`. The only change to the base Tauri configuration is the application version. The CSP keeps scripts local and permits renderer connections only to Tauri IPC. The configuration supplies no shell or filesystem capability to the webview. The ordinary Desktop invoke list is unchanged. Opening a generated note still goes through the Rust handler that validates the relative note target under the selected vault and constructs the Obsidian URI.

The additional Mac profile-trial command takes no arguments and prints a fixed observation marker. It is compiled only with `m17-sealed-profile-trial`. The independent updater remains a native executable outside the renderer invoke list. On Windows, all 34 tests in the capture, search, pairing, connection and native profile status suites passed with Node 24.19.0. The M7 managed-note write-back gate remains open.

## November 25 owner package review

The [installed updater run at `bceab35`](https://github.com/gungortarik/cryptex/actions/runs/45559939549) generated separate Windows and Mac receipts. Each recorded six transactions. A malformed health reply from the staged candidate caused the signed working image to be restored and reopened. Its reported PID matched the reopened Desktop, the daemon reconnected, and the original note still verified. The test also covered an interrupted activation, a timeout and a probe that exited with an error. A subsequent [Windows run at `0b6edd6`](https://github.com/gungortarik/cryptex/actions/runs/45560851067) reached the reopen check but produced no Desktop receipt within 60 seconds. The fixture now records the failing handshake, shutdown or lock-observation stage when its observer runs. The PID, version, connection, signature and timeout assertions remain in force.

At `0b6edd6` the [signed Windows installation](https://github.com/gungortarik/cryptex/actions/runs/45560851237) reopened history written by the daemon built from 1.0.1 revision `a8e1d3f`. A standard-user process verified the original event signature with both versions, compared canonical files and the public identity metadata, and enrolled the 1.0.2 transport with the original device credential. This profile started without an enrolled transport. A previously paired 2.1.5 transport still needs an installed migration and cold rollback trial. Its cached package, exchange binding, native database and OS credential are part of that test. Canonical events are outside the transport rollback.

The [upstream rebuild comparison](https://github.com/gungortarik/cryptex/actions/runs/45562637527) inspected the approved 2.1.7 archives on November 25 and regenerated the Windows x64 and Apple Silicon linked-dependency notices. Both archive and executable hashes matched the packaging pins. Both regenerated notice files matched the checked-in distribution bytes and their pinned digests. Both binaries had the same scanner IDs as the [earlier inspection](https://github.com/gungortarik/cryptex/actions/runs/45554396914). The source for these unmodified binaries is available at [upstream revision d6d0181](https://github.com/syncthing/syncthing/tree/d6d0181f7d466381c4b39ec8fd944c364f939b32).

The November 25 OSV query at `00d2eea` covered 727 public Rust package versions. Zero findings applied to the normal or build dependency graphs of Desktop, the daemon or the updater on either target. The separate Windows codec trial still matched RUSTSEC-2026-0387. The production pnpm audit and the local MCP and backup-trial npm audits returned zero known findings. This dependency review leaves the separately inspected Go binary findings in the owner's two-device decision.

The M16-to-candidate runtime diff adds package verification using the existing SHA-256 library and platform signature APIs. Windows uses WinVerifyTrust for the executable and CryptVerifyMessageSignature for the attached CMS content. Mac uses SecStaticCode and the existing Security.framework CMS verifier. Both compare the signer certificate to the independently provisioned DER bytes and verify the release statement for the package, OS and architecture. The downloaded updater receipts record refusal of a wrong signer and a foreign receipt on both platforms. No new cryptographic primitive was introduced.

The [repeat installed update run](https://github.com/gungortarik/cryptex/actions/runs/45562461238) passed all six scenarios on Windows and Mac. The [cross-platform restore rerun](https://github.com/gungortarik/cryptex/actions/runs/45560851222/attempts/2) completed both transfer directions. Its first attempt timed out while waiting for the other hosted runner's artifact.

At `ee04d14` the signed Mac updater gives the candidate one interactive Keychain check and verifies access from a fresh noninteractive daemon before committing. Three updater decision tests at `8c33eff` passed locally on Windows, including refusal, single-use access, and a candidate dropping the required credential check. In the [first installed Mac trial](https://github.com/gungortarik/cryptex/actions/runs/45565297623) an inaccessible credential triggered rollback and the original complete package digest matched. The window check then failed. The [repeat](https://github.com/gungortarik/cryptex/actions/runs/45565752589) stopped at the same check. The fixture used a different home for the updater than for Keychain enrollment. Using the [same disposable login account throughout](https://github.com/gungortarik/cryptex/actions/runs/45568216620) verified rollback and the original note. It also revealed that a grant for the device key alone could leave the new Desktop unable to open.

### Mac credential acceptance and rollback

At `d1592b6`, the fresh daemon verified the signed history and reopened any enrolled transport configuration with Keychain interaction disabled. The [Apple Silicon installed trial](https://github.com/gungortarik/cryptex/actions/runs/45571094635) passed three transactions against the same signed note and device identity.

| Candidate access in disposable Keychain | Selected application |
| --- | --- |
| No device signing key access | Restored working package |
| Device signing key access only | Restored working package |
| Device signing key and enrolled transport access | Updated package |

Each transaction checked the package digest, observed the exact reopened PID with a visible window, quit it and searched for the signed note in a fresh window. The public identity and complete canonical event inventory matched their original hashes after each transaction. The [general installed rollback suite](https://github.com/gungortarik/cryptex/actions/runs/45568216253) passed on Windows and Mac. The Keychain grants were provisioned for two exact disposable daemon hashes. The physical approval dialog on the owner's Mac remains untested.

### Historical Mac key and local diagnostics

At `1de87a6`, the [installed Mac profile check](https://github.com/gungortarik/cryptex/actions/runs/45563945777) seeded the note with the genuine 1.0.1 daemon and compared the public identity, signed descriptor and canonical event files after two separate 1.0.2 application launches. The disposable Keychain initially authorized only the creator's code hash. The fixture granted the exact 1.0.2 daemon, then both launches verified the original note. This check exercised a prepared permission. Native dialog behavior and package rollback have their own check.

The 1.0.1 Mac [bundle manifest](https://github.com/gungortarik/cryptex/blob/a8e1d3fe1fb4c31b370fec0baac649e94a982688/apps/desktop/src-tauri/tauri.bundle.conf.json) packaged the Desktop and daemon. Its enrolled-profile fixture created a signed note and reopened it without enrolling a transport. Reproducing an existing 2.1.5 pairing needs a previously transport-enabled package or fixture with that enrollment. A fresh 2.1.7 enrollment supplies different evidence.

Review of the M16-to-`8c33eff` runtime diff covered the twenty changed source files in Desktop, the daemon and the updater. New standard output includes fixed UI results, public package fingerprints, and bounded structured health responses. The signed package verification and protected profile check return fixed stage results. A key read stays inside the daemon. The updater's credential check carries the interactive setting and the verifier process ID. The existing public signer fingerprint identifies a certificate. The changed output paths do not serialize note bodies, Keychain item values or signing material. The final release still needs its installed log review.

### Mac update admission and Keychain access

The [Mac admission and rollback trial at `48acbee`](https://github.com/gungortarik/cryptex/actions/runs/45575472228) also covered an owner-signed receipt for the wrong application digest. Admission and apply both refused, both installed image digests and the signed note stayed intact, and the candidate gained no device-key grant. With the correct receipt, the same trial verified rollback on denial, rollback on missing transport access, and an unattended reopen with the original note when both grants were present. The fixture controlled the disposable Keychain grants. It did not measure the physical consent dialog.

### Previously enrolled Windows profile

The [standard-user installation at `150b35c`](https://github.com/gungortarik/cryptex/actions/runs/45574853280) used the 1.0.1 daemon to create a signed note and enroll official Syncthing 2.1.5 in the same disposable profile. The installed 1.0.2 daemon returned `transport_enrollment_AlreadyExists` for a fresh enrollment request. Every file in the enrolled transport tree matched its pre-request SHA-256, including the engine, certificate, private key and native configuration. Both daemons verified the original note after the refusal. This covers local enrollment. Migration of an existing peer roster and database has a separate acceptance gate.

### Native paired profiles with the 2.1.7 release engine

At `bbd2571` the [native compatibility workflow](https://github.com/gungortarik/cryptex/actions/runs/45576506411) passed all four jobs. The Windows standard account ran the ordinary Cryptex connection owners: two peers paused, saved notes offline, resumed, exchanged them and replayed after restart. Restriction and retirement also propagated. On Apple Silicon the signed Desktop and daemon paired disposable profiles through the authenticated helper, reopened the transport, applied an endpoint change and verified pause/resume. Each platform used the owner release engine pins. The two additional jobs exercised the underlying engine upgrade and cold rollback. All profiles in the Cryptex controller trials were created for the test. Running the installed updater over a previously paired 2.1.5 profile remains open.


### Previously paired exchange journal across release builds

The [Windows x64 trial](https://github.com/gungortarik/cryptex/actions/runs/45583416688/job/139098856761) and [Apple Silicon trial](https://github.com/gungortarik/cryptex/actions/runs/45583416688/job/139098856672) passed at `64240e2`. Each runner built the 1.0.1 service source at `a8e1d3f` and the candidate source independently. A test-only driver created a disposable pair with a signed history and let the 1.0.1 exchange installer write its own 2.1.5 binding. Each subsequent process verified the histories against the original public anchor.

The normal candidate verifier refused the old binding. The migration component reconstructed the expected old configuration and peer binding from verified authority, then produced the three derived 2.1.7 files in a separate private directory. The candidate read them at the original enrolled path. After the test restored the retained directory, the original 1.0.1 verifier reopened the binding. SHA-256 inventories of that directory and both signed histories matched the originals. The private signing keys used to create the synthetic history lived only in the first test process. These trials exercised the exchange persistence code. Cutover of the complete native database and package through the installed updater is still pending.

### Detached native database during an update

The [installed Windows candidate at `8d26450`](https://github.com/gungortarik/cryptex/actions/runs/45591106221/job/139125233947) refused its update health probe before starting the daemon against a 2.1.5 enrollment. Every file in the disposable runtime matched its original SHA-256 and the old service reopened the signed note. The first trial exposed different transport pins in the Desktop and sidecar builds. The release configuration now selects 2.1.7 in both.

The [Windows standard-account run](https://github.com/gungortarik/cryptex/actions/runs/45591653011/job/139127107964) and [Apple Silicon run](https://github.com/gungortarik/cryptex/actions/runs/45591653011/job/139127107988) verified two transitions at `3dabb92`. The two release readers reopened the paired history before and after changing the candidate's multi-page SQLite fixture. The restored database passed SQLite's integrity check and the protected files matched their original hashes.

Separately, two official Syncthing 2.1.5 processes paired and transferred a file. After a clean stop, Cryptex copied the receiving engine's closed SQLite database, device identity and native files into a private directory. The driver checked every entry against the stopped source, supplied the generated test configuration and selected the copy. The official 2.1.7 engine transferred data, restarted and transferred again. In the rejection case the driver stopped 2.1.7, restored the retained original directory, started 2.1.5 and transferred another file. The test driver selected the directories. A signed installed update still needs to coordinate the application, transport package and native directory across interruption.

At `5bb1416` the updater authenticates the retained package in place, restores it to the active installation and then checks the daemon connection. A retained or pending Desktop cannot acquire a probe session. The [installed Windows update rehearsal](https://github.com/gungortarik/cryptex/actions/runs/45595680254/job/139140978090) recorded a successful update, protocol rejection, recovery after process termination, timeout, failed probe and invalid reply. Each reopened window reported a connected daemon and the original note passed signature verification.

At `e94140e` the [Windows standard-account release reader](https://github.com/gungortarik/cryptex/actions/runs/45596384977/job/139143441958) recognized the original enrolled executable and both 1.0.1 notices. It refused a previous notice with one changed byte, refused 2.1.7 as a predecessor, and admitted only 2.1.7 to the current launcher. The same job passed the paired journal, native transfer and cold rollback trials. Windows 1.0.1 installed the repository's LF notice bytes. The Apple Silicon enrollment used the original archive notice bytes. The verifier checks each platform's historical digests.
## Interrupted selection of the native transport

At `dd41d85`, the Windows standard account and the disposable Apple Silicon account terminated the process after each of four directory moves during activation and restoration. Recovery selected the previous pinned executable together with its original database. Both native release-reader jobs, both product exchange jobs and the two fixture jobs passed. [Native run](https://github.com/gungortarik/cryptex/actions/runs/45697770767)

At `6a2c610`, the installed Mac trial exercised device-key refusal, a device-only grant with an inaccessible transport credential, and an accepted grant followed by an independent silent read. The installed application reopened the original signed note. An updater unit test covered loss of the credential verifier's reply. The hosted trial used a disposable Keychain and controlled access grants. The dialog on the owner's Mac and the installed migration of an already paired 2.1.5 profile require separate evidence. [Signed Mac trials](https://github.com/gungortarik/cryptex/actions/runs/45698215386)

