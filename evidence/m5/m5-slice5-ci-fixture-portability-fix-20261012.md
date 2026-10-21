# M5 Slice 5 — fixture portability fix and required CI

**Status:** Required CI PASS; native macOS Timeline/detail review remains open.

## Build identity

- Branch: `codex/m5-capture-timeline`
- Commit: `1f7420525824783417bdff37d5e79890e21a9961`
- Product-code commit under native review: `ae6f5a64dbf2825080778462257068744197c501`

## CI failure and correction

The first required CI run (`38274870464`, candidate `d00f3ceb81bafffc113b798e05293fe2d77b7aff`) exposed a test-only portability assumption in `timeline_fixture_creates_an_accepted_v1_event_through_capture_service`: it treated the first `canonical/events` directory entry as the device directory. macOS returned a file entry first, so the test attempted `read_dir` on a file and failed with `NotADirectory`.

The test now filters entries to find the device directory. It does not change product behavior or weaken the assertion that the capture service created a canonical segment.

## Verification

- Windows `cargo fmt --all -- --check`: PASS
- Windows `cargo test --locked -p cryptexd --bin cryptexd timeline_fixture`: **3/3 PASS**
- `git diff --check`: PASS
- Required CI run [38275499486](https://github.com/gungortarik/cryptex/actions/runs/38275499486), HEAD `1f7420525824783417bdff37d5e79890e21a9961`: Windows readiness PASS; macOS readiness PASS; Windows IPC cross-account ACL PASS. Unsigned Windows package job was skipped as configured for manual-only packaging.

## Remaining gates

- Native macOS packaged Timeline row, full-note/provenance detail, and clean-shutdown review.
- Final M5 DoD/security/UI review and human review.
- M5 is not complete; M6 has not started.
