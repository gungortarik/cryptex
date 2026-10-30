# M8 protocol addendum independent review

**Review source:** Cursor read-only design/security review at `ae61603b517f68b63d559ccb5ee77d744176e6a6`
**Review scope:** `ADR-0019-m8-protocol-addendum-proposal.md` §§2.4, 4, 5.1.1, 6–7; Master Spec; accepted ADR-0019 product direction; restriction-authority addendum; M8 milestone.
**Recommendation:** **NOT READY**

The reviewer found the direction consistent with the Master Spec and accepted
ADR-0019 direction: Syncthing remains transport, there is no master device, and
the daemon remains the canonical writer. The blockers concern protocol
completeness and the proposal's own acceptance gates.

## Findings

| ID | Finding | Required follow-up |
|---|---|---|
| B1 — Critical | §2.4 R1 and §6 require concrete Windows/macOS connection-bound peer credentials and process replacement/reuse rejection; these are not pinned. Windows current IPC uses named-pipe peer PID plus a retained process handle. macOS currently uses `LOCAL_PEERPID` and PID comparison, without a process-generation binding. | Specify each OS API, the exact identity compared for every connection, and fail-closed replacement/reuse behavior; then verify it natively. PID-only is insufficient. |
| B2 — Critical | Independent review is still an explicit precondition for R1–R5 and the §4 cycle/Brain-wide-hold vectors. Prior documentation follow-ups do not satisfy it. | Keep the addendum PROPOSED until a scoped independent re-review returns READY. |
| B3 — Critical | §5.1.1 does not fully define committed-marker fields, move/flush order, idempotent replay, or the crash decision for partial visibility. | Separate the implemented pre-publication abort-only slice from the target algorithm; define a deterministic commit/abort/typed-hold matrix before acceptance. |
| B4 — High | Implementation status used stale durability and test claims. | Updated the status text in the proposal: scoped staging flush is distinguished from pending-journal and publication durability; current Windows counts are recorded. |
| B5 — High | The separate R3 pending-journal Windows parent-sync guarantee remains unclaimed while its fault-injection tests are still an acceptance gate. | State the residual crash behavior and require matching fault-injection evidence; do not infer this guarantee from the staging helper. |
| B6 — Medium | Candidate-key verification primitives could be mistaken for complete trust/cutoff validation. | Clarify that publication must bind verification to the reducer and `RetrievalSafetyState` under the required locks. |

## Windows verification update

After the earlier Application Control 4551 denial, local Windows execution
subsequently passed:

- Two pre-publication process-interruption tests: **2/2**.
- `import_staging::tests`: **19/19**.
- `cryptexd --lib`: **417 passed, 0 failed, 4 ignored**.

These results cover the named process-crash recovery states on this Windows
host only. They do not establish power-loss durability, canonical publication,
partial-publication replay, macOS parity, trust activation, or M8 completion.
No Actions ran. No code, commit, or push was performed for this review.

## Cursor follow-up review of the revised working-tree draft

The owner authorized a read-only second Cursor review against working-tree
`docs/adr/ADR-0019-m8-protocol-addendum-proposal.md` at the same Git HEAD
`ae61603b517f68b63d559ccb5ee77d744176e6a6`. It returned **NOT READY**.
The review examined §§2.4 and 5.1.1 plus current `services/cryptexd/src/ipc.rs`.

| ID | Finding | Required proposal correction |
|---|---|---|
| C1 — Critical | The inherited macOS pipe carries parent-supplied bytes; the draft did not require kernel attestation that the token represents the live, expected Desktop parent. | Treat the pipe only as a carrier. Require exact-size/fail-closed read, parent-liveness and identity binding, packaged-image validation through a kernel-backed mechanism, close the pipe, and protect inheritance. Token bytes are not authentication. |
| C2 — Critical | Full byte equality of an audit-token snapshot is not established as stable and its process-generation semantics were not specified. | Match PID plus a kernel process-generation identity or equivalent live-token validation; do not claim unspecified full-token equality. Verify exact APIs and fields against primary sources and native tests. |
| H1 — High | Windows peer PID + retained handle describes OS peer binding, not the complete R1 approval protocol. | Keep approval digest binding, one-shot consume, cancellation serialization, and consume-before-append as separate open requirements. |
| H2 — High | Pipe inheritance, exact read, and Desktop exit/restart race behavior were unspecified. | Require exclusive intended-child inheritance, fixed-size read, live parent validation before pin, no token logging, fail closed, and fresh handoff after daemon restart. |
| H3 — High | `RetrievalSafetyState` binding was placed under staging, leaving publication/recovery timing ambiguous. | Bind reducer/RSS to verified tips under canonical locks immediately before publication and again before recovery finish moves. |
| H4 — High | A subset-finish branch could publish against stale tips/policy after a crash. | Re-run the full step-5 locked validation immediately before every recovery finish move; hold if any current state differs. |
| H5 — High | Marker-missing/partial states and Windows marker-parent durability were not fully covered. | Enumerate marker absent/truncated/temp/present with incomplete destinations; do not claim Windows committed status until marker-parent durability and native interruption coverage pass. |
| M1 — Medium | Duplicate normative reader-gate sentence. | Removed duplicate sentence from the proposal. |

