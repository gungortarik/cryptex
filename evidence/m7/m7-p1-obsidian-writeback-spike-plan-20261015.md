# M7 prerequisite spike plan — Obsidian controlled write-back

**Date:** 2026-10-15
**Status:** In progress; Windows and owner-reported macOS synthetic-only partial results include manual editor conflict observations and demonstrated link-boundary risk; actual Windows plugin runtime has scoped preflight and pending-socket-unload observations, while race-safe containment and cross-platform P1 claims remain open
**M6 product base:** `74bc3b14127962579ae03838bafea9336612a033`

## Goal

Resolve the required P1 spike in `docs/23_REQUIRED_TECHNICAL_SPIKES.md` before
M7 productization. Prove that an Obsidian plugin can safely observe edits to
managed notes, use Obsidian's safe file-processing API, suppress projection
loops, and communicate with a local Cryptex daemon through the existing
supported IPC boundary.

This spike must not implement the M7 proposal/review/apply workflow or write
canonical events. It is a disposable technical probe using synthetic data.

## Guardrails

- Use a fresh synthetic Obsidian vault outside the Git checkout and outside any
  personal vault. Include one generated-note fixture and an unrelated sentinel.
- Never modify canonical events, the M4 model, M6 renderer semantics, or the
  production daemon's write capabilities.
- Exercise the existing local IPC transport. Do not add a public or loopback
  HTTP endpoint as a workaround. If the plugin cannot use the existing
  transport safely, record the blocker and alternatives; do not choose a new
  architecture in the spike.
- Use Obsidian Vault APIs for note operations. Do not use direct filesystem
  writes for managed notes in the plugin.
- Capture only synthetic note contents and bounded reason/status diagnostics;
  no memory bodies, secrets, or personal vault data in logs.
- Run on both Windows and macOS. Record Obsidian version, OS, plugin API types
  package/version, Cryptex source SHA, and exact test commands.

## Experiments and pass criteria

1. **Event lifecycle:** Subscribe to relevant vault events after initial vault
   loading, observe a user edit to the synthetic managed note, and prove
   listeners are removed when the plugin unloads. Startup/indexing events must
   not be misclassified as user edits.
2. **Safe update:** Apply a deterministic synthetic update using
   `Vault.process()` (or the current documented equivalent). A concurrent
   intervening edit must not be silently overwritten; report the API behavior
   and observed conflict result.
3. **Loop suppression:** Cause a Cryptex-originated projection refresh and
   prove it does not become a user-edit proposal or trigger recursive IPC.
   Then edit the note manually and prove that edit is observed exactly once.
   Do not select a permanent origin marker based only on the spike; record the
   evidence and remaining tradeoffs.
4. **Plugin-to-daemon IPC:** From the actual Obsidian plugin runtime, connect
   to an isolated real `cryptexd` instance using its existing local IPC
   transport and an already implemented bounded read-only operation. Verify
   peer/auth behavior, typed request/response, and unload/close cleanup. Use a
   separate protocol test fixture for timeout and malformed-response cases;
   label those as transport robustness tests, not daemon interoperability.
   Neither path may mutate canonical data or add a production daemon command.
5. **Path and ownership safety:** Exercise spaces, Unicode, case differences,
   rename/delete, missing managed file, and a symlink/reparse-point boundary
   where supported. Confirm the unrelated sentinel stays byte-identical.
6. **Repeatability:** Repeat projection refresh and user edit cycles; event
   counts and final synthetic note content must be deterministic, with no
   feedback loop or duplicate proposal signal.

## Required evidence

Produce one report with environment/version details, source SHA, commands,
observed event/IPC trace (redacted and synthetic), pass/fail table per OS,
cleanup confirmation, and any unsupported behavior. Resolve each of the four
P1 claims individually. A one-OS result or a mock-only IPC result is not a
cross-platform spike PASS.

## Current status

