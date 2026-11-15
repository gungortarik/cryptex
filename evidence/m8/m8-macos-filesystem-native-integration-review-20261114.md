# Mac filesystem and existing-profile native integration — November 14

Verified code: `d9561d81af883bcdcc17bc7893a566ac0b305cdf`.
Scope: [bounded contract](../../docs/proposals/m8-macos-import-filesystem-20261114.md).
**M8 OPEN; ordinary activation remains disabled.**

## Change and source review

Descriptor-relative Darwin import operations enforce no-follow, private
ownership/ACL and single-link checks, original directory/lock identity, exclusive
same-volume promotion, flush and guarded recovery/cleanup. Observed namespace
drift never authorizes redirecting cleanup/publication. Supported private writers
retain import, canonical and instance/admission coordination. POSIX descriptors
do not prevent arbitrary same-UID renames; process-interruption tests do not
certify physical power-loss durability.

A distinct default-off ExistingProfile opens an initialized Keychain profile
without seeding, key creation, fixture fault IDs or Unix fallback. The shared
dispatcher consumes sealed native requests and keeps grants connection-local.
The actual response owns admission through delivery. Canonical-capable replies
close new admission before releasing that lease; the same instance performs full
R4 recovery, authenticated peer-slot setup and M3/M4 rebuild before more work.
Original backend device caps survive reconciliation/publication. Ready, status
and domain sends validate the original namespace before and after actual send.

The owned helper reader retains the requested locator and no-follow capability
before canonicalizing a launch locator. It validates them across startup and
actual observations. The OS-owned /var spelling retains its association; user
symlinks/replacements cannot acquire one. Bounded inbox metadata supplies hints,
never publication authority. Status never claims remote protection from helper
liveness/local publication. The selected macOS26 floor and independently pinned
development signer apply. Ordinary configured-image/require_candidate gates are
unchanged.

The separate read-only review agent `/root/independent_m8_review` reviewed through
this exact source and reported no remaining actionable finding in the bounded
slice. It edited no files and executed no tests/CI. This is not an independent
human security certification. Corrected findings include original child/lock and
cleanup bindings, post-domain reconciliation, pre-send checks, persistent device
caps, helper root association, and transport fixture UUID verification against
its actual signed seed while fixed-ID fixture constraints remain.

Native stress reproduced failure of concurrent nonexclusive O_CREAT lock opens.
The replacement uses strict existing lookup, exclusive first creation, and one
strict existing open only on EEXIST. Typed Absent arises solely from raw lookup
ENOENT after original-parent validation. Post-open disappearance cannot recreate
or split a held lock. No Apple kernel root cause, errno-create retry, relaxed
identity/ACL check or weakened test is asserted. Local Windows Rust execution
remains blocked by Application Control4551; no bypass was used.

## Exact-source execution

