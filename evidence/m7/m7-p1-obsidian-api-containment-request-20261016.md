# Obsidian API request: safe managed-note containment

**State:** submitted to the official `Devs: Plugins & API` forum on 2026-10-16; moderator approval pending. The forum confirmed one post is pending; it is not yet publicly visible and no topic URL is available. On 2026-10-16, the forum system also notified the account that it is temporarily on hold until staff review recent posts; the notice says new topics and replies are unavailable during review. No appeal or follow-up was sent.  
**Purpose:** unblock M7 P1 without weakening the accepted physical-containment
requirement or enabling managed-note I/O early.

The submitted topic title was **“API request: physically contained Vault operations for managed paths”**. The public post summarizes the problem and asks whether a supported Vault API contract or operation can prevent symlink/reparse-point escapes and parent-replacement races on Windows and macOS. It states that Cryptex write-back remains disabled until a documented, testable guarantee exists. The detailed acceptance matrix below is retained as Cryptex's internal verification bar; it was not included in the public post.

## Problem statement

Cryptex M7 needs to observe and propose edits to generated notes through
Obsidian's supported Vault API. A vault-relative path does not ensure the
physical file is within the managed directory: a symlink or Windows junction
can point from the managed subtree outside it. Synthetic tests have observed
Vault operations following such links on both platforms. Checking metadata
before the Vault operation cannot close a race where a link changes between the
check and the actual operation.

Obsidian's current public API declarations expose path-based Vault and adapter
operations; the reviewed declarations do not document a no-follow,
directory-handle-bound containment guarantee. Obsidian Help also documents
symlinks and junctions as a way to store files outside a vault and warns of
data-integrity risks.

References:

- [Obsidian public API declarations](https://github.com/obsidianmd/obsidian-api/blob/master/obsidian.d.ts)
- [Obsidian Help: Symbolic links and junctions](https://obsidian.md/help/symlinks)
- `evidence/m7/m7-p1-vault-api-containment-review-20261016.md`

## Requested clarification or capability

Can Obsidian provide and document a supported Vault API operation for a plugin
to read or update a note only when the operation is physically contained under
a specified managed root, with the containment decision bound to the actual
filesystem operation? The contract must cover Windows reparse points/junctions
and macOS symlinks and state how callers can rely on it across supported app
versions.

If no such API is available, please clarify whether a supported plugin API
design can meet this guarantee. Cryptex will keep managed-note reads/writes
disabled until a supported, testable answer exists.

## Required acceptance evidence before Cryptex enables writes

On both Windows and macOS, using a synthetic vault and no personal notes:

1. Normal managed-root create, read, and `Vault.process` operations succeed.
2. A symlink/junction at every managed path component pointing outside the
   physical root is rejected before any outside target is read or changed.
3. Symlink/junction chains, dangling links, loops, and parent replacement are
   rejected safely.
4. Race tests repeatedly swap or replace a link while the operation runs; no
   operation touches outside bytes.
5. Failure cases leave the outside target and a sentinel unchanged and produce
   a typed, non-sensitive error.
6. The guarantee and test results are documented for the exact supported
   Obsidian API/app version on each OS.

These tests are a Cryptex-side acceptance bar. They do not prescribe Obsidian's
implementation. Metadata preflight plus a later path-based write is not enough.

## Current Cryptex decision

The owner selected the existing Master Plan and disabled managed-note writes.
This draft makes no architecture change and authorizes no implementation. Once
an API contract is identified, resume with a synthetic-only spike that proves
each acceptance condition on both operating systems. If the API cannot provide
the guarantee, return to the owner with the documented conflict before
changing the M7 design.
