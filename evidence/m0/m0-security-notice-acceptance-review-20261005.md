# M0 security, notice and acceptance review

Status: **PARTIAL — renderer launch issue corrected on Windows; Mac check and
remaining acceptance decisions open**. Implementation source `82c4d32`.
Authority: Master Spec, accepted ADR-0007/0013, M0 contract and Definition of Done.
This is implementer review, not human approval or a complete security certification.

## Reviewed boundaries

| Area | Observation | Scope/result |
| --- | --- | --- |
| Renderer executable selection | `p0_2_start_daemon` formerly checked requested sidecar only in pilot mode; normal sessions accepted a renderer-selected executable | Concrete finding corrected for all sessions; Mac regression pending |
| Correction | Resolve approved sidecar in backend; require absolute ordinary request/expected files and canonical path equality before runtime creation or spawning; launch/store backend path; make displayed sidecar read-only | Windows three new Rust tests PASS; developer environment selection remains backend-controlled |
| Renderer content | React renders probe log/path as text; no raw HTML rendering or shell/filesystem plugin is installed; CSP limits connection origins to local IPC | Reviewed narrow foundation surface; no imported memory rendering or production UI acceptance claim |
| IPC | `services/cryptexd/src/ipc.rs` explicitly installs Windows current-user DACL and validates private Unix socket directory; bounded 64 KiB framing, version/role/capability and response correlation | Existing physical proofs and accepted ADR-0007 retained; roles are not cryptographic device identities |
| Canonical writer | Implemented IPC exposes only health; Desktop supervises/read-only IPC and test snapshots; Golden uses explicit synthetic benchmark packs | No new canonical writer/API introduced; production event schema remains outside M0 |
| Syncthing boundary | Test-only Node controller, explicit pins, authenticated loopback, isolated owned homes, no discovery/relay/NAT/auto-upgrade | Both-platform managed/changed-version fixture proofs retained; no shipped supervisor or secure-store implementation claim |
| Diagnostics | Syncthing runners print measurements/typed codes, not REST bodies or key/config contents; private key digests used only in memory for fixture equality | Scoped source review; not exhaustive dependency/native-log certification |
| Golden/Doctor | Golden manifest explicitly synthetic/no production schema; Doctor actual PID/IPC/lock checks with `not-implemented` canonical/Syncthing fields | Foundation requirements have both-platform proof; unimplemented checks are not healthy assertions |
| Rollback | Fresh isolated synthetic profiles; pause/clean stop and cold verified restore; rejected profile retained | No live canonical event rollback, migration-failure or power-loss acceptance |

## Windows correction verification

- Desktop Rust **20/20**; three new tests verify backend-selected path only,
  refusal to replace a backend developer override, and Windows hard-link alias
  refusal (Mac counterpart uses a real symlink).
- `cargo fmt --all -- --check`, strict Desktop Clippy, frontend build and release
  Desktop build PASS; lifecycle Node **160/160**, storage/sidecar **8/8**, no skips.
- No visible Desktop opened for this review; no startup/global Syncthing change.
- These are validator/CLI tests and source wiring review, not a physical renderer
  invocation test. Mac task: `p0-2/macos-sidecar-boundary-test-plan-20261005.md`.

## Notice inventory and remaining distribution work

Exact LF notice fingerprints match both the repository and packager:
LICENSE `3f3d9e0024b1921b067d6f7f88deb4a60cbe7a78e76c64e3f1d7fc3b779b9d04`;
AUTHORS `5a0044d13ddf6f013bdd5c2bc419bf45d6123c356567510237e82f304d113d48`.
Both-OS staged-package proofs recheck their bytes and membership. Original
CRLF failure and older bundles remain historical. No native binary is in Git.

The retained license is MPL 2.0. Its executable distribution section addresses
source availability and informing recipients how to obtain that source; its
notice section addresses preservation of license notice substance.
Primary source: [Mozilla MPL 2.0 sections 3.2/3.4](https://www.mozilla.org/en-US/MPL/2.0/).
This is an inventory review, not a declaration of complete distribution compliance.
The repository README supplies upstream release/source links; the staged verifier
currently includes only binary/LICENSE/AUTHORS. Installer inclusion, a delivered
source-access notice and the native binary's dependency-notice inventory have
not been established. They remain explicit prerequisites before distribution.

## Remaining acceptance decisions and limits

1. **New Mac boundary check:** 13 Desktop Rust tests, format/strict Clippy,
   frontend/release build and four negative pre-Tauri CLI tests. Passed unrelated
   Syncthing/native plans need no repetition.
2. **ADR-0012:** both-platform lifecycle/selection/recovery with synthetic packs
   exists, but proposed acceptance still explicitly requires signed package
   integration and production canonical acceptance. Windows certificate readiness
   found no available valid code-signing key. No certificate purchase/export or
   security-policy change is authorized/inferred from this review.
3. **Milestone boundary review:** roadmap M1 includes OS secure storage; M14
   includes signed/notarized packaging. These later product tasks must not be
   implemented opportunistically in M0. The owner must explicitly resolve how
   the proposed ADR's production prerequisites relate to M0 spike acceptance;
   this review does not waive them or change the roadmap/ADR status.
4. **Secret/permission boundary:** test-native config/private files are not
   production OS secure storage. Windows inherited profile ACLs were not
   independently verified here. Do not adopt fixture secrets as device identity.
5. **IPC availability limit:** framing bounds bytes, but synchronous `read_until`
   has no demonstrated whole-read deadline; the client's timeout bounds connect
   retry, not every read. Same-user stalled-peer behavior needs explicit follow-up
   before treating this as a hardened production API. No broader IPC acceptance
   or timing guarantee is inferred from successful health requests.
6. **Final CI/human gate:** readiness coverage is prepared; cloud execution and
   human diff/ADR acceptance remain open. Final CI can verify build/test/ACL jobs
   but cannot supply signing, notice completeness or a human decision.

No Actions run for this review. M0 remains **IN PROGRESS**; ADR-0004/0012 remain
**PROPOSED**. Final CI waits for the new Mac check and remaining review decisions
to avoid paying for a knowingly incomplete completion candidate.
