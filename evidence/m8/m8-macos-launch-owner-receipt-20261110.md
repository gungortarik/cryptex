# Personal-Mac launch receipt: incomplete; measurement corrections prepared

**Update:** the corrected owner run has returned; see the
[second receipt and stopped carrier candidate](m8-macos-launch-owner-receipt-2-20261110.md).
Its five controls pass but six mapped-child cases fail. The next-package
instructions below are historical; do not repeat that run unchanged.

The returned `MAC-BAGLANTI-SONUC.zip` was inspected as data, not executed.
SHA256: `01b3348109602953e910744ec7740b3c2d247a86fbf3ae008781d03508ff36df`.
Inventory: one regular `result.json`,1680 bytes. Collected2026-11-10T22:15:21Z.
Its closed native schema/conditional case inventory and incomplete status were
validated against the committed source. Source/native hash match b234322:
`2c51c9850a7d36eba77154040317b75eacad2fb11dbec6807ae9204191ded259`.

## What the owner receipt actually establishes

- Personal Mac27.2/SDK27.2 arm64, ci_only=false.
- Native compile passed; existing-certificate fixture/bundle signing and strict
  bundle verification reported success. No certificate selector/TeamID exported.
- The common supervisor-vs-child-deadline distinction passed.
- Aggregate atomic headers/runtime gates were false. That aggregate does not
  identify which declaration/version-macro condition failed, and does not prove
  the Mac runtime lacks the individual APIs.
- Self validation reached the signing metadata stage, then reported
  `certified_team_unavailable`. The anchor/counterpart/channel positive cases did
  not run. `candidate_complete=false`, runner_exit0, production_changed=false,
  M8 false. A clean runner exit is not positive custody evidence.

Neither launch finding is closed by this receipt. It does not justify changing
the owner's certificate, Mac configuration, platform support floor or IPC design.

## Confirmed harness bug and correction

`policy_open` passed flag0 to `SecCodeCopySigningInformation`, then attempted to
read `kSecCodeInfoTeamIdentifier`. Apple's public header labels that field
**Signing**: it requires `kSecCSSigningInformation`; generic information alone
predictably omits it. This was a test-code defect, not evidence of a missing
certificate or unavailable team identity on the Mac.

The correction requests signing information after the same successful strict
live-self/fixed-identifier/Apple-anchor check. Certified-Team revalidation, exact
counterpart requirements, image hashes and all refusal conditions remain in place.
Generic image-hash queries correctly retain their generic flags.
[Public contract](https://github.com/apple-oss-distributions/Security/blob/main/OSX/libsecurity_codesigning/lib/SecCode.h),
[Apple implementation](https://github.com/apple-oss-distributions/Security/blob/main/OSX/libsecurity_codesigning/lib/StaticCode.cpp).

The aggregate version/preprocessor gate was also replaced with bounded public
SDK compile-only probes under the actual compiler/SDK/minimum12 configuration:
baseline headers; guarded pipe2 declaration; guarded O_CLOFORK; guarded FD_CLOFORK;
and a combined guarded pipe2/fcntl expression. Each observation is recorded.
Baseline failure or an abnormal compiler/timeout/output/pipe failure is a tool
error, never API absence. Only all five literal true observations enable the
atomic source branch. Native compilation/link and runtime availability still
must succeed. No copied constants, private declaration or fallback was added.

The runtime report distinguishes an unexercised gate due to public-header refusal
from an actual OS-guard observation. The child's newer flag operation is itself
guarded too. The correction does not pre-approve macOS27 as the product minimum.

## Verification and next exact package

Corrected candidate: `888d68acaedd30e9622fe08e97ef7deeff1094c2`.
[Exact Mac CI42914376385](https://github.com/gungortarik/cryptex/actions/runs/42914376385)
**SUCCESS**: native compile/refusal, public-SDK observations, bounded cleanup and
scoped regressions. Mac41 Python tests:40passed, one platform skip. Local Windows
41 tests:36passed, five native-Mac skips. Added regressions cover every individually
missing declaration, baseline/abnormal compiler failure, exact boolean gating
and all-true build enablement. Source-only independent review found no actionable
issue in the corrected delta; it did not execute or accept native positive cases.

This CI host is still Mac26.7.2/SDK26.5 with ad-hoc signing. Its negative controls
cannot establish the owner's positive signed Mac27 carrier/bundle behavior.

Updated outgoing:
`C:\Users\Tarik\Downloads\cryptex-mac-baglanti-888d68ac.zip`,22709 bytes.
SHA256: `c4db8cdff70444f45989324aff5c8032533523e84037e6c43c684492cd49a088`.
Native C SHA256:
`84c91c53b239e22c49769587ace3cbce7a5774fa185f2fee0ed53fff2c76917d`.

Owner: extract this updated package on the same personal Mac, run
`MAC-BAGLANTI.command`, return the new `MAC-BAGLANTI-SONUC.zip`.
No new certificate, Windows VM, repeated pairing or unrelated API/Mach package.
If a declaration genuinely fails its corrected public probe, stop this candidate
rather than copying flags or forcing a weaker carrier. Keep the existing accepted
Unix transport and all Mac owner mutation gates unchanged.

Even positive eleven-case evidence leaves production bidirectional/lifetime
binding, bundle replacement consistency, forwarded-descriptor/frame attribution,
owner-session restart, support-policy and final acceptance gates open as recorded
in the [candidate scope](m8-macos-atomic-launch-candidate-20261110.md).