[Full CI43671790009](https://github.com/gungortarik/cryptex/actions/runs/43671790009)
passed both readiness jobs and Windows cross-account ACL. Daemon library:
Mac589passed/7ignored, Windows682passed/5ignored, zero failures. All ten Darwin
filesystem cases passed, including16x8 first creation, no truncation/link adoption
and disappearing-opened-lock refusal with original flock exclusion. The shared
reply-admission regression passed on both platforms. Format, lint, frontend/status
policy, conformance and release/negative CLI checks passed. Existing ignored
cases are not relabeled as executed.

[Native CI43671787980](https://github.com/gungortarik/cryptex/actions/runs/43671787980)
passed both selected platform jobs, real signed Mac startup/content/session
checks and the exported durable, transport and combined-loss launchers. The
changed signed harness completes a new pairing in ExistingProfile, imports
signed incoming bytes without restarting, confirms an exact permitted cutoff,
checks fresh status/approval, reopens/replays without event changes and rejects
old on-wire confirmation/local handles. Desktop copies only untrusted transport
test bytes; the daemon alone publishes canonical events. This is a local
two-engine test, not cross-device Syncthing folder synchronization.

The [structured receipt](m8-macos-filesystem-native-integration.json) pins source,
jobs, tests, markers and relevant source-file SHA256 values. Earlier failures are
superseded within the corrected, executed paths; older owner receipts do not
establish acceptance of this changed code.

## Owner handoff and remaining gates

Exact-source Mac CI exported and executed `cryptex-mac-transport.zip`. Its unchanged
24,224,511 bytes are delivered as `Cryptex-Mac-Profil-d9561d8-70db1c49.zip`, SHA256
`70db1c49e4e37e53236794737c673d84d4c103c87b816fd93f982be22e7955e4`.
Target: arm64; expiry: 2026-11-16T03:21:56+00:00. Local inspection checked 15 safe
unique members, executable launcher and 9 signed-inventory file sizes/digests.
Native signatures/execution were verified on Mac CI, not Windows.

Move the ZIP to the Mac, extract there, double-click
`Cryptex-Mac-Aktarim/MAC-AKTARIM.command`, keep test windows open and return the new
`MAC-AKTARIM-SONUC-*.zip`. Fresh private profiles/fenced credential cleanup are
used; personal memory is not transferred.

Owner return `MAC-AKTARIM-SONUC-1bcdf362.zip` (SHA256
`d314e353d0c5a584dd0277ec7eef5b49f71fb2876dd6092e3090505f954e1b4e`)
reports source `d9561d8`, identity restart/refusal success and
`real_native_managed_pairing_failed`. It contains only the aggregate JSON;
exit status, timeout/output flags and native substep are unavailable. Physical
owner acceptance **failed**; its cause is unconfirmed. The CI successes above
do not supersede this receipt. Do not infer pairing, import or later parent-loss
success. Preserve the original failed runtime on the Mac.

Next handoff uses the same signed app bytes with a diagnostic-only collector.
It records fixed allowlisted codes/markers and command outcome flags/timing,
never raw stdout, arguments, paths, memory bodies or Keychain values. Timeouts,
cleanup fencing and acceptance checks are unchanged. Seven new collector tests,
ten existing checker tests and five package-export tests pass locally. Their
mocked processes are not native execution evidence. No repeated terminal paste
or unchanged historical packet is requested.

Diagnostic archive: `Cryptex-Mac-Tani-d9561d8-b3014b96.zip`, SHA256
`514e153692a2bba679c2ca24e50870b93eb269ca564601866e93334d10ef7c06`,
24,226,987 bytes, 17 safe members. Collector source `aadd078`, executable at
`Cryptex-Mac-Aktarim-Tani/MAC-AKTARIM.command`. All11 app members match the original
archive byte-for-byte,9 inventory entries reverified, helper bytes match Git.
Expiry remains2026-11-16T03:21:56+00:00. Read-only agent review found no actionable
collector/test-delta finding. The owner returned the following failed diagnostic.

November15 return: `MAC-AKTARIM-SONUC-f7684e37.zip`, SHA256
`763b65b071420f362c14ffe48eb595e216ae11875e01de594fac034bbd388be3`,
matches the collector hash. Its [aggregate receipt](m8-macos-owner-rejection-20261115.json)
shows exit2 after20526ms, `existing_profile_incoming_Rejected`, and no timeout,
output-limit or pipe failure. Identity/mutation/cleanup passed. The inner reason
is still unconfirmed. The new collector adds one existing validation-only CLI
invocation after that exact failure, within the remaining original240s budget
and before exact-key cleanup. It uses fresh fixture paths only, never publishes,
and preserves both the original failure and unresolved-writer cleanup fencing.
Fourteen owner tests, ten checker tests and five export tests pass locally;
read-only source review found no actionable diagnostic-delta issue. This is not
native execution evidence for the new branch or a repair of the rejection.

Ordinary configured-image/release policy, actual peer/folder synchronization,
cross-device durable replicas, installed-product/UX and final protocol/M8
acceptance remain separate gates. No release signer, paid membership, schema
acceptance, production activation or M9 start is implied.
