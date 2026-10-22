# M6 entity/relation Obsidian Graph check — macOS synthetic scope

**Date:** 2026-10-14  
**Host:** macOS 27.0.1 (26A434), arm64  
**Commit:** `74bc3b14127962579ae03838bafea9336612a033`  
**Verdict:** PASS for projection-library output and Obsidian Graph consumption only.

## Fixture and boundary

The one-off harness ran outside the repository and called the public
`rebuild_vault_with_knowledge` API with a synthetic `VaultKnowledgeView`:

- Entity A: `aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa` (`Alpha Entity`)
- Entity B: `bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb` (`Beta Entity`)
- One directed `depends_on` relation from A to B
- No canonical events, daemon seed, production v2 write enablement, or repository edits

The temporary vault and harness were removed after verification.

## Results

| Check | Result |
|---|---|
| Projection rebuild | PASS; two notes only under `Cryptex/Entities/` |
| A's directed WikiLink to B | PASS |
| Reverse link in B | PASS; absent |
| Obsidian opened generated A note | PASS |
| Obsidian Graph view opened for the unique vault | PASS |
| Sentinel outside `Cryptex/` | PASS; hash `9b7fa25f…875ad294` unchanged |
| Temporary harness/vault cleanup | PASS |

The generated directed WikiLink and successful Graph view are the connectivity
evidence. Graph node/edge pixels were not independently enumerated through
Accessibility APIs.

## Scope limit

This does not verify the daemon's canonical-history scan, shared retrieval-safety
gate, or packaged Desktop-to-daemon entity/relation rebuild end to end. Those
remain covered by focused automated checks, not this native check. It is not an
M6-complete claim. No Actions, commit, push, or merge was performed.
