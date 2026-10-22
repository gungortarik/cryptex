# M7 P1 macOS Obsidian runtime version inventory

**Date:** 2026-10-18  
**Vault:** Existing synthetic vault only.  
**Checkout:** Clean detached verification worktree at
`74bc3b14127962579ae03838bafea9336612a033`; dirty main checkout untouched.

## Findings

| Source | Version |
|---|---|
| App bundle | `/Applications/Obsidian.app` |
| Bundle plist and `mdls` version/build | `1.13.7` / `1.13.7` |
| Bundled `app.asar` and `obsidian.asar` | `1.13.7` |
| Running auto-updated payload (`lsof` and `~/Library/Application Support/obsidian/obsidian-1.14.4.asar`) | `1.14.4` |
| Window title and plugin runtime `apiVersion` | `1.14.4` |
| Plugin manifest `minAppVersion` | `1.5.0` |
| Obsidian API-types package | Not installed; temporary probe is plain CommonJS |

The earlier values were not a measurement error: the installed shell remains
1.13.7 while its active auto-updated application payload is 1.14.4. This
resolves the inventory discrepancy, but does not select a supported Cryptex
compatibility range or prove API stability across versions.

Owner-reported report outside Git:
`~/Library/Application Support/Cryptex-Dev/m7-p1-obsidian-038a331d135f423da4fe8a7473fea2f8/obsidian-version-discrepancy-resolve-20261018T172007Z.md`

No install/update, plugin change, test, source edit, or worktree mutation was
reported.
