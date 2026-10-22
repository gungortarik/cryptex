# M7 P1 process-sandbox feasibility review

**Date:** 2026-10-22\
**Scope:** Read-only review of whether an OS process sandbox presently provides
a supported, cross-platform boundary for the stock Obsidian application while
retaining the accepted plugin/Vault API workflow.\
**Disposition:** No such Cryptex-controlled cross-platform design is established.
No sandbox, packaging, or architecture change was made.

## Constraints

M7 requires Vault-API-compliant note operations physically contained beneath
the managed root on Windows and macOS, including link/reparse and replacement
cases. The plugin runs inside Obsidian's Node-enabled application process; the
public guide says plugins can access the filesystem like Node.js applications
and documents `Vault.process()` for content concurrency, not a process or path
containment boundary.

## Platform options reviewed

| Option | Evidence | M7 disposition |
|---|---|---|
| Windows AppContainer sandbox launch | Microsoft documents `Experimental_CreateProcessInSandbox` with AppContainer and Bound File System grants, but explicitly labels the APIs experimental and subject to change. | Not a supported, cross-platform M7 design as-is. Obsidian Electron GUI compatibility, exact reparse behavior, IPC, updates, and lifecycle would require a separately approved native prototype. |
| macOS App Sandbox on stock Obsidian | Apple documents that the App Sandbox entitlement is part of the app's code signature. Cryptex cannot add that entitlement to the vendor-signed Obsidian executable without a vendor-supplied sandboxed build or a re-signed distribution. | Requires upstream/vendor packaging support; not a plugin-level containment control. |
| Different Obsidian build or Cryptex-controlled wrapper | Could potentially apply platform-specific process isolation while retaining Obsidian UI, but changes the launch/distribution integration and needs exact-path, symlink/reparse, replacement-race, IPC, update, and usability validation on both platforms. | An architecture/product integration change; not selected or authorized. |
| FUSE/WinFsp mount, VM, or separate OS identity | These add a filesystem driver, virtualized app, or separate-user workflow and deployment/permission requirements. | Not selected by the accepted plan; requires an owner-reviewed design change and broad DoD review. |

## Conclusion

OS process isolation may be a future design avenue, but current evidence does
not establish a supported, vendor-compatible mechanism that Cryptex can apply
to the stock Obsidian app on both platforms. Choosing one of these alternatives
would change the integration/launch design and cannot be done silently under
the owner-selected plan. Continue to keep managed-note I/O disabled.

The supported path that preserves the current architecture is a documented
Obsidian API guarantee, followed by synthetic Windows/macOS tests against the
existing race/reparse acceptance matrix. The API request remains recorded in
`m7-p1-obsidian-api-containment-request-20261016.md`; its last recorded state
was moderator approval pending. A public search did not surface a response,
which is not proof of account inbox state.

## Sources

- [Obsidian Vault developer guide](https://docs.obsidian.md/Plugins/Vault)
- [Microsoft Create Process in Sandbox](https://learn.microsoft.com/en-us/windows/win32/secauthz/createprocessinsandbox)
- [Apple App Sandbox entitlement](https://developer.apple.com/documentation/bundleresources/entitlements/com.apple.security.app-sandbox)
- [Apple code-signing and entitlements](https://developer.apple.com/library/archive/documentation/Security/Conceptual/CodeSigningGuide/AboutCS/AboutCS.html)
- [Obsidian symbolic links and junctions help](https://obsidian.md/help/symlinks)
