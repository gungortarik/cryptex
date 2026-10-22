# M7 P1 current Obsidian API documentation check

**Date:** 2026-10-22\
**Scope:** Read-only review of current public Obsidian developer documentation
and existing synthetic Windows/macOS link-escape evidence.\
**Disposition:** Physical containment remains **UNRESOLVED**; managed-note I/O
stays disabled.

## Current public contract

The current Vault guide describes `Vault.process()` as a way to avoid
overwriting a concurrent content change between reading and updating a note.
It does not document a no-follow option, root capability, or handle-bound
physical containment guarantee. The guide also says plugins can access the
filesystem like any Node.js application, so the API surface is not itself a
process sandbox.

Obsidian's help page explains that symbolic links and Windows junctions can
expose files outside a vault. Existing Cryptex synthetic observations are
consistent with that behavior: Windows `Vault.create()` followed a junction
below the managed prefix, and the owner-reported macOS `Vault.create()` followed
a symlink. Preflight metadata checks remain vulnerable to replacement between
check and operation.

This bounded public-documentation review does **not** prove that Obsidian can
never add a supported containment guarantee. It confirms that the reviewed
current public contract does not provide the guarantee needed by the accepted
M7 gate. An OS-level sandbox or alternate filesystem boundary could be studied
only as a separately reviewed design; this report does not select or authorize
one.

## Relation to accepted requirements

The accepted M7 contract still requires Vault-API-compliant operations,
physical containment on Windows and macOS, no direct-filesystem fallback, and
disabled managed-note writes until containment is proven. Neither
`Vault.process()` concurrency safety nor M6's owned-subtree validation closes a
race between filesystem inspection and a later Obsidian path operation.

## Sources

- [Obsidian Vault developer guide](https://docs.obsidian.md/Plugins/Vault)
- [Obsidian Vault TypeScript API reference](https://docs.obsidian.md/Reference/TypeScript+API/App/vault)
- [Obsidian symbolic links and junctions help](https://obsidian.md/help/symlinks)
- Cryptex's existing native/synthetic evidence:
  `m7-p1-windows-writeback-spike-partial-20261014.md` and
  `m7-p1-macos-writeback-spike-partial-20261015.md`

## Next condition

Resume the containment spike only after either (a) Obsidian publishes a
supported operation/guarantee that can be tested against the full acceptance
matrix in `m7-p1-obsidian-api-containment-request-20261016.md`, or (b) the owner
accepts a separately reviewed design that preserves the selected Obsidian
workflow and meets the same physical-boundary bar. Until then, do not enable
managed-note reads/writes or claim M7 complete.