The proposal now records these constraints as still-proposed requirements.

### Primary-source follow-up

A separate read-only check of Apple-published XNU source confirmed:

- `bsd/sys/un.h` defines `LOCAL_PEERTOKEN` as retrieving the peer audit token:
  <https://github.com/apple-oss-distributions/xnu/blob/main/bsd/sys/un.h>.
- `osfmk/mach/task_info.h` defines `TASK_AUDIT_TOKEN` and its count:
  <https://github.com/apple-oss-distributions/xnu/blob/main/osfmk/mach/task_info.h>.
- `osfmk/mach/message.h` calls `audit_token_t` opaque and warns that its
  representation may change over time:
  <https://github.com/apple-oss-distributions/xnu/blob/main/osfmk/mach/message.h>.
- Current XNU's internal `proc_find_audit_token` obtains the PID, checks the
  token's embedded PID-version against the live process, and rejects a
  mismatch:
  <https://github.com/apple-oss-distributions/xnu/blob/main/bsd/kern/kern_proc.c>.

This supports the design direction of using kernel-provided peer tokens and
generation-aware validation, while disproving any guarantee that arbitrary
full-token byte equality is a stable public contract. The kernel implementation
is internal source: this check did not establish a supported user-space API for
comparing the token's process generation, a public token-to-code-signature
check, or native behavior on the supported macOS release. Those remain open;
do not inspect opaque `audit_token_t` slots directly or claim a Mac security
PASS.

No source implementation, protocol acceptance, trust activation, Actions,
commit, or push occurred.

## Next gate

Resolve the macOS parent-token attestation and process-generation APIs against
primary platform sources, then obtain a fresh independent review of the revised
proposal—including R1–R5, §4 cycle-hold vectors, and the publication/recovery
matrix. Native platform tests remain required. Keep the proposal PROPOSED and
all trust/transport activation disabled until those gates pass.

## Cursor follow-up review of revised peer pins and recovery matrix

The owner authorized a further read-only Cursor review of the revised working
tree at Git HEAD `ae61603b517f68b63d559ccb5ee77d744176e6a6`. It returned
**NOT READY**. The prior Windows/RSS/subset-revalidation changes improved the
proposal but did not close the remaining evidence gates.

| ID | Finding | Required next step |
|---|---|---|
| C1 — Critical | The macOS process-generation/token-bound API composition remains unidentified. Accepting this leaves implementers choosing private token-slot reads, PID-only matching, or unverified byte comparison. | Name and SDK-bound a supported public composition, or conclude none exists for the accepted Unix-socket design and seek an owner-approved IPC redesign. |
| H1 — High | One-shot pipe wording does not fully define ownership of each inherited end or exclude child/grandchild inheritance. | Specify parent-only writer, child closes writer before fixed-size read, prevent inheritance of all unintended ends, and fail closed if the parent exits before binding. |
| H2 — High | A recognized committed-marker temporary has no closed filename/schema definition. | Define exact temporary/final names, bounds, and no-replace completion rule; unknown names hold. |
| H3 — High | Windows committed-marker parent durability remains unverified; staging/journal flush is not marker-directory proof. | Preserve the non-claim; require a platform strategy and native interruption coverage before claiming committed import. |
| H4 — High | Subset recovery is safe only if every canonical mutator actually participates in the shared lock API. | Keep trust disabled until lock migration and interleaving tests cover all canonical writers. |
| M1 — Medium | Requiring a literal public PID-version accessor may be too narrow; an equivalent documented kernel-bound identity could qualify. | Evaluate a complete supported composition without decoding opaque token slots. |
| M2 — Medium | R1–R5, reducer vectors, bootstrap parser, publication, and native test gates remain open. | Keep status PROPOSED / NOT READY. |

