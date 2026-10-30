# Cursor task: M8 macOS XPC peer-identity feasibility

Run this task on the Mac only. This is a bounded synthetic feasibility probe,
not a production change or an M8 completion run.

## Before starting

1. Read `AGENTS.md`, `docs/34_CURRENT_IMPLEMENTATION_HANDOFF.md`,
   `docs/adr/ADR-0007-local-ipc-implementation-addendum.md`,
   `docs/adr/ADR-0019-m8-protocol-addendum-proposal.md`, and
   `evidence/m8/m8-macos-xpc-peer-identity-feasibility-20261025.md`.
2. Report the exact macOS version/build, Xcode/Command Line Tools SDK version,
   architecture, and checkout branch/HEAD/dirty status. Preserve any dirty
   checkout exactly; do not reset, stash, clean, switch branches, commit, push,
   or run GitHub Actions.
3. Keep all probe sources, binaries, and runtime artifacts in a uniquely named
   temporary directory outside the repository and outside all personal data.
   Do not use any real Cryptex memory, vault, transport, trust state, or
   canonical data. Do not edit production IPC or amend ADR-0007.
4. If the required compiler, SDK, or signing identity is unavailable without
   changing security settings or exporting/importing credentials, stop and
   report that limitation. Never weaken a test to manufacture a pass.

## Probe objective

Determine whether documented XPC peer-code-signing checks plus connection
custody can authenticate the exact expected Cryptex Desktop process instance
for daemon authorization. Signer identity alone or `xpc_connection_get_pid`
alone is not a pass. Do not inspect private or opaque audit-token fields.

Build a minimal public-API XPC client/server pair with a benign nonce-echo
operation. Set the expected peer code-signing requirement before resuming or
accepting connections, and use per-message sender validation where the API
requires it. Derive a `SecCode` object from the actual received XPC message and
check it against the exact expected designated requirement. Record the
requirements and signing identifiers used, but never log credentials or
private-key material.

Exercise and report each case separately:

1. Expected signed client succeeds and the server returns only the nonce.
2. Unsigned client is rejected before the handler performs the operation.
3. Different signer is rejected before the operation.
4. Same team but wrong signing identifier is rejected before the operation.
5. Forward an endpoint/connection to a second process signed with the exact
   same requirement. Determine whether it is accepted. If accepted, document
   that signer validation does not bind one process instance and determine
   whether connection custody can exclude forwarding in the actual
   parent-launched-daemon shape. If not, report the exact-instance design as
   infeasible under the tested public APIs; do not replace it with PID-only or
   signer-only authorization.
6. Terminate/restart the expected client while retaining an old endpoint or
   connection. Verify stale state is rejected and a fresh connection performs
   a fresh peer check. Do not infer PID-reuse resistance from equal or unequal
   PID values.
7. Compare connection-level peer requirement results with
   `SecCodeCreateWithXPCMessage` plus `SecCodeCheckValidity` for all applicable
   messages.

Use only APIs available on the product's actual minimum macOS target. If the
probe host is newer, distinguish API availability from minimum-OS proof.
Capture exact command lines, exit codes, and bounded logs. A test counts as
PASS only when its intended positive or negative behavior is observed.

## Deliverable

Write one concise report under the temporary proof directory with source and
binary SHA-256 hashes, API availability, test matrix, exact observations, and
limits. Copy only that report and minimal source needed to reproduce the probe
into `evidence/m8/` after checking the target paths and confirming the source
checkout remains otherwise untouched. Do not commit or push. Update the
handoff only with the scoped result and mark native XPC **NOT RUN** if any
required case could not execute. The outcome must be one of:

- **FEASIBLE WITH EVIDENCE** — all required negative cases reject and the
  exact-process-instance requirement is satisfied by a documented, tested
  mechanism;
- **INFEASIBLE FOR EXACT INSTANCE BINDING** — same-signer forwarding succeeds
  or no public API/custody mechanism establishes the required binding; or
- **INCOMPLETE** — a required case/API/identity could not be tested.

Even a FEASIBLE result is not approval to migrate production IPC, accept the
ADR-0019 proposal, enable trust/transport, or claim M8 complete. Return the
report and the exact one next owner decision, if any.
