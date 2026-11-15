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
used; personal memory is not transferred. Physical owner acceptance is pending.
No repeated terminal paste or unchanged historical packet is requested.

Ordinary configured-image/release policy, actual peer/folder synchronization,
cross-device durable replicas, installed-product/UX and final protocol/M8
acceptance remain separate gates. No release signer, paid membership, schema
acceptance, production activation or M9 start is implied.
