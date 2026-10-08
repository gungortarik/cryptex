# M3 final dual-OS CI gate — PASS

**Workflow:** [37720627242](https://github.com/gungortarik/cryptex/actions/runs/37720627242)  
**Branch:** `m3/local-projection-search`  
**Tested SHA:** `51e6dc772757510e08c14ef565a87534a8a9478b`  
**Result:** completed successfully

| Job | Result |
|---|---|
| Readiness (`windows-latest`) | PASS |
| Readiness (`macos-latest`) | PASS |
| IPC cross-account ACL (Windows) | PASS |
| Windows Desktop package (unsigned, manual only) | SKIPPED by design |

The readiness jobs include the M3 projection crate tests and the existing full
`cryptexd` test suite. The Mac local four-check evidence is recorded separately
at `evidence/m3/m3-slice7-macos-four-check-20261008.md` for source
`428b03f33eda8fbf645678220dbd3ce3d88d108e`; this workflow tested the later
documentation-only branch tip above.

This closes the Slice 7 cross-platform verification gate. It does not by itself
complete M3. Slice 8 Definition of Done, security review, and owner review remain
required. No additional Actions run is needed for this gate.

## Slice 8 final candidate — PASS

**Workflow:** [37725223487](https://github.com/gungortarik/cryptex/actions/runs/37725223487)
**Branch:** `m3/local-projection-search`
**Tested SHA:** `275fb7acf0c2c7b4d7c050acb54670e81520377f`
**Result:** completed successfully

| Job | Result |
|---|---|
| Readiness (`windows-latest`) | PASS |
| Readiness (`macos-latest`) | PASS |
| IPC cross-account ACL (Windows) | PASS |
| Windows Desktop package (unsigned, manual only) | SKIPPED by design |

The readiness jobs include workspace format/lint/check, M2/M3 tests and the
existing foundation checks. Owner macOS four-check PASS at the same exact SHA is
recorded in `evidence/m3/m3-slice8-macos-four-check-20261008.md`. Windows-local
verification for the candidate is recorded in the Slice 8 security/DoD evidence
and the branch change record.

This closes the candidate's cross-platform local and required CI verification
gates. M3 is still not complete: the final security/DoD disposition and human
owner review must be recorded before any completion or M4 claim. This run does
not authorize a merge by itself.
