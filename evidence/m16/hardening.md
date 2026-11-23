# M16 hardening campaign

M16 is open. The implementation baseline is the M15 closeout at `50869d6`. The campaign uses disposable identities and synthetic records. Each run below identifies the revision it executed. Acceptance requires a reviewed candidate, both native platforms, recorded limits and the [Definition of Done](../../docs/21_DEFINITION_OF_DONE.md).

## Failure coverage

The section numbers refer to the [failure matrix](../../docs/17_TEST_AND_FAILURE_MATRIX.md). Earlier acceptance records identify existing checks that must be considered in the final review.

| Sections | Failure or boundary | Runtime evidence and outstanding work |
| --- | --- | --- |
| 8, 9 | Interrupted canonical write, incompatible or damaged record, checkpoint loss, stale index and rebuild | [Storage baseline](../m2/m2-closeout-l1-l2-f4-f6-20261007.md), [projection baseline](../m3/m3-final-review-20261008.md), [M16 signed-pack reconstruction](https://github.com/gungortarik/cryptex/actions/runs/45119773599). All six scale jobs passed. |
| 10 | Storage layout and platform-specific scale | [Original append, filesystem and LAN measurements](../m0/p0-1/README.md), [M16 reader and reconstruction](https://github.com/gungortarik/cryptex/actions/runs/45121866739), and [guarded search and vault publication](https://github.com/gungortarik/cryptex/actions/runs/45122988170). Fixture generation time is excluded from reader measurements. |
| 11 | Window close, login, duplicate writer, service crash, reconnect and failed update | [Installed platform review](../m14/platform-closeout-review.md) includes real temporary-account receipts. [Installed update rehearsal](https://github.com/gungortarik/cryptex/actions/runs/45119157833). |
| 12, 15 | Local endpoint, role, capability, incompatible or malformed request, restricted retrieval and explicit consent | [General CI and cross-account Windows ACL](https://github.com/gungortarik/cryptex/actions/runs/45120098263), [local MCP rehearsal](https://github.com/gungortarik/cryptex/actions/runs/45119158085), [prior endpoint review](../m1/m1-final-review-20261007.md). Final review must also trace renderer and Doctor outputs. |
| 13, 14 | Managed transport isolation, unavailable peer, delayed delivery, duplicate replay, restriction and repeated reconnect | [Trust and replication review](../m8/m8-final-candidate-review-20261118.md), [packaged repeated partition run](https://github.com/gungortarik/cryptex/actions/runs/45120714690). Both [owner sleep observations](#offline-and-system-sleep) passed. |
| 16 | Interrupted backup, large object, damaged generation, wrong recovery key, restore and reconstruction after source loss | [Large-object and installed recovery](https://github.com/gungortarik/cryptex/actions/runs/45119158424), [independent Kit recovery and accepted import evidence](../m15/cortex-import-review.md#final-acceptance), [bidirectional rejection and retry with patched dependencies](https://github.com/gungortarik/cryptex/actions/runs/45121906178). |
| 17 | Purged history, stale replica, shared attachment, unreachable copies | [Forget and purge rehearsal](https://github.com/gungortarik/cryptex/actions/runs/45119157942), [scope of cleanup](../m10/forget-purge-closeout-review.md). Review interactions with backups and accepted imports. |
| 18, 19 | Displayed health during service loss, maintenance and restoration, privacy of diagnostics | [Desktop and Doctor rehearsal](https://github.com/gungortarik/cryptex/actions/runs/45121923267), [desktop review](../m13/desktop-shell.md) and the decoder and redaction tests below. An endpoint alone cannot establish remote durability. |
| 20–23 | Required CI, traceable receipts, complete hardening scope | The Mac and Windows VM sleep receipts passed. Final platform CI, material risk review and human acceptance remain open. |

## Reproduction

- **Signed history.** Run the exact ignored daemon test `projection::scale_test::signed_pack_projection_recovers_at_scale` in Cargo release mode. Set `CRYPTEX_M16_EVENTS` to 10000, 100000 or 500000. One synthetic device contributes signed notes in packs of 1,000. For each count the harness measures two independent rebuilds, reopens SQLite, checks every pack digest, the signed chain tip, the exact note count and an FTS match at the end of every pack. The recorded times are workload observations.
- **Repeated partitions.** Run the M16 repeated offline sync workflow. Each OS hosts two authenticated native transports bound to loopback. A disposable standard account runs the Windows test. The same pair retains its enrolled identities across six cycles of four new notes per author. Each phase starts a fresh daemon test process. After each online publication the receiver retries the same signed pack, compares the canonical bytes, and checks guarded search again after both transports exit. This exercises 48 new notes plus two genesis notes.
- **Encrypted peer recovery.** Run the M11 cross-platform restore workflow. The recipient tries a truncated ciphertext and a valid generation with a different disposable Kit. It checks for any newly published or residual staging entries after each failure. It then uses the original generation and correct Kit at the same destination, verifies the signed inventory through the daemon and checks the reconstructed notes. Only synthetic ciphertext and public exchange material cross CI hosts.

## Results and open gates

### Signed history measurements

All six native release jobs at `4a87c53` passed. Each timed rebuild includes opening the derived database and verifying the signed history. The query below selects a known note from every pack after closing and reopening the database. A displayed zero means less than one millisecond at the timer's reporting resolution. [Native job logs](https://github.com/gungortarik/cryptex/actions/runs/45119773599)

| Platform | Signed notes | Missing DB rebuild (ms) | Corrupt DB rebuild (ms) | Query after missing / corrupt rebuild (ms) |
| --- | ---: | ---: | ---: | ---: |
| macOS | 10,000 | 1,400 | 1,328 | <1 / <1 |
| macOS | 100,000 | 18,623 | 22,846 | <1 / <1 |
| macOS | 500,000 | 84,710 | 84,225 | 1 / 1 |
| Windows | 10,000 | 2,471 | 2,450 | <1 / <1 |
| Windows | 100,000 | 29,550 | 24,102 | 1 / 1 |
| Windows | 500,000 | 96,538 | 96,354 | 4 / 3 |

All six scale jobs also passed on the patched product revision `6a40f00`. At 500,000 events the missing/corrupt rebuilds took 75,454/74,225 ms on Windows and 91,110/89,252 ms on macOS. The reopened FTS queries took 2/3 ms and 1/1 ms respectively. The timings describe the same low-level signed-pack workload. [Patched native measurements](https://github.com/gungortarik/cryptex/actions/runs/45121866739)

A separate release workload at `53019de` rebuilt knowledge from 10,000 signed notes, closed and reopened the database, and ran the daemon's guarded search and Obsidian publication paths. On each system the search returned the ten expected pack-edge matches, all ten bodies matched their generated Markdown, both publications produced exactly 10,000 note files, and an independent user file survived. The workload invokes the daemon operations directly; the measurements exclude WebView and Obsidian startup. [Native job results](https://github.com/gungortarik/cryptex/actions/runs/45122988170)

| Platform | Guarded search (ms) | Initial vault (ms) | Vault refresh (ms) |
| --- | ---: | ---: | ---: |
| Windows x64 | 1,359 | 83,348 | 159,776 |
| macOS arm64 | 1,685 | 10,678 | 11,305 |

### Native product verification

All eleven workflows requested at product revision `6a40f00` passed, including general CI on Windows and macOS and the Windows cross-account IPC check.

| Area | Verified runs |
| --- | --- |
| Build, lint, authorization and native desktop | [General CI](https://github.com/gungortarik/cryptex/actions/runs/45121899289), [installed rollback and reconnect](https://github.com/gungortarik/cryptex/actions/runs/45121909232), [desktop shell](https://github.com/gungortarik/cryptex/actions/runs/45121923267) |
| Visibility and authority | [Attachment flow](https://github.com/gungortarik/cryptex/actions/runs/45121919699), [forget and purge](https://github.com/gungortarik/cryptex/actions/runs/45121866875), [MCP consent](https://github.com/gungortarik/cryptex/actions/runs/45121912596), [Cortex import](https://github.com/gungortarik/cryptex/actions/runs/45121915978) |
| Storage and recovery | [Signed-history scale](https://github.com/gungortarik/cryptex/actions/runs/45121866739), [repeated partitions](https://github.com/gungortarik/cryptex/actions/runs/45121866737), [installed encrypted backups](https://github.com/gungortarik/cryptex/actions/runs/45121902856), [recovery from the other OS](https://github.com/gungortarik/cryptex/actions/runs/45121906178) |

All five workflows at `53019de` also completed successfully: [general CI](https://github.com/gungortarik/cryptex/actions/runs/45122995104), [scale and guarded vault](https://github.com/gungortarik/cryptex/actions/runs/45122988170), [historical reader](https://github.com/gungortarik/cryptex/actions/runs/45122988132), [desktop attachments](https://github.com/gungortarik/cryptex/actions/runs/45122988192) and [forget/purge](https://github.com/gungortarik/cryptex/actions/runs/45122988305). The difference from the physical sleep packet is test, workflow and evidence content.

### Security and recovery review

The MCP tool list and dispatch use explicit names for health, requesting review and searching public notes. The daemon checks the approved stream, client role, capability, deadline and profile admission. The reader filters visible public notes before returning up to ten excerpts. Read access to a recovered profile is held when the MCP reader cannot verify both histories. The successful native MCP and forget rehearsals exercise the restricted and stale-index cases. [Bridge dispatch](../../tools/local-mcp/server.mjs) · [Daemon authorization](../../services/cryptexd/src/ipc/public_context.rs) · [Guarded context](../../services/cryptexd/src/knowledge_materialization/ai_context.rs)

The Windows worker's retained first-failure record uses an explicit schema with integer fields for the subsystem and optional OS code. The recorder preserves the first error across later shutdown and returns the original failure even when it cannot persist the diagnostic. The Desktop declares an explicit invoke list. The installed-update rehearsal exercised retained package fingerprints, the service connection after rollback and canonical note preservation on both native systems. [Worker diagnostics](../../services/cryptexd/src/managed_profile/worker/diagnostics.rs) · [Desktop registration](../../apps/desktop/src-tauri/src/main.rs) · [Update and rollback run](https://github.com/gungortarik/cryptex/actions/runs/45119157833)

At `cf2cf3a` both platforms rejected the truncated peer backup and a valid backup with the incorrect Kit. Both then opened the original generation with the correct Kit and verified two signed notes via the native daemon. [Cross-platform recovery run](https://github.com/gungortarik/cryptex/actions/runs/45120234060)

The large-object rehearsal recovered a 67,108,987-byte synthetic blob and a signed frame after deleting the source and plaintext archive. It compared lengths and SHA-256 digests, kept the last complete generation across an interrupted write, and rejected an incorrect key and modified ciphertext. The separate installed restore verifies the daemon's canonical inventory and reconstructed search. [Large-object workload](../../tools/m11-backup-spike/rehearse.py) · [Native backup run](https://github.com/gungortarik/cryptex/actions/runs/45119158424)

The desktop status decoder rejects a claim of remote durability even when a local publication and a live transport have been observed. The connection view displays only allowlisted diagnostic codes. At `6a40f00`, all 27 Windows Node checks in `native-profile-status.test.mjs`, `connection-settings.test.mjs` and `capture-policy.test.mjs` passed. The webview CSP restricts script origins to the bundled app and network connections to IPC. [Status decoder](../../apps/desktop/src/native-profile-status.mjs) · [Status and redaction tests](../../apps/desktop/src/connection-settings.test.mjs) · [Webview policy](../../apps/desktop/src-tauri/tauri.conf.json)

### Dependency review

The November 23 OSV scan found advisories in four dependencies of the native product. The lockfile now resolves bundled SQLite through `libsqlite3-sys 0.39.1`, Tokio 1.53.3, Rustix 1.1.6 and URL 2.5.9. The updated SQLite binding requires exclusive access to install a progress hook. A deadline-limited read now uses the binding's interrupt handle, joins the timer thread before returning the shared connection, and checks the deadline before releasing results. A Windows regression interrupted an executing SQL aggregate and then successfully queried the same connection. All 68 projection tests passed. [SQLite](https://osv.dev/vulnerability/RUSTSEC-2026-0459) · [Tokio](https://osv.dev/vulnerability/RUSTSEC-2026-0386) · [Rustix](https://osv.dev/vulnerability/RUSTSEC-2026-0486) · [URL](https://osv.dev/vulnerability/RUSTSEC-2026-0592)

At `6a40f00` the repeat scan checked 727 public crate/version pairs across the three lockfiles. None of the findings occurred in the Windows x64 or macOS arm64 normal/build dependency trees of the desktop, daemon or updater with all features. Seven affected entries were outside those graphs and the native codec trial. One remaining entry, `crypto-bigint 0.5.5`, was present in the separate Windows interoperability trial. Keep that trial scoped to synthetic credentials and repeat its dependency review before considering it for distribution. Production-only npm audits in the local MCP and backup tools, and the workspace production pnpm audit, reported zero findings. These counts describe registry advisories available at review time. [Reproducible Rust scan](../../scripts/m16/check_rust_advisories.py) · [Trial advisory](https://osv.dev/vulnerability/RUSTSEC-2026-0387)

### Offline and system sleep

The two `6a40f00` soak archives passed their extracted native smoke checks. Their Syncthing executables matched the established Windows x64 and Apple arm64 SHA-256 pins; both bundled license and authors files compared byte-for-byte with the monitored inventory. The exercises bind transport peers locally. [Patched package CI](https://github.com/gungortarik/cryptex/actions/runs/45121866737) · [Upstream source and notice inventory](../../third_party/syncthing/2.1.5/README.md)

For verification of the physical run, both owner ZIPs were compared with fresh downloads from Actions run 45121866737. For each download the enclosing artifact matched GitHub's published SHA-256. The inner ZIP matched the copy delivered to the owner, and the independently hashed daemon test executable matched its build manifest.

| Physical packet | Actions artifact ID | Owner ZIP SHA-256 | Expected receipt field `runner_sha256` |
| --- | --- | --- | --- |
| Windows x64 | 14794531146 | `9c6aaf9e448184e4adb9bed366310f9728ffac05a5e19b3f974f79301e33c00f` | `dadacb04bb0ad30cf1dd1e08f581d41f2fc49eb4c204c7a8dd7c7700168abb33` |
| macOS arm64 | 14794370826 | `5364f469dbc9049a05387e7dc0691a43f2344a13907e1094ff3abfc501da5e7a` | `d84057aaa8ce21a1acde037722cd6d3299f3709e1413e9f5f90d430c9e16cf45` |

Compare each owner-returned schema 1 receipt with its platform, runner and full revision `6a40f00367962ccad4dc0fc7b4ae949086263659`. A completed run reports at least five kernel-observed suspended seconds, both retained-instance resume and cold read, six rounds and 50 verified notes per replica. The JSON is an owner-supplied test observation; the build digests identify its claimed executable. [Receipt producer](../../scripts/m16/owner_soak.py)

At `0cf55da`, the packaged two-peer smoke completed six disconnect/reconnect rounds on both platforms. It verified all 50 notes per replica after a cold read. An owner receipt additionally requires a suspend observed by the kernel while the retained peer is alive. Sampling brackets bound the actual suspended duration from below. A stopped user process or a change of wall clock does not count. Each package uses two local synthetic peers. [Native package smoke](https://github.com/gungortarik/cryptex/actions/runs/45120714690) · [Apple continuous clock](https://developer.apple.com/documentation/kernel/1646199-mach_continuous_time) · [Windows interrupt time](https://learn.microsoft.com/en-us/windows/win32/sysinfo/interrupt-time)

The owner returned receipts from the [Mac](M16-UYKU-SONUC-Darwin-1cf6b966a7.json) and [Windows VM](M16-UYKU-SONUC-Windows-87b897f236.json). Both match the expected schema, platform, architecture, revision and runner digest. The observed sleep lower bounds were 38 and 69 seconds respectively. Both runs resumed the retained instance and verified 50 notes per replica after six rounds and a cold read. Each run used two peers on the same OS.

### Restore after purging an accepted Cortex note

At `f1095fb` the encrypted backup writer and restore verifier match each signed Cortex source receipt to either a live note or its authenticated purge bridge. For a live note they check the text digest and classification. For a bridge they check the original event hash and any retained original classification. The locked backup snapshot verifies the separate owner-signed purge approval. The original batch decision still covers the complete set of signed source receipts, including relative source paths. This preserves provenance inside the encrypted backup.

The [Windows and macOS rehearsal](https://github.com/gungortarik/cryptex/actions/runs/45239337808) passed at `7cd3651`. Windows ran under a temporary standard account. The test imports and accepts three synthetic Markdown files through the daemon in a temporary profile. After confirming one purge, it encrypts a new backup, removes the source profile and its disposable credentials, and reconstructs a fresh directory using the recovery key and selected checkpoint. It compares the two surviving signed records exactly, checks that the deleted body is absent from the backup payload, and verifies the purge bridge. Removing the purge approval or removing or substituting the deleted note's source receipt must block verification. Run `python -B scripts/m16/cortex_purge_backup.py` with the repository's Rust toolchain and the M11 OpenPGP test dependencies installed.

### Remaining decisions and observations

**Transport release risk (R-04/R-28).** Inspection of the packaged executables found Go 1.27.1, x/crypto 0.55.0 and x/net 0.58.0 in the pinned Syncthing 2.1.5. The Go advisory database reports affected versions in all three. As an independent comparison, the official 2.1.7 Windows x64 and macOS arm64 ZIPs matched GitHub's published SHA-256; their binaries contain Go 1.27.3, x/crypto 0.57.0 and x/net 0.60.0. The later release still matches GO-2026-6859. Neither upstream candidate was installed during inspection. Run `govulncheck -mode=binary -format=json <syncthing executable>` (scanner v1.10.0 at review) for the binary findings. Stripped binaries can produce conservative module-level results even in symbol mode; these results do not establish network reachability. An old relay advisory ending at Syncthing 1.15.0 does not cover the reviewed 2.1.x versions. The release gate needs a disposition and native package compatibility/rollback evidence for any replacement pin before personal use. [Go advisory](https://pkg.go.dev/vuln/GO-2026-6859) · [Official release](https://github.com/syncthing/syncthing/releases/tag/v2.1.7) · [Managed transport ADR](../../docs/adr/ADR-0004-managed-syncthing-implementation-addendum.md)

Losing all import evidence before creating a recovery selection can eliminate the evidence identifying Cortex origin. The post-purge recovery rehearsal covers a profile with its signed source receipts, batch decision and independent recovery selection intact. [M15 evidence and limits](../m15/cortex-import-review.md#evidence-and-limits)

The final review also needs current evidence for privilege boundaries, dependency and native-package provenance, redaction, and the distinctions between a locally ready transport, completed replication and verified recovery. [Risk register](../../docs/22_RISK_REGISTER.md)
