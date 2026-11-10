# Mac atomic launch and verified-self package candidate

Status: bounded native candidate, **NOT production integration or M8 closure**.
Source: `b2343229058336b35ddab42bd3cdd974284e89a0`.

## The two findings and their candidate corrections

The previous draft's `pipe()` followed by `fcntl` could expose a descriptor
between creation and noninheritance setup. The new candidate compiles the
atomic path only when public headers declare the required macOS27 surface,
uses runtime availability checking, and requires one `pipe2` call with
`O_CLOEXEC | O_CLOFORK | O_NONBLOCK`. Both returned descriptor flags must be
successfully read and contain both protections. There is no copied numeric
constant, private declaration or non-atomic fallback.

The intended child uses explicit `posix_spawn` descriptor duplication with
`POSIX_SPAWN_CLOEXEC_DEFAULT`; the rejected Rust `pre_exec` route is not used.
Whether the kernel permits this explicit mapping while preserving CLOFORK
protection for unrelated fork/spawn is an actual native acceptance question.
Failure ends this candidate; never clear the parent's protection to force it.

The previous draft derived its expected signing requirement from the very
sibling it intended to authenticate. The new candidate starts with a **trusted
launched TCB**: validate the current live executable's fixed own identifier and
Apple certificate anchor before reading signing metadata. Extract a bounded
alphanumeric TeamID, revalidate the certified organization on self, and derive
only the fixed counterpart identifier/Apple-anchor/same-organization requirement.
Ad-hoc, missing-team, invalid-self and wrong-identifier images refuse.

The temporary signed app bundle and its nested code are checked against that
policy. The current on-disk self hash must match the validated live self hash;
the expected counterpart hash is retained. The intended daemon validates its
opaque parent token jointly with the actual live `getppid()` using Security,
checks the exact counterpart hash and retains a task-name right during validation.
After acquiring that right it performs a fresh joint lookup/hash check.

This self-anchored same-organization policy closes the sibling-controlled-root
mistake under the trusted-TCB premise. It does **not** establish the identity of
the Cryptex publisher after the entire TCB is re-signed, approve a release
certificate, prove notarization or authenticate an atomic bundle snapshot.

## Exact test inventory

The positive candidate requires all eleven observations, including successful
normal child exits rather than a supervisor pretending to observe them:

1. A supervisor-killed stalled child is distinct from a child enforcing its own
   read deadline. Supervisor outcome44 cannot satisfy expected child exit42.
2. Wrong-identifier counterpart refused.
3. Ad-hoc counterpart refused.
4. Raw fork excludes both original pipe ends.
5. Unrelated spawn excludes both original pipe ends.
6. Explicitly mapped intended child reads exactly32 opaque bytes plus EOF,
   closes the carrier before framework use and validates its actual live parent.
   The child's protected read end also must not survive its own raw fork.
7. Short handoff refused.
8. Extra handoff refused.
9. Forged parent token refused.
10. Missing EOF refused by the child's bounded read deadline.
11. Fresh child/restart uses a fresh one-shot channel and validates again.

The carrier is not authentication. Token bytes, certificate selectors and TeamID
are not included in the JSON receipt. The collector validates closed schemas,
real booleans, complete conditional inventories, exact case results and package
hashes. Failed, interrupted and CI-only executions cannot retain a positive
`candidate_complete` claim. Normal developer/runtime data is never opened.

## Verification performed

- [Mac CI42911736801](https://github.com/gungortarik/cryptex/actions/runs/42911736801)
  on exact source b234322: **SUCCESS**, macOS26.7.2/SDK26.5 arm64.
- Native C compiled with warnings/availability errors enabled and the older
  deployment target. CI's ad-hoc signed self explicitly refused at the Apple
  anchor/fixed-self-identity check. The common supervisor-timeout distinction ran
  successfully. Public atomic headers/runtime were unavailable on this host.
  Therefore `candidate_complete=false`; **positive native cases NOT RUN**.
- Python suite inventory39: Mac38 passed/one Windows-only skip; local Windows34
  passed/five native-Mac skips. The new candidate suite has nine cases, including
  package tampering, receipt corruption, failure preservation, impossible CI
  acceptance and native temporary-directory owner/name validation.
- Existing raw Mach/XPC/API checks in that scoped workflow still ran. They do
  not convert this candidate's unavailable positive cases into passes.
- Source-only independent review found/corrected timeout-result conflation,
  failed-F_GETFD masking and inconsistent error-state completion reporting.
  Final review found no actionable blocker for the bounded candidate package.
  It did not execute code or accept production integration.

An earlier run42911153956 failed before native execution because Python's
`mkdtemp` alphabet can contain `_`, while the strict owned-work-directory grammar
allows alphanumerics. The allocator now creates an exclusive0700 directory with
a24-character OS-random hex suffix. The validation guard was not relaxed. All
three affected CI collectors use it; the native regression verifies the unchanged
owner/mode/path/name guard. Existing owner packages/receipts remain historical.

Native C SHA256:
`2c51c9850a7d36eba77154040317b75eacad2fb11dbec6807ae9204191ded259`.

Primary references: [Apple Swift System API and macOS27 availability](https://github.com/apple/swift-system/blob/main/Sources/System/FileDescriptor.swift),
[Apple's atomic-descriptor motivation and API proposal](https://github.com/apple/swift-system/blob/main/Proposals/0007-FileDescriptor-dup3-pipe2.md),
[Security dynamic-code metadata validation contract](https://github.com/apple-oss-distributions/Security/blob/main/OSX/libsecurity_codesigning/lib/SecCode.h).

## Owner package and precise continuation

Outgoing source-bound package:
`C:\Users\Tarik\Downloads\cryptex-mac-baglanti-b2343229.zip` (21931 bytes).
SHA256: `1f0793e1ddefc137b6baced7e1dbb573d4636d87b4474f95b6f44b1668d1df01`.

Personal Mac: extract -> `MAC-BAGLANTI.command` -> return
`MAC-BAGLANTI-SONUC.zip`. One existing-key codesign permission may need Allow
for this operation. No new certificate/private-key export, permanent permission,
launchd service, production endpoint, profile or canonical memory is involved.
No Windows VM action or repetition of passed Mach/pairing/API packages is needed.

Verify the returned exact source/native hash, public-header/runtime eligibility,
Apple-signed sealed-bundle eligibility, complete eleven-case inventory and runner
status. Missing support/signing prerequisites are incomplete evidence, not pass.

Do not activate production after a positive candidate alone. Remaining gates:
accepted minimum-OS/platform and Mac protocol policy; real Desktop/daemon
bidirectional instance pinning; persistent pin lifetime; concurrent bundle
replacement/snapshot handling; actual Unix connection and forwarded-descriptor
boundary; owner-session close/expiry/cancel/restart acceptance. The current
candidate retains its task right for one validation only. Normal Mac owner
mutation gates, existing Unix IPC and all production binaries stay unchanged.
