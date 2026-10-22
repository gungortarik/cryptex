# M7 P1 Obsidian write-back spike — Windows partial result

**Date:** 2026-10-14  
**Source SHA:** `74bc3b14127962579ae03838bafea9336612a033`  
**Host:** Windows 11 Pro `10.0.26300`  
**Obsidian:** `1.14.4`  
**Cryptex daemon:** isolated release binary, PID `40272` (stopped after the run)  
**Scope:** synthetic vault only; no personal vault or canonical memory writes.

## Observed result

| Probe | Result |
|---|---|
| Obsidian plugin → isolated daemon `health.ping` over the existing named pipe | **PASS** — plugin recorded `health.pong`, daemon PID `40272` |
| `Vault.process()` synthetic projection refresh | **PASS** — marker refreshed twice |
| Projection loop suppression | **PASS, scoped** — `ownWriteSuppressed=2`; neither refresh was counted as a user edit |
| Manual synthetic edits | **PASS, scoped** — two edits produced `userModify=2` |
| First-run event counts (before plugin reload) | `modify=4`, `ownWriteSuppressed=2`, `userModify=2`, `create=0`, `rename=0`, `delete=0` |
| Create/rename listener probe | **PASS, scoped** — one synthetic note created and renamed through Obsidian `Vault` APIs; `create=1`, `rename=1` |
| Unrelated `owner-sentinel.md` | **PASS** — SHA-256 remained `C0CD8B0F544E3FC65638674DF720B974B1615446DADBDBD286535F6CDB0316D5` |
| Daemon shutdown | **PASS** — isolated process exited after its stop signal |

The spike vault was under `%LOCALAPPDATA%\Cryptex-Dev\m7-writeback-spike-27551853ab304734b2bc7f0e74b0a525\vault`.
The plugin wrote its bounded synthetic counters and IPC result under the vault's
`.obsidian\plugins\cryptex-m7-spike` directory. These files are local probe
artifacts, not production evidence or canonical data.

## Open at the initial 2026-10-14 run (historical)

Later sections record subsequent results. Use the current gate inventory at
the end of this report when continuing; this initial list is not today's status.

- `delete` listener behavior.
- Concurrent edit/conflict behavior during `Vault.process()`.
- Startup/indexing event classification, repeatability beyond the three observed cycles.
- IPC authentication/peer behavior, timeout and malformed-response handling, and
  connection cleanup on plugin unload.
- Path/ownership cases (Unicode, spaces, case, missing file, symlink/reparse boundary).
- macOS parity and the exact Obsidian API-types package/version.

This is a Windows partial result only; it does not authorize M7 implementation.
No Actions, commit, push, or merge was run.

## 2026-10-15 follow-up

The probe plugin was reloaded from the synthetic vault. After reload, its status
reported `listeners ready`. A fresh cycle produced exactly one suppressed
`Vault.process()` modification and exactly one observed manual edit, while
preserving the earlier create/rename counts:

| Counter | Result after reload cycle |
|---|---:|
| `modify` | 2 |
| `ownWriteSuppressed` | 1 |
| `userModify` | 1 |
| `create` | 1 |
| `rename` | 1 |
| `delete` | 0 |

The refreshed plugin connected to isolated `cryptexd` PID `34056` through the
existing Windows named pipe and parsed a valid `health.pong` response. Its binary
SHA-256 was `90FFACE4BF669E92AA917D6EF119EABB97CC91B7E5CACF7A7BB14C863D156344`.
The daemon was then stopped through its isolated runtime stop signal. The
unrelated sentinel SHA-256 remained unchanged.

Create/rename, post-reload event behavior, and plugin IPC are now observed on
Windows. P1 is still incomplete: concurrent write conflict, startup-event
classification, path-boundary cases, and macOS parity have not been
demonstrated.

## 2026-10-15 plugin-unload check

In the synthetic vault, the temporary plugin was disabled through Obsidian's
Community plugins settings. `community-plugins.json` then contained `[]`.
After the plugin unloaded, the synthetic managed note was edited and saved
through Obsidian. The plugin's `event-counts.json` remained unchanged at
`{"modify":2,"ownWriteSuppressed":1,"userModify":1,"create":1,"rename":1,"delete":0}`;
the post-unload edit was visible in the note. The unrelated owner sentinel remained unchanged at SHA-256
`C0CD8B0F544E3FC65638674DF720B974B1615446DADBDBD286535F6CDB0316D5`.

