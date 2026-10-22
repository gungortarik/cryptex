# M8 independent protocol security review — follow-up disposition

**Review supplied by:** owner, pasted read-only security review  
**Candidate reviewed:** `docs/adr/ADR-0019-m8-protocol-addendum-proposal.md` at `7fc6089`, unchanged through `5d5076e`  
**Disposition:** **NOT READY**  
**This follow-up:** documentation-only correction to two reducer ambiguities; no protocol acceptance or implementation claim.

## Findings and disposition

### Critical — preserve eligible global cutoffs across SCC filtering

The reviewer demonstrated that an eligible restriction from an already-resolved external issuer can constrain a device even after an SCC-local edge removal. If step 3 recomputes the device cutoff only from the remaining SCC edges, an issuer action that is globally ineligible can incorrectly tighten another target.

The proposal now defines every effective cutoff over the whole verified candidate graph: `u64::MAX`, the valid self-retirement cap, and every incoming restriction whose issuer cutoff is fully resolved and whose event sequence is within that issuer cutoff. It explicitly says eligible incoming restrictions remain in scope after SCC-local edge removal, and unresolved issuer authority cannot be treated as absent. A golden vector checks `X→A(cutoff 2)`, `A@10→B`, `B→A`, and `A@3→E(cutoff 0)`: `A` resolves to cutoff 2, `A@10→B` is excluded, and `A@3→E` does not apply.

### High — mixed split after SCC filtering

The reviewer noted that filtering may leave both an acyclic piece and a cyclic piece. The proposal now requires recomputing SCCs and the condensation DAG; if any cycle remains anywhere in that recomputed subgraph, the Brain enters the safe-conflict hold. The acyclic piece must not be resolved separately. A golden vector covers this split.

## Remaining NOT READY gates

- **Importer publication barrier:** no reviewed daemon-owned staging/import path atomically publishes canonical event files under the required inventory/device locks. Direct Syncthing mutation bypasses process locks. Keep trust activation blocked until the publish path prevents partial or mixed snapshots and interleaving/failure tests pass.
- **OS peer credentials:** the approval IPC still needs concrete, connection-bound Windows and macOS peer-credential mechanisms and process replacement/reuse tests.
- **Retrieval safety:** v3 `security.*` records do not yet update `RetrievalSafetyState`; the proposal correctly prohibits writes until the integration exists.
- **Bootstrap parser:** bounded pre-allocation parsing remains a hard pre-implementation gate; current event parsers do not satisfy it.
- **Independent review:** these edits respond to the supplied review; they are not an independent re-review. The protocol remains PROPOSED / NOT ACCEPTED.

The reviewer also confirmed that cycle holds align with Master Spec §9 and accepted ADR-0019, with the documented availability/DoS consequence still requiring owner acceptance as part of protocol acceptance.

## Scope

No code, tests, trust state, Actions, or runtime data changed. No M8 completion or protocol acceptance is claimed.
