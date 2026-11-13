# Disabled Mac transport: independent source review

The owner previously requested an independent M8 security review and delegated
agent versus external reviewer selection. The existing independent review agent
performed a read-only source review, without edits, CI dispatch or test execution.
This is a separate agent review, not an external human audit or owner-device run.

## Findings and corrections

At `04fa52eb6b8ba527036a07b9eee5b49a89b80d15`, two P2 findings remained despite
passing scoped CI:

- Live observations did not revalidate persisted profile/config/certificate/key
  and receipt continuity. `14e8839` adds before/after profile and pinned-package
  validation, including semantic identity/intent/receipt equality, exact markers,
  strict XML, hashes and OS credential binding. Eleven mutation cases require
  real initial identity, refusal, verified worker cleanup and unchanged corrupt
  bytes before an explicit fixture-only restore.
- A bounded identity-driver loss could skip Rust tail cleanup and leave its
  generated Dev API key. The first outer cleanup correction still raced a late
  helper provisioning write. `0c360c1` / `dbb3657` fence deletion and final typed
  Missing on the same exclusive ManagedProfile lease. The helper rechecks its
  captured parent's liveness after lease acquisition and before provisioning.
  Raw unresolved driver exits refuse cleanup. No PID adoption or raw CLI fallback
  can replace this fence. Real-lock tests cover an earlier gated writer, a delayed
  dead-parent zero-put path and lock retention through both already-Missing gets.

Focused review through `b3ee9bad0f0afb46101160e93eb4b85d3fb30ea4` found no remaining
actionable blocker in these corrections or their orchestration. The closed 6+5
mutation batches preserve all eleven cases and existing individual deadlines;
concurrency is capped at two, both futures and finally cleanup settle even when
one fails, and the aggregate marker requires both exact case-count markers.
The existing exported 360-second watchdog remains unchanged. Native helper wire
commands remain Observe/Stop only. Pairing API and parent-loss API cleanup use
the same signed profile-lease cleanup driver.

The review also inspected pre-lifecycle helper role/purpose/epoch/root binding,
captured stopped image and unreaped group ownership, established reverse socket
matching before credentials, fresh status-reader behavior, signed pairing reply
contracts, fixed-stop shutdown, package whitelist and aggregate-only collector.
No additional concrete blocker was reported in this disabled slice.

## Evidence limits and continuation

Follow-up source review through `74b95bd711c89277d360dc68861a1089bf1be2ac` found
no actionable finding in canonicalizing backend-owned fixture locators before
driver/batch/cleanup calls. Native no-alias, ownership, lock and signature
validation stays unchanged. The regression requires physical root arguments.

Local format, strict Windows daemon clippy and 87 Python tests pass (5 native
platform skips). Exact-source [CI 43330307492](https://github.com/gungortarik/cryptex/actions/runs/43330307492)
is pending at this review checkpoint; source review does not establish execution.
The prior unsplit 180-second identity aggregate timed out after nine successful
drift refusals, so it supplies no complete mutation-suite acceptance. No native
deadline was raised to accommodate the split.

Only a reviewed exact-source passing artifact may proceed to the owner's Mac.
Combined supervisor loss, other endpoint/exec races, publisher/product support
floor and activation decisions remain open. This review does not activate the
ordinary product routes, prove cross-device replication or close M8.

## Disposable signing lifecycle follow-up

Both attempts of `43330307492` passed all new native checks but timed out at
the older `resign_wrong_identifier` codesign fixture. Focused read-only review of
`0d98a16d075ca317b208c6e3dd6b0c66b64df707` found no actionable issue in unlocking
only the exact disposable CI keychain immediately before certificate use.
Unlock failure prevents signing; ad-hoc negative signatures, flags, deadlines,
redaction and outer cleanup remain unchanged. The three new regressions cover
ordering, failure and the ad-hoc exclusion. No review tests were executed.

This maintenance change does not establish idle relocking as the cause of the
observed hangs. Actual exact-source [CI 43334196206](https://github.com/gungortarik/cryptex/actions/runs/43334196206)
subsequently passed both selected platform jobs, all native/mutation/refusal
checks and the exported transport launcher, including all five matching-source
receipt fields. The codesign negative fixtures passed on that run. The
[structured execution receipt](macos-transport-native-0d98a16-ci.json) records this
separately from source review. Owner-Mac execution and declared product gates
remain pending.