This demonstrates that the observed listeners no longer record vault events
after the plugin is disabled/unloaded. It does not prove that a daemon IPC
connection is closed on unload; the isolated daemon had already been stopped
before this check. The plugin was temporarily re-enabled for the subsequent
delete check and then returned to its original disabled state. No personal vault
or canonical memory was touched.

## 2026-10-15 delete-handler mock check

A one-off Node VM harness loaded the temporary `main.js` with a mocked
Obsidian API surface. It verified that the plugin registers four vault event
references, the registered `delete` callback increments the counter for a
`Cryptex/` synthetic path, and an unmanaged `owner-sentinel.md` delete event is
ignored. Result: PASS within the mocked event API only.

## 2026-10-15 native delete event check

With the plugin enabled only in the synthetic vault, the generated
`m7-lifecycle-1792036821461-renamed.md` fixture was opened and deleted through
Obsidian's command palette after confirming the app's delete dialog. The file
was removed from `Cryptex/Entities`; the live plugin counter became
`delete=1` (`modify=0`, `create=0`, `rename=0` in that post-enable session).
The unrelated sentinel remained byte-identical at SHA-256
`C0CD8B0F544E3FC65638674DF720B974B1615446DADBDBD286535F6CDB0316D5`.
The plugin was disabled again after the check; `community-plugins.json` is `[]`
and no Cryptex process remains. This is a synthetic Windows native event check,
not macOS parity or a proof of trash restoration behavior.

## 2026-10-15 concurrent `Vault.process()` API check

On the same isolated synthetic note, two `Vault.process()` calls were started
concurrently with `Promise.all`; each appended a distinct run-scoped marker.
The final note read contained both markers, and the temporary plugin recorded
`{"ok":true,"operation":"parallel_vault_process","marker_a_present":true,"marker_b_present":true}`.
The unrelated owner sentinel remained unchanged at the SHA-256 recorded above.

This is a narrow API concurrency observation on Windows/Obsidian 1.14.4. It is
not a manually intervening editor-write race and does not establish conflict
resolution semantics for an edit made while a projection update is in flight.
The probe's two own modifications were not registered in its loop-suppression
expectation map, so its temporary event counter classified them as two user
modifications; those counters are not evidence of event-origin classification
for this run. The spike plugin was disabled afterwards (`community-plugins.json`
is `[]`). No product source, canonical memory, or personal vault was changed.

## 2026-10-15 startup/indexing observation

The temporary plugin was instrumented to persist its zeroed counters on load,
then Obsidian was reloaded with only the isolated synthetic vault open. After
startup and layout readiness, the saved counters remained all zero:
`modify=0`, `ownWriteSuppressed=0`, `userModify=0`, `create=0`, `rename=0`,
`delete=0`. This shows no startup event was delivered to the registered listeners
in this run. Because the plugin attaches listeners at `onLayoutReady`, this does
not characterize events emitted before that point or prove classification of
every indexing event. The plugin was disabled afterwards (`community-plugins.json`
is `[]`); the owner sentinel hash was unchanged and no Cryptex processes remained.

## 2026-10-15 manual editor conflict and path API follow-up

**Source SHA:** `74bc3b14127962579ae03838bafea9336612a033`  
**Host:** Windows 11 Pro `10.0.26300`  
**Obsidian:** `1.14.4`  
**Temporary plugin:** manifest `0.0.1`, `minAppVersion=1.5.0`; plain JavaScript, no `obsidian` or `@types/obsidian` package was used.  
**Scope:** isolated synthetic vault; temporary plugin only; no production code.

The async conflict probe read the managed synthetic fixture with
`Vault.cachedRead()`, paused for a manual editor change, then used
`Vault.process()` to compare the current value with the baseline. During the
pause, the note was edited and saved through Obsidian. Result for run
`1792068479835`: `{"conflict_detected":true,"manual_marker_present":true,"plugin_marker_present":false,"overwrite_prevented":true,"outcome":"intervening_edit_preserved"}`. The probe therefore preserved an intervening edit by returning the current content. This
is a scoped Windows observation of the compare-and-preserve strategy, not proof
that `Vault.process()` itself reports conflicts or that the production plugin
policy is finalized.

