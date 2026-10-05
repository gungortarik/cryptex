# M0 security, notice and acceptance review

Status: **PARTIAL — renderer launch issue corrected with both-platform regression
proof; release development-root finding and remaining acceptance gates open**.
Implementation source `82c4d32`; final implementer audit at `8437559` in
`m0-final-gate-audit-20261005.md` records the path finding and retained-report
spot checks. No independent verifier or production security certification.
Authority: Master Spec, accepted ADR-0007/0013, M0 contract and Definition of Done.
This is implementer review, not human approval or a complete security certification.

## Reviewed boundaries

| Area | Observation | Scope/result |
| --- | --- | --- |
| Renderer executable selection | `p0_2_start_daemon` formerly checked requested sidecar only in pilot mode; normal sessions accepted a renderer-selected executable | Concrete finding corrected for all sessions; Windows and Mac regressions PASS |
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

1. **Mac boundary check closed:** operator/Codex reported PASS at full source
   `388e46a7e5f750d88e7fc786e0c108dcea5329e4`: Desktop 13/13 and CLI 4/4,
   no failures/skips/ignored; format/strict Clippy/frontend/locked release/diff
   PASS, every exit 0, real POSIX symlink refusal and clean fixture cleanup.
   Evidence: `p0-2/macos-sidecar-boundary-evidence-20261005.md`. This is
   validator/pre-Tauri CLI proof, not physical renderer invocation.
2. **ADR-0012:** both-platform scoped lifecycle/selection/recovery with synthetic
   packs exists. Revised proposal separates its M0 review/CI/human acceptance
   from mandatory signed installed update and production canonical gates.
   Windows certificate readiness found no available valid code-signing key;
   no certificate purchase/export or security-policy change is authorized.
3. **Milestone boundary decision recorded:** the owner approved preparing the
   concrete scoped ADR revisions on 2026-10-05. M1 includes OS secure storage;
   M14 packaging/updater coordination; M17 signed/notarized release and final
   notices. Revised ADR-0012/0004 remain PROPOSED. Production requirements are
   retained as mandatory product gates; no accepted specification or milestone
   is weakened and no later implementation is authorized.
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
7. **M0 development-root finding:** ordinary release Desktop defaults to the
   product namespace on both OSes, despite being a development probe. Correct
   this to Cryptex-Dev and verify both platforms before final CI. Existing
   outside-repo synthetic product-root runs remain historical evidence; no
   data move/delete or login change is implied.

No Actions run for this review. M0 remains **IN PROGRESS**; ADR-0004/0012 remain
**PROPOSED**. Final CI waits for the remaining review decisions
to avoid paying for a knowingly incomplete completion candidate.
