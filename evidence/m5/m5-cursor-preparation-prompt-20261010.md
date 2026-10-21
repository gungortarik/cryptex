# M5 preparation handoff for Cursor

M4 PR #10 merged at `a6c32bd30f883e546e2eb976787a59057fae211b`.
Use this in the Windows Cursor checkout after the M5 preparation branch is pushed. This instruction authorizes preparation and
read-only review only, not production implementation.

```text
Continue Cryptex M5 preparation on codex/m5-capture-timeline.

First inspect git status. If the working tree is clean, fetch origin, switch
to codex/m5-capture-timeline, and pull --ff-only from that exact remote branch.
Print the branch, full HEAD SHA and working-tree status. Verify that the M4
PR #10 merge commit recorded in the current handoff is an ancestor of HEAD.

If the checkout is dirty, preserve every change. Do not reset, clean, stash,
delete or overwrite it. Continue the read-only document/code assessment and
report the exact affected files; do not run synchronization or write operations
through user changes. Do not treat a dirty tree as a reason to skip all useful
read-only analysis.

Read AGENTS.md, docs/34_CURRENT_IMPLEMENTATION_HANDOFF.md, the Master Spec,
docs/milestones/M5_CAPTURE_TIMELINE.md, the Definition of Done, accepted
ADR-0013/0015/0016/0017/0018, and
evidence/m5/m5-kickoff-plan-20261010.md. Read the actual M2 writer, M3 projection,
M4 retrieval-safety/local identity, and Desktop IPC implementation before
proposing changes.

Review the kickoff plan against the accepted M5 contract. Return a small,
concrete Slice 1 implementation proposal with actual file/API boundaries,
focused tests, failure semantics and remaining decisions. In particular,
resolve durable acknowledgement versus uncertain commit outcome and safe
lost-response retry before any user-facing Save flow. Do not deduplicate
intentional identical captures by text.

Use existing strict v1 memory.note for durable raw text. Do not add fields or
event types to v1, enable production v2 writing, silently create/replace Brain
or signing identity, infer peer trust, or convert arbitrary note text into
confirmed structured knowledge. Capture must work offline without an LLM.

Desktop must use explicit authenticated bounded daemon operations. It must
not write canonical files, access SQLite directly, receive private keys or
gain arbitrary shell/filesystem permissions. Timeline/detail/source retrieval
must honor the same RetrievalSafetyState as search. Review existing provenance
and classification policy without inventing new canonical metadata/defaults.

Keep runtime data, fixtures and Cargo targets outside the repository, home,
Desktop and Documents defaults. Use synthetic Golden Brain data only. The
Windows LocalAppData Cargo target used previously is available if needed
during a later authorized implementation slice; do not disable Smart App
Control or alter OS security policy.

Do not implement M6/M7/M8/M9/M10/M11/M12/M13/M14 features opportunistically.
Do not run GitHub Actions for this planning review. Production implementation
must wait for explicit authorization. Do not create an ADR for routine API
details inside the accepted contract; report any genuine architectural conflict
with its exact authoritative references before changing direction.

Report in English: full reviewed SHA, accepted versus proposed decisions,
Slice 1 files/APIs/tests, genuine blockers, and the next concrete action.
Do not claim M5 implemented or complete.
```