The same temporary plugin created and renamed a unique path containing spaces,
Unicode and case-only differences:
`Cryptex/Entities/M7 Spïke Éntity 1792068348674.md` to
`Cryptex/Entities/m7 spïke éntity 1792068348674.md`. Exact-case lookup,
missing-path lookup, content preservation, API deletion and post-delete
absence all passed. Alternate-case `getAbstractFileByPath()` lookup returned
absent in this Obsidian run; this records the Vault API behavior and does not
assert underlying Windows filesystem semantics. The plugin's exact-marker
cleanup then removed only `M7MANUALINTERVENING1792068479835`; a readback
confirmed all other note content matched the expected content byte-for-byte.

A Windows directory junction inside `Cryptex/Entities` pointed to a fresh
synthetic directory outside the managed subtree. During a 12-second observation
window, `Vault.create("Cryptex/Entities/m7-boundary-junction/probe-1792068882020.md", …)`
created a 47-byte file in that external target; the plugin read it back through
Obsidian and then deleted it through `Vault.delete()`. The target was verified
empty afterward. This proves that the Vault API followed this junction and that
a `Cryptex/` path-prefix check alone does not enforce physical containment.
A separate one-off Rust metadata probe against a Windows junction returned
`symlink_metadata`: `is_dir=false`, `file_type().is_symlink()=true`,
`file_attributes=0x410`. Source inspection shows M6's `validate_owned_tree`
rejects non-directories and symlinks, and its recursive scan rejects symlinks;
that function was not run against this fixture, so this is source-aligned
supporting evidence, not an end-to-end M6 guard test.

A read-only check inside the actual Obsidian plugin runtime then used
`adapter.getBasePath()` / `adapter.getFullPath()` plus `fs.lstatSync()` on each
path component. It detected the junction ancestor at component 3 with
`isSymbolicLink()=true` and stopped before inspecting the nonexistent child.
This demonstrates that the plugin runtime can identify this Windows junction
before calling a Vault note operation. It is only a metadata-check prototype:
no race-free guarantee between `lstat` and a later Vault API call was tested,
and this is not an accepted product architecture.

Afterward the temporary plugin was disabled (`community-plugins.json` is `[]`),
the junction and empty target directory were removed after checking their exact
paths and target, Obsidian remained open, no `cryptex*` processes remained, and
the unrelated `owner-sentinel.md` SHA-256 remained
`C0CD8B0F544E3FC65638674DF720B974B1615446DADBDBD286535F6CDB0316D5`.

## 2026-10-15 actual-plugin static guard check

**Source SHA:** `74bc3b14127962579ae03838bafea9336612a033`  
**Host:** Windows 11 Pro `10.0.26300`  
**Obsidian:** `1.14.4`  
**Temporary plugin:** manifest `0.0.1`; plain JavaScript, no
`obsidian` / `@types/obsidian` package  
**Scope:** same isolated synthetic vault; no production source or canonical
memory changes.

The temporary plugin's existing junction command was replaced with a
fail-closed preflight that computes the path under the vault and `lstat`s each
existing parent before any Vault mutation call. With a fresh Windows junction
at `Cryptex/Entities/m7-boundary-junction` pointing to an empty synthetic
directory outside `Cryptex/`, the command
`M7 Spike: synthetic junction boundary probe` reported:

```json
{
  "checked_components": 3,
  "reparse_detected": true,
  "blocked_before_vault_api": true,
  "vault_api_calls": 0,
  "target_absent": true,
  "reparse_component": "m7-boundary-junction",
  "reparse_is_symbolic_link": true,
  "reason": "reparse_ancestor"
}
```

Run ID: `1792084154317`. The probe result's `vault_api_calls: 0` is an explicit
output value, not an intercepted-call counter; the guarded command contains no
managed-note Vault mutation call. The external target remained empty. The
plugin was disabled afterward (`community-plugins.json` returned to `[]`); the
exact junction and empty target were removed. `owner-sentinel.md` remained byte
identical at SHA-256
`C0CD8B0F544E3FC65638674DF720B974B1615446DADBDBD286535F6CDB0316D5`.

This verifies the static preflight in the actual Obsidian plugin runtime, not
physical containment across a race: a reparse point could be swapped after
`lstat` and before a later Vault API call. The Vault API's previously observed
link-following behavior remains a risk. No accepted product link/race policy
exists, so this does not close P1 or authorize M7 product implementation.

## 2026-10-15 pre-layout startup event observation

