# M8 final candidate review

**Closed on November 18, 2026.** The Windows and macOS readiness jobs, Windows cross-account IPC check and both native recovery jobs passed for the owner-reviewed candidate.

The owner reviewed candidate `7be9287f38a2378d838ed6b5fc326a34c76cd286` and approved M8 closure subject to green required checks. The review covers the Windows development product and the locally signed macOS 26+ development product. Public distribution has its own signing and release decisions. [Reviewed changes](https://github.com/gungortarik/cryptex/compare/c20d97a...7be9287) · [Accepted product scope](../../docs/proposals/m8-product-flow-activation-20261115.md)

## Contract evidence

| M8 requirement | Evidence used for acceptance |
| --- | --- |
| Pairing | The owners approved the exact device and key bindings. Normal product captures went in both directions and remained searchable after paused reopen. [Owner observations](m8-owner-bidirectional-delivery-20261117.json) · [Pairing and replay](m8-windows-two-profile-pairing-replay-20261107.md) |
| Managed transport | Normal owner processes exchanged sealed segments over the supervised transport and continued after a restart. The device run below used the Windows VM and Mac over LAN. [Two-owner transport](m8-normal-offline-backlog-20261117.json) |
| Incoming validation | The daemon verifies signed incoming data before publishing immutable records. Authenticated policy conflicts retain evidence across restart and hold access to guarded content. [Policy hold](m8-incoming-policy-hold-review-20261114.md) · [Mac filesystem and native integration](m8-macos-filesystem-native-integration-review-20261114.md) |
| Offline convergence | The Windows and Mac follow-up receipts below agree on all sixteen signed records. Each device found all four interrupted-run markers after a paused offline reopen. |
| Restriction and retirement | The remaining authorized peers continued to exchange records after excluding a compromised device. With the retiring device offline, the first recipient relayed its original signed notice to a third disposable profile after interruption and restart. [Remaining-peer restriction](m8-remaining-peer-restriction-20261117.json) · [Three-profile Windows relay and Mac interruption checks](https://github.com/gungortarik/cryptex/actions/runs/44449594662) |
| Sequence cutoff | The two-host disposable test rejected post-cutoff data from each direction and preserved the accepted prefix after reopening. The conformance suite also exercises signed boundary cases. [Windows and Mac receipts](m8-physical-lan-cutoff-9a6dc973.json) · [Signed cases and compatibility](m8-protocol-schemas-compat-review-20261114.md) |
| Replication and protection status | A local IPC response identifies local service readiness. The panel reports local pause, authority restrictions, lost replies and incoming verification separately. Remote durability stays unverified. [Rendered screen checks](../../apps/desktop/scripts/connection-screen.test.mjs) · [Native status policy](../../apps/desktop/src/native-profile-status.test.mjs) |

## Interrupted two-device run

Run `9b502d61-00ed-4686-841d-10bce8fec61b` timed out with fifteen records on each device and different digests. The follow-up reused those four markers. The paired-report comparator checked both returned archives against the issued follow-up packet for source `7684286342e9acf4260887214e0dc5b4b04fabb2`. Both devices reached sixteen records with aggregate SHA-256 `bd2c949fc008d2363caf2eb4d9a7bdc8f217e07fe3e3d698e130bc4f4cac3abd`. This establishes subsequent recovery from process interruption. It does not measure power-loss durability.

| Original archive | SHA-256 |
| --- | --- |
| `Cryptex-Toparlanma-7684286.zip` | `2fa2a529d16fa5cc0f9892e5d3a99c80837e5177219d01ab412efc8fc80b3be8` |
| `TOPARLANMA-SONUC-Windows-1801b0320f2b.zip` | `838e5b701a67b3ef737320db1f918f108cdc1e5026884c9345d30a07623f8c69` |
| `TOPARLANMA-SONUC-Darwin-c670b0de12fc.zip` | `4453ee48e9e5bc02302a904d58957a6acf977df7e3a82b4d786fb7dc8209e2e4` |

## Status and security review

A Chromium test loads the built Desktop frontend with simulated Windows and Mac IPC replies. It checks a lost save response, recovery after reload, read-only restrictions, a recovering Mac status channel, and viewport width in the light and dark layouts. The service and native transport tests exercise the real platform boundaries separately. No device owner had to repeat pairing or create a new note for this review.

The native owner path binds the Windows caller to the expected Desktop process and the Mac request to its authenticated owner channel. The daemon checks local trust and the retired device's original signed terminal record. Relaying that record grants no new peer authority. Incoming publication preserves prior canonical bytes. The native file chooser keeps paths out of the renderer. The candidate fixes a file-selection race by checking the type and length on the open handle used for reading. Windows refuses final reparse points and macOS refuses symbolic links at open. The decoder independently bounds the input. [File handling and regression tests](../../apps/desktop/src-tauri/src/retirement_file.rs) · [Retirement handoff design](../../docs/proposals/m8-self-retirement-delivery.md)

The accepted availability tradeoff applies throughout this candidate. Unresolved trust cycles or authenticated history after terminal retirement may hold guarded content for the whole Brain indefinitely. M8 provides no routine override. The owner reviewed the candidate with this consequence disclosed. [Protocol decision](../../docs/proposals/m8-final-protocol-acceptance-decision-20261114.md) · [Independent findings and correction reviews](m8-final-protocol-review-20261114.md)

## Candidate checks

The daemon and shared crates match the three-profile relay source `c20d97a` byte-for-byte in Git. Physical receipts above identify their own original source. Exact-candidate runs cover readiness, the rendered UI, cross-account Windows IPC and native recovery. [Readiness CI](https://github.com/gungortarik/cryptex/actions/runs/44452195353) · [Native recovery CI](https://github.com/gungortarik/cryptex/actions/runs/44452198734)