Windows has a synthetic-only partial result at
`evidence/m7/m7-p1-windows-writeback-spike-partial-20261014.md`: the actual
Obsidian plugin reached an isolated daemon's read-only health IPC; repeated
`Vault.process()` refreshes were suppressed from user-edit counting; manual
edits were observed; `Vault.create`/`Vault.rename` events were counted; and
disabling the plugin stopped further listener counter updates after a synthetic
note edit; an enabled synthetic-vault run then observed the native `delete`
event exactly once, with the plugin returned to disabled state afterwards. The
  unrelated sentinel remained byte-identical. Follow-up Windows observations
  cover pre-layout startup creates and pending local-socket closure on plugin
  unload, but do not establish daemon peer authentication, real-daemon in-flight
  cleanup, macOS parity, race-safe path-boundary enforcement, or complete
  cross-platform P1 coverage.

Owner-reported macOS follow-up is recorded at
`evidence/m7/m7-p1-macos-writeback-spike-partial-20261015.md`. It reports that
`Vault.process()` preserved an intervening manual edit, while `Vault.create()`
followed a symlink from `Cryptex/Entities/` into an external synthetic target.
A read-only `lstatSync()` walk detected the symlink before the Vault operation,
but does not establish race-free containment. The Mac result used source
`87e9c4b0a49c5b670bdd7c6812d5a7c3fa768219`, an ancestor of but not identical to
the current M6 product base `74bc3b14127962579ae03838bafea9336612a033`. The
reported Obsidian versions also disagree (plugin plist `1.13.7`, window title
`1.14.4`); API-types package/version and exact commands were not supplied.

Both OS observations therefore strengthen the finding that a Vault path prefix
does not enforce physical containment, but neither proves a race-safe policy.
P1 remains open and M7 product implementation has not started. Other open
claims include plugin-to-daemon IPC authentication and real-daemon in-flight
cleanup, macOS startup/unload parity, loop-suppression repeatability, the plugin
API-types/version baseline, and complete repeatability/failure coverage. A
one-OS or partial result is not a P1 pass.

A 2026-10-15 local Node VM transport fixture subsequently exercised the actual
temporary plugin's `healthProbe()` with a valid response, malformed JSON,
mismatched request ID, and a no-response timeout; all expected outcomes passed.
This closes those four bounded client-response cases only. It is not Obsidian or
daemon interoperability evidence and does not close peer authentication,
real-daemon in-flight cleanup, or the remaining repeatability/failure matrix.
Details are in `evidence/m7/m7-p1-windows-writeback-spike-partial-20261014.md`.

A later Windows API-level check ran two concurrent `Vault.process()` calls on the synthetic note and observed both appended markers in the final content. That earlier check was not a manual editor-write race; the probe event counters were not valid loop-suppression evidence.

An initial post-layout-only observation left registered event counters at zero,
but did not cover events before `onLayoutReady`. A temporary early observer
then counted startup vault events before that hook across two Windows app
reloads. Both runs emitted exactly two `create` events under the synthetic
managed prefix: `Cryptex/Entities` and
`Cryptex/Entities/synthetic-entity.md`; there were no pre-layout
`modify`/`rename`/`delete` events. The normal plugin listener currently attaches
at `onLayoutReady`, which excludes those initial population events from user
edit counts. Evidence and run IDs are in
`evidence/m7/m7-p1-windows-writeback-spike-partial-20261014.md`. This closes the
observed Windows startup case only; macOS parity and other startup/indexing
event cases remain open.

A separate Windows fixture used a local no-response named-pipe server to keep a
bounded synthetic request pending while the actual temporary Obsidian plugin
was unloaded. The pending socket closed with `closed_on_unload=true`; the first
short-deadline attempt had timed out before unload and was not counted. This is
transport-cleanup evidence only, not real-daemon interoperability or
authentication evidence. Full details are in the Windows partial report.

## Containment-policy review