**Source SHA:** `74bc3b14127962579ae03838bafea9336612a033`  
**Host:** Windows 11 Pro `10.0.26300`  
**Obsidian:** `1.14.4`  
**Scope:** same isolated synthetic vault; event type/path only, no note bodies.

A temporary observer registered `create`, `modify`, `rename`, and `delete`
listeners during plugin `onload`, before `workspace.onLayoutReady`. On two
consecutive app reloads it saw the same two pre-layout `create` events:

```json
[
  { "event": "create", "path": "Cryptex/Entities" },
  { "event": "create", "path": "Cryptex/Entities/synthetic-entity.md" }
]
```

No pre-layout `modify`, `rename`, or `delete` events were observed. Run IDs:
`1792084461742` (counts-only repeat) and `1792084532068` (paths captured).
When the plugin was enabled in the already-open vault, the immediate
`onLayoutReady` callback recorded zero pre-layout events (`1792084439757` and
`1792084511693`). This supports waiting for `onLayoutReady` before installing
the user-edit listeners: initial vault population emits `create` events before
that hook and must not be attributed to user edits. The observation is Windows
and Obsidian-version scoped; it does not establish macOS parity or classify
every possible startup/indexing event.

## 2026-10-15 IPC socket close-on-unload fixture

**Source SHA:** `74bc3b14127962579ae03838bafea9336612a033`  
**Host:** Windows 11 Pro `10.0.26300`  
**Obsidian:** `1.14.4`  
**Scope:** actual temporary Obsidian plugin in the synthetic vault; local
no-response named-pipe fixture; no real daemon or memory data.

The plugin opened the existing Cryptex named-pipe path, sent a bounded synthetic
`health.ping` request, and remained pending because the local fixture withheld
its response. Disabling the plugin through Obsidian's Community Plugins UI
unloaded it. The recorded result for request
`obsidian-m7-ipc-unload-1792084911601` was:

```json
{
  "connected": true,
  "request_sent": true,
  "response_received": false,
  "closed": true,
  "closed_on_unload": true
}
```

The first attempt (`1792084788222`) reached its 15-second timeout before the
plugin was unloaded and is not a cleanup pass. The repeated 60-second attempt
was unloaded while pending and closed on unload. Afterwards
`community-plugins.json` was `[]`, the local mock server was closed, and the
synthetic `owner-sentinel.md` SHA-256 remained
`C0CD8B0F544E3FC65638674DF720B974B1615446DADBDBD286535F6CDB0316D5`.

This is scoped evidence that the actual plugin cleanup closes its pending local
socket. The fixture is not a real `cryptexd`; it does not establish peer
authentication, real-daemon in-flight cleanup, timeout/malformed-response
coverage, or macOS parity.

The Windows observations now include manual-intervening-edit preservation,
basic Unicode/space/case/missing-path behavior, pre-layout startup events, and
pending local-socket closure when the actual plugin unloads. The junction result
remains a demonstrated ownership-boundary risk, not a pass; a safe containment
policy and implementation remain open. Still open: macOS parity (including
startup and unload behavior), real-daemon authentication/peer and in-flight
cleanup coverage, API-types package/version inventory, and repeatability/failure
cases not covered above. P1 remains open; M7 product implementation is not
authorized. A later local fixture covers the bounded valid/malformed/mismatch/
timeout response cases only; it is documented below and is not daemon
interoperability evidence.

## 2026-10-15 read-only daemon IPC authorization source review

At source SHA `74bc3b14127962579ae03838bafea9336612a033`, `create_listener()`
builds the Windows named-pipe security descriptor for the current user's SID
only (`services/cryptexd/src/ipc.rs:978-995`). The read-only `health.ping`
authorizer accepts roles `desktop`, `cli`, or `mcp` when the sole capability is
`status.read`; it rejects other operations and request bodies
(`services/cryptexd/src/ipc.rs:2660-2688`). The temporary plugin's recorded
`health.pong` therefore demonstrates interoperability with the isolated daemon
for this bounded health operation, not distinct Obsidian process identity or
authorization for future proposal/write-back commands. No code or runtime state
was changed for this source review. M7 proposal authorization and real-daemon
in-flight cleanup remain unverified; this does not close P1.

## 2026-10-15 local IPC response robustness fixture