### Apple API source check after the Cursor review

Apple's published Security documentation exposes `kSecGuestAttributeAudit`,
`SecCodeCopyGuestWithAttributes`, and `SecTaskCreateWithAuditToken`. Those docs
describe identifying/representing code or a task from an audit token, but do not
document a public process-generation accessor or promise that a Unix-socket
peer token can be compared to a launch token as a stable byte string.

Apple DTS recommends XPC peer code-signing requirements for validating a
connection's signed executable; the C-XPC API is documented from macOS 12. The
same Apple docs warn `xpc_connection_get_pid` can become stale and another
process can reuse the PID before a message is received. XPC is therefore a
feasibility path, not proof of the exact parent-process occurrence required by
this proposal.

This route also conflicts with accepted ADR-0007's choice of `interprocess`
local sockets across macOS and Windows. Switching macOS to XPC requires an
explicit architecture amendment; none has been made. No SDK-header audit on
Mac or native XPC/UDS adversarial test has run. Keep M8-R1 open and trust and
transport disabled. Sources:

- Apple, [SecCodeCopyGuestWithAttributes](https://developer.apple.com/documentation/security/seccodecopyguestwithattributes%28_%3A_%3A_%3A_%3A%29)
- Apple, [Guest Attribute Dictionary Keys](https://developer.apple.com/documentation/security/guest-attribute-dictionary-keys)
- Apple, [SecTaskCreateWithAuditToken](https://developer.apple.com/documentation/security/sectaskcreatewithaudittoken%28_%3A_%3A%29)
- Apple DTS, [Validating Signature Of XPC Process](https://developer.apple.com/forums/thread/681053)
- Apple, [XPC connection peer PID caveat](https://developer.apple.com/documentation/xpc/xpc_connection_get_pid%28_%3A%29)

**Owner decision (2026-10-25):** run a bounded synthetic XPC feasibility spike.
This is not approval to change production IPC or amend ADR-0007. The spike
design and native-run boundary are recorded in
`evidence/m8/m8-macos-xpc-peer-identity-feasibility-20261025.md`.

## Cursor follow-up review — marker matrix, placement tests, lock audit

An owner-authorized read-only follow-up review of the current working diff at
HEAD `ae61603` returned **NOT READY**. The reviewer found the marker names and
closed schema explicit, the no-replace helper tests appropriate for their
scope, and the durability statements careful. The review did not rerun tests.

Remaining findings:

- Windows parent-directory durability remains open. Successful scoped staging
  directory flush on one NTFS host does not prove committed-marker durability
  or power-loss behavior.
- There is still no production marker parser/writer, canonical publisher, or
  recovery replay. The placement tests exercise helpers only.
- Existing production append/seal/open-recovery paths use the shared
  inventory-to-device lock convention, but no importer publisher exists to
  prove participation. Storage mutation APIs remain publicly callable.
- The proposal did not explicitly say that a valid final marker's matching
  prepared manifest and staging tree are required companions before cleanup.
  ADR §5.1.1 now states this and fails closed when those companions are missing
  or unverifiable.

The reviewer confirmed that the reported 21/21 focused Windows result was not
rerun in the review. The full daemon suite result of 417 passed / 4 ignored
belongs to the earlier checkpoint before the two marker placement tests.

The next implementation gate is a production marker parser/write/recovery
path under the shared lock set, with Windows process-interruption tests at
marker boundaries. Do not claim Windows power-loss durability or enable trust.

**Documentation follow-up:** Updated proposed pipe ownership/inheritance rules
and assigned the marker's exact final/temp names, closed fields, and 4-KiB size
bound in ADR-0019 §5.1.1. Added a normative recovery decision table that
distinguishes valid final marker, recognized temporary, no marker, empty,
partial, and complete destination states. All paths that create/complete a
marker require current locked trust/cutoff/`RetrievalSafetyState` validation
and destination rechecks. These are target requirements only; no
implementation, durability proof, or crash test exists for the marker. The
macOS API/ADR-0007 owner decision remains open. `git diff --check` passes.
