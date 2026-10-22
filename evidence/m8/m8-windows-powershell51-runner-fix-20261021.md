# Windows PowerShell 5.1 verification-runner correction

Date: 2026-10-21. Owner's VM transcript verifies original ZIP hash
`1058be763a7b198113a15d57c25bc3da05ae86fb388c0b3574174d2cd35c775a`.
The runner reached Cargo but Windows PowerShell converted its normal
`Downloading crates ...` stderr line into a terminating NativeCommandError
because `$ErrorActionPreference = 'Stop'` surrounded native stream redirection.
This is a runner defect; it establishes neither a Cargo nonzero exit nor a
Windows verification PASS. The prior 4551 host application-control refusal is
a separate condition.

The runner now resolves the Cargo application before invoking it, initializes
the native global exit status to failure, temporarily permits native stderr
capture, immediately restores the caller's preference in `finally`, and checks
the actual native exit code. Nonzero exits still stop and record the failed
step. Failure to resolve/launch an executable cannot become a successful step.
No product Rust, native transport, security setting or test expectation changed.

Added `scripts/m8/test-verification-runner.ps1`, which extracts the actual helper
AST without executing the verification batch and mocks only executable lookup.
Real Windows PowerShell 5.1 child fixtures check stderr with exit 0, actual exit
7, and an absent executable. All three passed; preferences were restored and
stderr retained. Proof:
`C:/Users/Tarik/AppData/Local/Cryptex-Dev/m8-verification/ps51-runner-regression-590a70b22a4e4d4c927438469fc35f56`.
An intermediate reset using local `$LASTEXITCODE` incorrectly shadowed the
native global status; the regression caught it and final code explicitly uses
`$global:LASTEXITCODE`. The missing-launch fixture can throw its native launch
exception directly; the final regression requires fail-closed behavior rather
than weakening it to require only the wrapper's message.

Updated the runner's entry in the 16-file LF-normalized source manifest:
old `c2ad2e39a868e9d4424f9ed85a68a40ba2a797856a61ad6690cf458c8e678f24`,
new `3cfa2b7000910c613b20f3af0ae21d6fd78e441f435c1175b0e67755a722e213`.
The sealed original ZIP and original proof manifest remain unchanged historical
artifacts. A deterministic owner-side two-file repair (runner and its manifest
entry) was checked against the original extracted ZIP: generated runner bytes
match the corrected, regression-tested source exactly. The repair checks the
known new runner hash before changing the manifest; source checking is not
disabled. No full batch/native guest result is claimed until rerun.

No Actions, commit, push, merge, canonical writes or M8 completion.