A one-off Node VM harness loaded the actual temporary spike `main.js` and
replaced only its `net` transport with a local fake socket. The cases covered a
valid bounded `health.pong`, malformed JSON, a mismatched request ID, and no
response until the plugin's two-second timeout. All four expected outcomes
were observed: valid response accepted; malformed, mismatched, and timed-out
responses produced the bounded `{ "ok": false }` evidence result and safe
failure notice. This is transport robustness evidence only: it used no
Obsidian UI, real daemon, or canonical data and proves no peer authentication or
daemon interoperability. No repository product code or runtime state changed.

## 2026-10-16 Windows isolated health and unload recheck

At source SHA `74bc3b14127962579ae03838bafea9336612a033`, the existing release
Desktop and daemon were launched with a fresh marker-only pilot runtime under
`%LOCALAPPDATA%\Cryptex-Dev\m7-p1-health-8f0b43a1135d4764a3eff6bab62e5414`.
Desktop SHA-256 was
`A44D133410BC8E6E9591D676450CD263EF4722759C820359C42A3A9D8A847A56`; daemon
SHA-256 was
`90FFACE4BF669E92AA917D6EF119EABB97CC91B7E5CACF7A7BB14C863D156344`.

| Probe | Result |
|---|---|
| Obsidian 1.14.4 plugin → isolated daemon `health.ping` | **PASS** — bounded result `{"ok":true,"operation":"health.pong","daemon_pid":35036}` |
| Managed synthetic note | **UNCHANGED** — remained 362 characters; no write command was run |
| Isolated Desktop/daemon shutdown | **PASS** — `clean_quit`, `clean shutdown released instance lock`; both processes absent afterwards |
| Pending IPC cleanup | **PASS, plugin-side** — local no-response named-pipe fixture received the request; plugin evidence recorded `response_received=false`, `closed=true`, `closed_on_unload=true` |
| Plugin state after check | Restored enabled in the synthetic vault; status returned to `listeners ready` |

The pending-socket fixture did not return a response. The fixture's own polling
did not independently observe peer closure before its helper was interrupted;
the closure evidence is the plugin's close event and unload flag. Event counters
remained zero and no managed note or sentinel was modified. The real-daemon
health result remains generic status-only interoperability; it does not prove
Obsidian peer identity or authorize proposal/write-back IPC. Path containment
through Obsidian Vault APIs remains unresolved, so managed-note writes stay
disabled and M7 implementation remains blocked. No build, tests, Actions,
commit, push, or merge was run for this recheck.

## 2026-10-16 observation-only restriction and ordinary health cleanup fix

The owner reconfirmed the current plan and selected "Mevcut planı koru;
yazma kapalı kalsın". Inspection found that the enabled disposable Windows
plugin still registered older managed-note mutation probes. Its local source
was restricted to observation/status operations before further work.

Changes to this temporary plugin only:

- Only `m7-spike-counts`, `m7-spike-ipc-health`, and
  `m7-spike-ipc-unload` remain registered.
- All seven former managed-note/path probe methods refuse direct invocation.
  Their old Vault read/write code was removed. Event callbacks count metadata
  only while loaded and stop after unload; no managed-note content is read,
  and no event is labelled as a user proposal or a suppressed projection write.
- Ordinary `healthProbe()` sockets now join `activeIpcSockets`, so unload
  closes them as well as the dedicated unload fixture. A peer closing before
  replying now settles the request, and the 65,536-byte bound measures bytes.
  The operation remains generic `health.ping` with only `status.read`.
- Late layout callbacks stop after unload, and pending IPC completion does not
  emit a late UI notice after unload. Plugin metadata evidence writes remain
  enabled; this restriction applies to managed-note IO, not all vault files.

The original source and tested replacement are preserved under the synthetic
root's `proof/observation-only-20261016-a076a5820465/` directory:

| Artifact | SHA-256 |
|---|---|
| `main.before.js` | `002419a005477a35b37aec4fde6bade47a3fe90a3547abf36c8bd49ddb5786c4` |
| staged and installed `main.js` | `722d7a27a2ce66c5340c2ceda9f45a8249d750e1866ed15165231fad3afc136b` |
| `guard.test.cjs` | `e00ca4939dbd62a7ee14768e6028903d4e0046f84e682cd7de65976ff6830040` |

`node --check <proof>/main.js` and `node --test <proof>/guard.test.cjs`
both exited 0. The **11/11** Node VM fixture checks cover the command allowlist,
direct invocation refusal, event observation without note IO, late layout,
bounded status requests and matching responses, malformed/mismatched replies,
UTF-8 byte limits, premature closure, ordinary-health unload, explicit pending
probe unload, and the health timeout. Test doubles refuse any managed-note
Vault call; these checks are not native-daemon or macOS results.

