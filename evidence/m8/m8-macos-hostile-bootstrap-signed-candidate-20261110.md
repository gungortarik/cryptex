# Public bootstrap hostile-endpoint checks and signed Mac candidate

Status: **reviewed isolated experiment; owner signed run pending; M8 OPEN**.
The owner authorized this bounded research while retaining accepted Unix product
IPC. This receipt does not authorize product activation or an architecture change.
[Design and boundaries](../../docs/proposals/m8-macos-message-bootstrap-experiment.md).

## Exact final source and native CI

- Source: `8e8fabbc8e50bcbb65f889a3de5b2156f0cc836b`.
- [CI42952782022](https://github.com/gungortarik/cryptex/actions/runs/42952782022):
  **SUCCESS**, exact head verified; mechanism job `130300753899`.
- Native runner: macOS26.7.2 arm64, SDK26.5. Compilation uses public headers,
  Wall/Wextra/Werror, availability errors and deployment target12. This does not
  establish runtime support on12 or select a product support floor.
- C SHA256: `d6f66eaa4cdeb6a6395da298ea9e0145a847aac88e4198e09bc10c988c1dab18`.
- Policy-header SHA256: `fb36977d15bbef59d4111343d052227e498909b69c872e6c453256b3933d6347`.

Both actual JSON reports were parsed from the completed job log; their C/header
hashes match exact committed Git bytes. The ad-hoc mechanism has all17 required
closed-schema observations passing and native_exit0. In addition to the prior13
rows, the new observations are:

| Case | Required actual observation |
| --- | --- |
| substituted_endpoint | Victim child_exit52, kernel sender PID refused |
| transferred_reply | Victim child_exit52, kernel sender PID refused |
| transferred_service | Parent validate52, kernel sender PID refused |
| genuine_after_transfer | Actual victim validate0 using unchanged next sequence |

Each helper first authenticates the actual parent and is authenticated against its
captured spawn PID. Root spawns the victim itself. Setup failure, timeout, signal,
wrong signature or replay cannot count as the intended PID refusal. Control
CONFIG1/TRANSFER2/REJECT3 is separate from victim ACK1/STOP2. The copied-service
attacker uses the same Daemon file/role/hash as the victim in signed mode; fake
endpoint/reply helpers use the Desktop image they impersonate. All fifteen
control/data/attack names require independent removal checks.

Both modes passed required native fault controls: injected reaper paths exit0,
intentional wrong-right cleanup exit68, real full-queue send timeout exit67 and
invalid send-once header exit59. Mechanism forced termination observed signal9;
a separate lookup confirmed namespace removal, cleanup_exit0.

The certified two-binary policy compiled and explicitly refused its ad-hoc live
self with `apple_anchor_or_fixed_self_identifier_refused`. The report has
ci_only=true, candidate_complete=false and no positive owner signature/native
rows. This is expected refusal evidence, **not nineteen signed cases passing**.
Both reports keep production_changed=false and m8_complete=false.

## Review and corrections

Independent source-only review covered helper custody/right ownership, typed
failure paths, certified live-self trust root, fixed roles, bundle checks and the
final exact-source delta. No actionable issue remained at8e8fabb. Reviewer ran no
tests and granted no product acceptance; native evidence is the exact CI above.

Review corrected the helper REJECT sequence from2 to3, preventing a control replay
from masquerading as the intended forgery result. Signed receipt parsing now
preserves validated partial prefixes and rejects non-string/unhashable labels.
The final service attacker was strengthened to share the victim's actual Daemon
role/hash, with matching positive control-plane policy checks both ways.

An earlier successful dispatch42950152593 actually used old heade826623 and was
excluded as phase-two evidence. Run42950273975 found a const-qualified CFRetain
assignment compile error; typed assignment and a separate retain corrected it.
Exact subsequent runs42950658480/f117a3b,42951646456/c89d5fb and42952006013/7420a52
passed their respective sources; final8e8 evidence supersedes those snapshots.

Local final suite:63 tests,58passed/five native-Mac skips. All22 bootstrap report/
failure/package tests passed. Native shell syntax and both bootstrap suites passed
on Mac CI. No product Rust behavior changed in this experiment.

## Source-bound outgoing package and owner action

Package: `C:\Users\Tarik\Downloads\cryptex-mac-mesaj-8e8fabbc.zip`.
Size:33437 bytes. SHA256:
`ea4a5d7527fd1cd6ca45bfffa6e68ac193869ffb6c44f4b9b2372a8485448c20`.
All eight payload members match exact committed source8e8fabb, with manifest
scope/source/SHA256 checks, UTF-8/LF and command executable mode0755 verified.
The earlier local7420 package is superseded; do not send it as this candidate.

On the same personal Mac: extract ZIP, open **MAC-MESAJ.command**, return
**MAC-MESAJ-SONUC.zip**. If Keychain asks, allow this codesign operation with
**Allow**, not Always Allow. It uses one existing signing identity in a private
temporary synthetic app, creates no certificate/permanent service and exports no
private key. No product data, device profile or trust record is changed. No VM or
old pairing/API/descriptor package rerun is requested.

Inspect source hashes, signature eligibility, all19 required observations (17
mechanism plus two signed-image refusals), fault controls and independent forced
cleanup before claiming signed-candidate success. Ineligible/incomplete output is
diagnostic evidence, never a pass. Do not edit signed messages or weaken checks.

## Gates still open

Owner signed-positive19-case evidence, concurrent bundle replacement/snapshot
consistency, broader forwarding/flooding/deadline behavior, actual product
bidirectional pin/lifetime and owner-session restart remain unverified. Startup
Mach binding does not establish steady-state Unix frame sender attribution.
Publisher/notarization and minimum-OS acceptance are separate. Mac managed
transport/publication parity, actual supported offline/restriction propagation,
final security/product/human review and M8 completion remain open. Existing Unix
IPC, Mac mutation gates and normal production feeding remain unchanged.
