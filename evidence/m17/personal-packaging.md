# M17 personal-device packages

The owner selected their own Mac and Windows VM as the audience. The packaging rehearsal uses a macOS app bundle and a Windows per-user NSIS installer. Apple notarization and public publisher credentials would be separate distribution decisions. The [release requirements](../../docs/27_RELEASE_DISTRIBUTION_AND_SIGNING.md) and the [M16 personal-use hold](../m16/hardening.md#personal-use-release-hold) still apply.

## Package inventory

At `207dd15`, both native installations started Desktop and the packaged daemon and compared the frontend and Unicode notice bytes with the inputs. [Installed release smoke](https://github.com/gungortarik/cryptex/actions/runs/45249384781)

The Rust inventory includes the locked normal and build dependencies for Desktop with Tauri's custom protocol and the packaged daemon. It records package names, versions, declared licenses and document hashes. Each document is included once. For registry packages missing their repository-level terms, supplemental notices are associated with the exact package version and a source URL in [the inventory](../../third_party/rust-notices/sources.json). Sources include repository-level terms for objc2's Apple SDK bindings. Embedded third-party code and the managed transport require their own notice review.

The native inventory includes the source notice from the bundled SQLite 3.54.1 header in `libsqlite3-sys 0.39.1`. On Windows it also includes both notice files from Microsoft's WebView2 SDK 1.0.3800.47. The three static loader libraries in `webview2-com-sys 0.39.1` matched the corresponding x64, x86 and Arm64 entries in the upstream NuGet archive byte for byte. The archive SHA-256 was `56c9f26bdd07916a2d1949fb58a5c7e434dfa1173577dca879206050c4e718db`. [Upstream SDK package](https://www.nuget.org/packages/Microsoft.Web.WebView2/1.0.3800.47)

## Signing rehearsal

Disposable hosted accounts create temporary code-signing identities. The Mac test signs the daemon and then the app, copies the app to an installation directory, checks both roles against the provisioned public certificate and checks the bundle seal. The Windows test requests Authenticode for the actual installer, verifies the signer before installation and checks the installed Desktop and daemon. Both tests include wrong-signer and altered-content controls. The workflow exposes test results only.

At `784cfba`, both native jobs passed. The Windows runner verified the NSIS installer before execution and checked the installed Desktop and daemon. The Mac runner verified both roles against its independent DER pin after copying the signed bundle. Both compared the installed Rust notice inventory to its input, refused the wrong signer and detected a changed file. Mac also checked cleanup of the disposable signing identity. [Windows and macOS test results](https://github.com/gungortarik/cryptex/actions/runs/45253534985)

Before a personal install, the owner needs to verify the release signer's fingerprint through an independent local channel. The final packaging and updater review must cover the installed payload and the previously working version used for rollback. CI fixture identities are discarded at the end of each job. No real-profile installation or key enrollment has taken place in these rehearsals.

## Official transport inspection

The [two-platform binary inspection](https://github.com/gungortarik/cryptex/actions/runs/45252879619) at `bc24732` matched the published archive digests for official version 2.1.7. Both binaries report Go 1.27.3 and the same four in-range findings recorded in M16. The report also contains the historical GO-2022-0888 finding, whose affected range predates this version. Inspection downloads the executables to temporary storage, performs static analysis and leaves the installed transport untouched. The job can be dispatched again when upstream publishes a stable release. Its success records a completed inspection. Decisions about personal use remain with the owner under the M16 hold.
