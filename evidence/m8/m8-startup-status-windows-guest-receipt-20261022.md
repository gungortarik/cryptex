# M8 startup/status Windows guest verification receipt

Date: 2026-10-22. Evidence class: owner-supplied terminal screenshot.

The owner ran the staged development package in the Win11 VM. Terminal shows:

```text
Fresh protected loopback-only native fixture prepared. No transport was launched.
Scoped Windows verification PASS.
This does not complete M8. No Actions, peer pairing or canonical writes were enabled.
Verification exit: 0
```

Guest report folder (wrapped across two terminal lines):
`C:/Users/Tarik/AppData/Local/Cryptex-Dev/m8-verification/profile-batch-68fe7065fe614873a77481d7b6b97de6`.

Package: `Cryptex-M8-StartupStatus-20261022.zip`, staged SHA-256
`1c9ba03b53801ce4b0a5cbc9a49fbdb82a3bb12d12339bb10ffc8bd27b814016`.
The screenshot does not independently show a guest ZIP hash or raw result files.
No exact test counts, native PIDs or individual native results are transcribed
as independently reviewed evidence here.

The staged runner prints the fixture-prepared line before its explicit native
tests, and the aggregate PASS only after all guarded Cargo steps, native fixtures
and receipt copies succeed. Thus the early “No transport was launched” line is
fixture-preparation scope, not a claim that the subsequent native tests launch
no process. The supplied result supports an owner-reported scoped batch PASS;
inspect the guest results/logs before making stronger per-test/native claims.

This advances the prior host execution blocker to a guest PASS receipt; it
does not resolve host Application Control 4551 or certify Windows M8 complete.
Next review input: guest `results.json`, native receipts and relevant logs.
ADR-0019 exact protocol/schema/issuer review, peer/incoming/convergence/cutoff
work and later consolidated macOS coverage remain open. M7 write-back stays off.
No Actions, commit, push or merge.