The tested source was installed only in the existing synthetic vault, and
Obsidian 1.14.4 was reloaded through its command palette. Its actual accessibility
tree showed `M7 spike: observation only; note IO disabled`. Filtering the command
palette by `M7 Spike:` showed exactly the three observation/status commands
above. No mutation command was invoked. Screenshot capture was unavailable
(`window crop is outside captured monitor`) and coordinate clicking reported
unavailable geometry; keyboard input and accessibility text supplied the native
status/command inventory. The palette was dismissed afterwards.

The synthetic note stayed at 362 characters with SHA-256
`e8606fb2943b72494432c3c29dbaa4764e70edbfaccaf4ad5a6ffbcad2b3dc12`;
the sentinel hash stayed
`c0cd8b0f544e3fc65638674df720b974b1615446dadbdbd286535f6cdb0316d5`.
The installed source matched the tested source hash. `baseline.json` and
`installed.json` preserve the local installation checks. Obsidian was left
open in the synthetic vault with the restricted plugin enabled.

This is a local disposable-spike correction. It does not solve Vault physical
containment, enable write-back, authenticate an Obsidian peer, or prove native
real-daemon in-flight cleanup. No production source, Actions, commit, push, or
merge changed.

## 2026-10-16 verification of the installed observation-only source

The owner requested a further check of the previous correction. The installed
plugin was reviewed and remained byte-identical to the tested replacement
SHA-256 `722d7a27a2ce66c5340c2ceda9f45a8249d750e1866ed15165231fad3afc136b`.
No additional plugin source change was made during this review.

`node --check main.js` and all **11/11** existing VM fixture checks passed again.
Three new checks then exercised the same source with actual Windows named-pipe
sockets, remapping only the test transport to a unique fixture endpoint:

1. A valid health reply from a server that deliberately kept its side open did
   not retain a client socket or an entry in `activeIpcSockets`.
2. Unloading during an actual pending local health request closed the client,
   settled the operation, and emitted no late notice.
3. The dedicated pending probe also finished and cleared its tracked socket
   when its replying server kept its end open.

All **3/3** real local-transport checks passed (exit 0), and their fixture servers
and sockets were closed by test teardown. A possible half-open connection issue
considered during source review was **not reproduced on Windows**; it is not
recorded as a new defect or a new fix. The fake socket alone had not established
that result. These are operating-system transport fixture checks, not real
`cryptexd` authentication or in-flight interoperability results, and imply no
macOS result.

Artifacts are retained under the same synthetic root at
`proof/observation-only-review-20261016-4b10205d13f2/`: `main.js`,
`main.before.js`, `guard.test.cjs`, `real-socket.test.cjs`, `baseline.json`,
`verification.json`, and `review.json`. The installed/source hashes, only-three-
commands check, and absence of managed-note Vault IO calls all matched. Both
note and sentinel hashes stayed identical to the earlier recorded values.

A fresh accessibility observation of the existing Obsidian 1.14.4 window
confirmed `M7 spike: observation only; note IO disabled` and 362 note characters.
No UI input or managed-note mutation was performed for this verification.
`git diff --check` passed. No Actions, build, production changes, commit, push,
or merge were performed. Physical containment and M7 completion remain open.

## Current open P1 gates (2026-10-16)

| Gate | Current status |
|---|---|
| Physical containment using allowed Vault APIs on both OSes | **BLOCKED** — external link traversal was observed; no tested race-safe design |
| Generic health interoperability | Windows scoped native PASS; not write authorization |
| Ordinary health socket unload | Windows VM and actual local named-pipe fixture PASS; real-daemon pending case still open |
| Obsidian proposal peer/auth | **OPEN** — current daemon has no Obsidian proposal authorization path |
| macOS event startup/unload and transport parity | **OPEN** — only the previously owner-reported partial results are available |
| App/installer/API-types version baseline | Windows runtime inventory now recorded in `m7-p1-observation-portability-windows-20261016.md`; native Mac inventory and selected product minimum remain **OPEN** |
| Remaining native repeatability/failure coverage | **OPEN** — fixture and historical results retain their stated scopes |

The architecture policy is decided and was reaffirmed. M7 remains preparation
only; do not reopen that same policy question or enable managed-note IO based
on these observation-only checks.
