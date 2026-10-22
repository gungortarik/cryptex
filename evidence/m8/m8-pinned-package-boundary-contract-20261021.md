# M8 Windows pinned native package boundary

Date: 2026-10-21. Continuation in the isolated M8 worktree on base `74bc3b1`.
Authority: Master Spec, ADR-0004 implementation addendum, ADR-0014, M8 contract.
No production startup/IPC or peer-trust activation is introduced.

## Identity and lifetime

`PinnedSyncthingPackage` accepts an explicit backend-selected Windows amd64
package directory with these exact files. No download, PATH lookup, system
installation adoption, alternate digest or newline normalization is supported.

| File | Bytes | SHA-256 |
| --- | ---: | --- |
| syncthing.exe (2.1.5, Windows amd64) | 27,448,104 | `36a0f7bc372f64fa7cc4f5654fa324c0dd9f7fef2e07565e00c6e1cf73f50344` |
| LICENSE.txt (repository LF) | 16,726 | `3f3d9e0024b1921b067d6f7f88deb4a60cbe7a78e76c64e3f1d7fc3b779b9d04` |
| AUTHORS.txt (repository LF) | 16,811 | `5a0044d13ddf6f013bdd5c2bc419bf45d6123c356567510237e82f304d113d48` |

The executable is the retained native artifact already used for M0/M8 tests.
Notice provenance/source links remain in `third_party/syncthing/2.1.5/README.md`.
These digests prove identity with retained bytes, not independent publisher
authentication, installer inclusion, complete dependency-license compliance or
platform signing/notarization. Distribution/release review remains required.

An ordinary absolute drive path outside Git is required. Every ancestor and
the package directory are opened top-down with OPEN_REPARSE_POINT and without
DELETE sharing, and directory attributes are checked from the open handles.
The three files are opened without write/delete sharing; reparse points,
directories and multiple hard links are refused. Hashing is streamed/bounded
from those handles. A pre-existing writer prevents acquisition. Missing,
wrong-size or wrong-digest files fail before CreateProcess.

The pinned executable pathname feeds the existing atomic owned-job launcher.
`PackagedProcess` retains an Arc to all pins independently of the package
factory, including after the root process exits. It offers only a shared process
borrow plus explicit exit/wait/cleanup methods, not mutable access or ownership
extraction that would separate a live process from its package guard.

Drop waits for owned-tree termination, using a bounded forced cleanup when
necessary. Forced cleanup is never reported as a clean stop. If termination
cannot be observed, Drop emits a structured redacted failure and deliberately
retains an extra pin set until daemon exit rather than releasing a possibly
live executable. This exceptional fail-closed resource leak is not a successful
shutdown or restart authorization. An integrating controller must report the
explicit lifecycle result and refuse replacement after unresolved cleanup.

The boundary is not a sandbox against same-user process injection, an OS
administrator or a hostile kernel. Non-Windows and non-amd64 targets refuse;
their pins/platform adapters remain part of the later consolidated stage.

## Fixed generation role

Added a backend-only `generate --home <profile> --no-port-probing` launch role,
using the same atomic job assignment, minimal environment, no shell and
STNODEFAULTFOLDER as the serve role. The executable cannot be supplied through
an untrusted renderer, API argument or ambient PATH.

The explicit native test generates new config/cert/key only in a fresh protected
synthetic ManagedProfile. Generation does not start a Syncthing server. Generated
defaults MUST be provisioned and validated before serving. This change does
not implement that config provisioner, enroll the generated API key, choose
peer folders or authorize a native ID. The lower-level serve factory still
requires a controller to retain/validate the protected profile, config,
credential binding and native identity. It is not called by production startup.

## Verification and remaining scope

Default tests cover exact-size/digest behavior, no CRLF normalization, mutation
and replacement denial, pre-existing writers, hard links, root/ancestor junctions,
missing/malformed packages and Git path rejection. An opt-in native test also
requires exact notices, rejects same-size executable/notice tampering, retains
pins after dropping the factory, observes clean generation, and verifies Drop
cleanup with a retained OS observer. Synthetic secrets/config bodies are not
printed. No user-global Syncthing state is touched.

Next: strict native-config provisioning and interrupted-initialization recovery;
then controller start/health/restart/shutdown integration. Keep ADR-0019 protocol,
canonical security schema/writer and trust activation gates separate. Mac,
cross-device convergence, release/CI/human-review gates and M8 completion remain
open. Actions, commit, push and merge were not run.
