# M5 Slice 5 — isolated accepted-note pilot fixture

**Status:** implementation checkpoint; native timeline/detail review remains open.

The marker-only packaged pilot had no Brain identity, so Timeline correctly
returned `canonical_verification_unavailable`. This adds an explicit one-shot
`cryptexd pilot-fixture timeline-v1` command so the existing packaged UX can be
reviewed with a real accepted v1 `memory.note` without borrowing personal memory
or inserting canonical bytes outside `cryptexd`.

## Guard and write path

- Requires `--runtime-root` and `--confirm-isolated-pilot` explicitly.
- Accepts only an absolute runtime directory outside the repository.
- Requires the exact `m0-native-desktop-runtime-v1\n` marker and refuses every
  runtime containing anything except that marker. The pilot must be fresh.
- Acquires the runtime's daemon single-instance lock and initializes the normal
  install foundation before proceeding.
- Creates a random synthetic Brain identity, enrolls one local device using the
  OS SecureStore in the `dev` namespace, and writes one fixed synthetic `public`
  v1 note through `capture_memory_note` and the canonical writer.
- On success it writes `pilot-timeline-fixture.marker`; Desktop accepts an
  otherwise-populated pilot runtime only when this exact marker is present.
- No production/default runtime is eligible. A failed or uncertain run must not
  be retried against the same runtime; preserve it for inspection and use a new
  marked pilot only after resolving the failure.

## Windows pilot sequence

Run from PowerShell using a newly created runtime containing only the marker:

```powershell
$root = Join-Path $env:LOCALAPPDATA ("Cryptex-Dev\m5-s5-timeline-" + [guid]::NewGuid().ToString("N"))
$runtime = Join-Path $root "runtime"
New-Item -ItemType Directory -Path $runtime | Out-Null
[IO.File]::WriteAllText((Join-Path $runtime "pilot-runtime.marker"), "m0-native-desktop-runtime-v1`n", [Text.UTF8Encoding]::new($false))
& ".\cryptexd.exe" pilot-fixture timeline-v1 --runtime-root $runtime --confirm-isolated-pilot
if ($LASTEXITCODE -ne 0) { throw "Pilot fixture creation failed; preserve this runtime and inspect the error." }
& ".\cryptex-desktop.exe" --cryptex-pilot-runtime $runtime
```

The command prints only the synthetic Brain ID, event ID, and classification.
Launch Desktop only after the command exits successfully. Inspect Timeline order,
snippet/provenance, paging and full-note detail, then cleanly Quit through the
supported app menu so the daemon releases the pilot lock. Dispose of the entire
synthetic pilot runtime only after recording the review result.

## Verification on this Windows host

| Check | Result |
|---|---|
| `cargo test --locked -p cryptexd --bin cryptexd timeline_fixture` | **3/3 PASS** |
| Desktop `pilot_runtime` tests | **9/9 PASS** |
| `cargo check --locked -p cryptexd --bin cryptexd` | PASS |
| `cargo clippy --locked -p cryptexd --bin cryptexd -- -D warnings` | PASS |
| `cargo fmt --all -- --check` | PASS |
| `git diff --check` | PASS |

The tests use `FakeSecureStore` and a temporary runtime. They do not establish
Windows Credential Manager, macOS Keychain, packaged native UI, or Mac parity.
No Actions, merge, production-runtime change, or M5-complete claim.
