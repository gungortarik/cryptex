# M7 P1 — public Vault API containment review

**Date:** 2026-10-16  
**Scope:** read-only review of public Obsidian API declarations and help  
**Decision:** managed-note reads/writes remain disabled under the owner-selected
Master Plan. No compliant physical-containment design is established by this
review.

## Sources reviewed

- [Obsidian public TypeScript API declarations](https://github.com/obsidianmd/obsidian-api/blob/master/obsidian.d.ts)
- [Obsidian Help: Symbolic links and junctions](https://obsidian.md/help/symlinks)
- [Obsidian Help: Vault data storage](https://obsidian.md/help/data-storage)

The public declarations define path-based Vault and adapter methods, including
`Vault.create(path, data)`, `Vault.process(file, fn)`, and adapter operations
that take normalized paths. `Vault.process` documents atomic read/modify/write
for avoiding concurrent content changes. The reviewed declarations do not
document an operation option or API guarantee that pins a managed operation to
a physical root using no-follow traversal or directory/file handles. This is a
bounded review of the public declarations, not proof that Obsidian can never
provide such a capability.

Obsidian Help explicitly states that symlinks and junctions can expose files
outside a vault and warns of data loss/corruption risks. That behavior matches
the previously recorded synthetic Windows junction and owner-reported Mac
symlink escape observations. A vault-relative path therefore does not establish
the required physical boundary. A metadata preflight such as `lstat` also leaves
a race between inspection and the subsequent Vault API operation.

## Constraint check

The accepted M7 contract requires all of the following before write-back:

- Obsidian Vault API compliant managed-note operations;
- physical containment on Windows and macOS, including reparse points and
  symlinks;
- no direct filesystem fallback;
- no managed-note writes before the gate passes.

Under these constraints, the reviewed public API does not currently provide a
documented mechanism to prove the required physical boundary. No product code,
IPC capability, or architecture change was made. This result is **UNRESOLVED**,
not a claim that a future supported Obsidian API or approved design cannot
solve it.

## Next gate

Keep managed-note IO disabled. P1 can proceed only when a supported Vault API
contract or another owner-approved design provides a testable physical-boundary
guarantee. Once such a mechanism exists, validate link escapes and races on
both operating systems in synthetic vaults before enabling any write path.
Health IPC, authenticated proposal authorization, and real-daemon in-flight
cleanup remain separate unrun gates.
