# M8 Windows guest managed-profile verification — owner-reported PASS

Date: 2026-10-21. Source: owner-supplied PowerShell transcript attachment
`d1ec4a42-995d-458f-9952-74c988b34b30/Pasted text.txt`.

## Final invocation and result

The owner ran from:
`C:/Users/Tarik/Downloads/Cryptex-M8-Windows-ManagedProfile-20261020/source`.
The original downloaded ZIP was previously verified against SHA-256
`1058be763a7b198113a15d57c25bc3da05ae86fb388c0b3574174d2cd35c775a`.
The deterministic stderr helper repair matched runner SHA-256
`3cfa2b7000910c613b20f3af0ae21d6fd78e441f435c1175b0e67755a722e213`,
and the corresponding manifest entry was updated. No Rust source patch was
applied in this owner sequence.

The successful final run resolved the native executable with PowerShell
Resolve-Path and supplied its absolute path, preserving the downloaded helper
and all 16 source checks. Earlier relative-path and execution-policy failures
remain historical failures, not failures of this final invocation.

```text
Fresh protected loopback-only native fixture prepared. No transport was launched.
Scoped Windows verification PASS. Report: C:\Users\Tarik\AppData\Local\Cryptex-Dev\m8-verification\profile-batch-e3f006113b5b42b6b9e72a42331c2d6c
This does not complete M8. No Actions, peer pairing or canonical writes were enabled.
```

This closes the previously blocked scoped guest verification receipt. The
reviewed runner checks each Cargo exit status before printing PASS: fmt,
daemon/events package tests, strict daemon Clippy, daemon build, explicit
owned-process native fixture, and explicit private-profile/real Windows
Credential Manager native fixture. Native fixture preparation alone launches
no server; the subsequent explicit tests exercise the owned native lifecycle.

Evidence limit: this is the owner's aggregate terminal PASS, not independently
imported/reviewed `results.json`, test counts or per-run native PID/exit records.
Those remain in the report folder above. Do not invent counts, host build,
architecture or individual native PIDs from the aggregate receipt.

The verified guest source uses the first stderr-repaired runner and the
absolute-path workaround. The later worktree runner/preparer normalize the
path internally and have separate Windows PowerShell 5.1 regressions. Do not
claim the guest ran those later script bytes; product Rust is unchanged.

## Continuation and limits

Continue the Windows-first batch with native config/package provisioning,
established-endpoint ownership before API-key transmission and controller
lifecycle/health/restart integration. ADR-0019's reviewed bootstrap,
security-event/issuer-authority, reader compatibility and trust activation
gates remain mandatory. The owner asked to consolidate Mac work after the
Windows stage; this scoped PASS does not authorize switching platforms early.

No M8-complete, full-Windows-M8, cross-platform or peer-replication PASS is
claimed. M7 write-back remains deferred/off. No Actions, commit, push or merge
was performed to record this receipt; original dirty checkouts are preserved.
