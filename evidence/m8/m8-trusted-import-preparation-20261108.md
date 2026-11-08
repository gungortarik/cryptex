# Trusted-peer import preparation

Date: 2026-11-08. Local policy tests passed; native VM preparation pending.

## Authority and test shape

Source and receiver are the original isolated, previously owner-approved pilot
profiles in the existing Windows VM. Each private signing key stays in that
VM's OS store. The host will relay exported public synthetic segments over the
already tested native transport. A carrier, key descriptor or signed message
does not create local trust. This is not a new cross-OS owner-admission test.

`pilot-fixture trusted-import-prepare-v1` requires explicit isolated-pilot
confirmation, ordinary marked directories outside the repository and distinct
source/receiver roots. It holds runtime instance guards and performs startup
reconciliation before canonical inspection. A fresh marked negative root is
required. Both existing local anchors are loaded from their secure-store keys;
their exact peer key bindings and unrestricted authority must be reconstructed
from verified signed local history. No routine approves a peer or writes a
trust journal/roster to make the test pass.

Only the known synthetic timeline/probe note texts and verified public control
history can be exported from the source. An unrecognized source note fails
before the new capture/negative fixture is created. Source capture uses the
normal idempotent capture receipt and writer with seal threshold one. The
receiver's already-approved empty writer slot is allocated with the existing
canonical lock API. Its derived M3/M4 state is rebuilt from the same local-anchor,
cutoff-aware verified snapshot used by import safety; security controls are not
projected as memory notes. Snapshot locks remain held through these derived
updates. The normal gate and its exact existing watermarks are not relaxed.

The fresh negative device signs a standard v1 note claiming the same Brain ID;
it receives no approval. Its event UUID differs from the capture UUID. A public
descriptor permits an independent signature-only check, while full import must
still reject the unknown device. Exported manifests carry file hashes and
bounded relative segment names; copied files are rehashed. No secret, personal
note, arbitrary path or entire runtime directory is exported.

## Verification

Commands used:

```text
cargo test --locked -p cryptexd --lib trusted_import_probe::tests -- --test-threads=1
cargo test --locked -p cryptexd --lib incoming_validation::tests -- --test-threads=1
cargo test --locked -p cryptexd --bin cryptexd -- --test-threads=1
cargo clippy --locked -p cryptexd --all-targets -- -D warnings
cargo fmt --all -- --check
cargo build --locked --release -p cryptexd
powershell.exe -NoProfile -File scripts/m8/test-trusted-import-package.ps1 -Package <built-zip>
```

- Three new tests passed. Genuinely signed reciprocal approval fixtures admit
  the three-event peer stream through the full unchanged `validate_incoming`
  API. A second identical validation succeeds without publishing either time.
- Corrupting an existing materialization watermark still holds validation.
  A valid, same-Brain-ID but unapproved signature is rejected. A modified
  trusted stream is rejected. Unpaired profiles and non-synthetic source notes
  are refused before the extra source sequence is allocated.
- Nine incoming-validation regressions and fifteen CLI tests passed, including
  instance/recovery precedence and the explicit-confirmation boundary.
- Strict Clippy, formatting and release build passed. Package checks passed
  allowlisted membership/no source records or keys in the VM delivery ZIP,
  daemon digest, missing-profile failure ZIP, read-only copy, and path/hash
  rejection.

Initial development failures exposed incorrect assumptions in the test
preparation: unrestricted devices have a `u64::MAX` cutoff entry, the legacy
generic rebuild uses a different security-control progress view, and M3 and M4
both need rebuilding. The helper now uses the existing verified local-anchor
snapshot and ordinary projection routines. No failing assertion, admission gate
or unknown-device check was weakened.

## Pending native evidence

Prepared package `cryptex-guven-hazirlik-c223e671.zip` has probe UUID
`c223e671-3c5e-413d-a82c-419ed88a508f`. The owner must run its single launcher in
the VM and return the source bundle/error. The subsequent relay/receiver package
has not yet run. The actual VM policy result, managed adapter integration,
canonical publication, platform/power-loss coverage and independent review
remain unproven. `validated_only` must never be described as durable import.
