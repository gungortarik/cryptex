# Authorized branch/workspace cleanup

Date: 2026-10-22. Owner requested review/publication of required work, removal
of unnecessary files/merged branches, and checking an accidental main checkout.

## Merged branches

Deleted local and remote branch refs only after exact-tip, ancestry and open-PR
checks. Each tip remains reachable from both the default `m0/foundation` and
the M8 checkpoint:

| Deleted branch | Preserved commit |
|---|---|
| m0/p0-1-cursor | 6d4d10ad0f14887f37a3b418c9b1894b991ea902 |
| m0/p0-2-cursor | 2f703f08068f449368d7d0d0cb1a599a1c09d83c |
| m1/brain-device-identity | 9960a6218a2e32209f3fb1fe2de07a89e90ad318 |
| m2/immutable-event-store | de7d3267d30ead25d0a91c49b2ae007d564046f2 |
| m3/local-projection-search | eb5d2f50ef31d0ee85eadd73615469aa1bc1c3be |
| m4/knowledge-engine | c08181db7fd9a9d9c26e73519ad9f5fab5bda438 |

Remote deletion used atomic exact-tip leases; no commit history was rewritten.
Kept main/default foundation, active M8 and M5 associated with the Desktop
stash at review time. Open PR #2 (foundation -> main) was not changed.

## Accidental checkout / stash

Reflog showed GitHub Desktop switched the original checkout to main at
2026-10-22 00:57:37 America/New_York and preserved prior work in stash
`1c67881df188a8e36a35e4f1f799a032c275db5f`. Compared all 145 changed paths
with the pushed M8 checkpoint: 31 identical after text normalization, one
handoff fully covered by its newer content, 113 obsolete patch/inspection/
snapshot/cache files. No unresolved unique product/evidence path remained.

Before dropping that exact stash, created and verified a complete Git bundle
containing both the stash and M8 checkpoint. Backup:
`C:/Users/Tarik/AppData/Local/Cryptex-Dev/git-cleanup/workspace-20261022-40fef7d25e2443daadb7b1da3a4bfa9b/`.
It also contains a hash-verified copy of the old untracked October-13 generated
Desktop daemon executable, which was then removed by exact literal path.
No recursive clean/reset or broad filesystem deletion was used.

Released the M8 branch from its clean auxiliary worktree using a detached
checkout, then switched the original checkout to that same M8 commit. The
primary working path is now `D:/github_repos/cryptex` on `codex/m8-device-trust`.
The auxiliary `C:/Users/Tarik/.codex/worktrees/m8-device-trust/cryptex` remains
clean and detached at `9e8d684`; it is not a second active M8 branch owner.

No product source changed. Runtime tests were not rerun for Git/file metadata
cleanup; prior verification limitations and M8 security/Mac gates remain open.
No Actions, PR creation, merge, default-branch change or production deployment.