The accepted integration contract requires safe use of Obsidian's Vault API
rather than arbitrary direct filesystem manipulation for Obsidian-managed
content, and says the exact API/plugin design must follow the P1 spike
(`docs/13_OBSIDIAN_INTEGRATION.md`, §10). The M7 contract excludes unrestricted
filesystem access from the plugin. Current official Vault API guidance
recommends `Vault.process()` for read-modify-write and says it protects against
changes between reading and writing. Obsidian's plugin checklist also says to
prefer the Vault API over the Adapter API. Neither document states that Vault
operations enforce physical symlink/reparse-point containment ([Vault API](https://docs.obsidian.md/Plugins/Vault), [plugin API checklist](https://docs.obsidian.md/oo/plugin)).
The Windows and owner-reported macOS probes directly observed Vault operations
following links outside the managed path prefix.

### 2026-10-15 Windows static preflight prototype

In a newly created temporary subtree inside the synthetic vault, a standalone
Node harness tested a component-by-component `lstat` preflight against a real
Windows junction. It allowed a normal synthetic file target and rejected the
junction path at the `junction` component before invoking the simulated Vault
operation; the external target remained empty. The exact test-created file,
junction, and empty directories were removed after checking their paths and
contents. The Obsidian plugin remained disabled. This was a helper prototype,
not an Obsidian API/plugin integration result, and it explicitly made no
race-free claim.

### Actual Obsidian plugin static-guard check

The same static preflight was exercised from the actual temporary Obsidian
plugin runtime on Windows 11 Pro `10.0.26300` / Obsidian `1.14.4`, against a
fresh junction in the synthetic vault. It detected the junction at parent
component `m7-boundary-junction`, set `blocked_before_vault_api=true`, recorded
`vault_api_calls=0` (an explicit result value; the guarded command contains no
managed-note Vault mutation call), and confirmed the target absent and external
directory empty. The plugin was disabled and the exact junction/empty target removed;
the synthetic sentinel hash matched its prior value. Full result:
`evidence/m7/m7-p1-windows-writeback-spike-partial-20261014.md`, run
`1792084154317`. This is actual-plugin static-link evidence, not a
race-safe-containment result. The temporary plugin was not an M7 product
package and no production source changed.

Do not treat the `lstat` preflight as race-safe, and do not work around the
Vault-API requirement by silently switching managed-note operations to direct
Node filesystem calls. On 2026-10-15, the owner selected the conservative
policy: physical managed-boundary containment is required, and managed-note
write-back stays disabled until a Vault-API-compliant design proves it on both
Windows and macOS, including symlink/reparse-point cases. This resolves the
policy choice, not the technical implementation. P1 remains open; no M7 product
implementation is authorized until the proof exists.

### Owner reconfirmation and observation-only spike (2026-10-16)

The owner selected "Mevcut planı koru; yazma kapalı kalsın" after the
Desktop-only replacement was offered. Preserve the accepted M7 Obsidian
workflow and the physical-containment gate. Do not interpret the request to
continue as authorization to move managed-note editing to Desktop or weaken
the gate.

The Windows temporary plugin still exposed former synthetic mutation commands.
Its local source was restricted to observation and status operations: only
event counts, health IPC, and the bounded pending-socket cleanup probe remain
registered. All seven former mutation/path methods refuse direct invocation,
and event handlers no longer read managed-note content or identify events as
user proposals. Ordinary health requests now join the unload socket set and
settle on early closure. Syntax and 11 Node VM fixture checks passed; an actual
Obsidian reload confirmed the status and three-command inventory. Note and
sentinel hashes were unchanged. Plugin metadata evidence writes remain enabled;
this is not a claim that the entire vault is read-only. Details and local
backup/source/test hashes are in the Windows report.

This source is a disposable synthetic spike outside the checkout. No M7 product
package, proposal IPC, production capability, or architecture change was added.
Fixture cleanup results do not close the real-daemon in-flight gate, and no
macOS result is implied. Continue only observation/status work while the
containment requirement remains unresolved.

### Reproducible observation source (2026-10-16)

The restricted local spike is now reproduced under
`scripts/spikes/m7-obsidian-observation/`, with tests and an English bounded
Mac handoff. It adds runtime/API metadata inventory, a synthetic-scope opt-in,
and platform endpoint selection matching the existing daemon. Managed-note IO
and all former mutation commands stay disabled. The source is prerequisite
diagnostic tooling, not an M7 product plugin or proposal implementation.

Windows checks passed 18/18 (15 VM and 3 actual local named-pipe fixtures), and
the reloaded synthetic Obsidian plugin recorded runtime API `1.14.4` with no
API-types package because it uses plain CommonJS. The product minimum remains
unselected. A hash-verified local transfer permits Mac startup/lifecycle/version
checks without touching a dirty checkout. Native macOS execution has not been
performed here. Physical containment and all distinct real-daemon peer/auth
claims remain open. Exact artifacts and source hashes:
`evidence/m7/m7-p1-observation-portability-windows-20261016.md`.

### Public API surface review (2026-10-15)

The current official public declarations expose `Vault.process()` and adapter
path-based `stat()` operations, but no documented no-follow flag or
file-handle-bound operation that would pin a managed path against link
replacement. The Vault guidance describes protection against content changing
between read and write, not physical path containment. This review does not
prove that no compliant design exists; it means the current public API
references do not provide the missing containment proof. Keep write-back
disabled and P1 open until a tested design demonstrates it.

References: [Obsidian API declarations](https://github.com/obsidianmd/obsidian-api/blob/master/obsidian.d.ts),
[Vault API guidance](https://docs.obsidian.md/Plugins/Vault), and
[plugin API checklist](https://docs.obsidian.md/oo/plugin).

### Obsidian link behavior source review (2026-10-16)

Obsidian's own help says symlinks and junctions can place files outside the
vault into the vault tree, and strongly warns that their use can cause data
loss, corruption, or crashes. This matches the Windows and reported macOS
spike observations where Vault APIs followed a link outside `Cryptex/`.
Therefore a vault-relative path and the supported Vault API alone do not
establish the selected physical boundary. The public docs offer no opt-out
setting that disables external links for one managed subtree. This is evidence
of the gap, not a claim that no future compliant design can exist.

Reference: [Obsidian Help — Symbolic links and junctions](https://obsidian.md/help/symlinks).

### Daemon authorization source review (2026-10-16)

At the current source, the generic `health.ping` authorizer accepts only
`status.read` and a claimed role of `desktop`, `cli`, or `mcp`; it does not bind
that role to an OS-observed peer process (`services/cryptexd/src/ipc.rs`,
`authorize`, lines 2660-2680). This is limited to health status. Sensitive
operations such as content search/timeline, capture, enrollment, and vault
rebuild require the kernel-observed peer PID to match the authenticated Desktop
process and require the `desktop` role (connection routing around lines
1110-1125; operation authorizers around lines 2040+, 2110+, 2700+, and 2950+).
The current request router exposes no Obsidian-specific role, proposal
operation, or authorization path. The actual plugin's successful health probe
therefore does not provide a write-back IPC route. This read-only source review
does not authorize adding one; P1 remains open pending a design that preserves
the canonical-writer and least-privilege boundaries.

## Findings at plan creation

- No M7 plugin source/package or prior Obsidian write-back spike evidence was
  found in the current repository tree.
- The selected Obsidian minimum version/API baseline remains intentionally
  open until this spike, as required by `docs/31_TECH_STACK_BASELINE.md`.
- Official Obsidian guidance documents Vault event lifecycle and recommends
  `Vault.process()` for read-modify-write safety. This is reference material,
  not proof that Cryptex loop suppression or daemon IPC works.
- No product code, plugin package, dependency, daemon capability, or test
  fixture has been added by this preparation.

## References

- [Obsidian Vault API](https://docs.obsidian.md/Plugins/Vault)
- [Obsidian Events API](https://docs.obsidian.md/Plugins/Events)
- [Obsidian plugin lifecycle guidance](https://docs.obsidian.md/plugins/guides/lifecycle-management)
- `docs/23_REQUIRED_TECHNICAL_SPIKES.md` — P1 Obsidian write-back
- `docs/13_OBSIDIAN_INTEGRATION.md` §§6–10, 16
- `docs/milestones/M7_OBSIDIAN_CONTROLLED_WRITEBACK.md`
