# M8 Windows managed profile and credential boundary

Date: 2026-10-20. Scope: the Windows-first M8 batch, on the isolated
`codex/m8-device-trust` worktree. Master Spec, accepted ADR-0004 addendum,
ADR-0014 and the M8 contract govern. This is a backend implementation boundary,
not a canonical schema, new trust protocol or production activation decision.

## Explicit profile ownership

`ManagedProfile::create` requires an absolute ordinary drive path outside Git,
an existing parent and a fresh opaque 32-character hexadecimal leaf. No startup
or IPC calls it automatically. The later controller must choose the app-data
location; this API does not turn arbitrary renderer paths into an authorized
profile. UNC/device paths, repository paths, reparse directories and existing
objects are refused. The non-Windows constructor refuses until the Mac adapter
is implemented and verified in the consolidated Mac stage.

The Windows creation call receives an explicit current-user owner and a
protected DACL with exactly one inheritable current-user FullControl grant.
There is no create-then-tighten interval and no repair of existing broad ACLs.
Top-down directory handles reject reparse points and omit DELETE sharing,
pinning each existing ancestor and the acquired profile while in use. An
exclusive non-inherited `session.lock` handle excludes another profile session;
kernel handle cleanup releases ownership after abrupt process exit.

Profile metadata opens use OPEN_REPARSE_POINT. Handle inspection rejects
directories, reparse files and hard-linked files. Every metadata/lock file must
have the current user owner and the sole inherited current-user FullControl
ACE. Creation explicitly sets the current-user owner even for elevated tokens.
Readers are bounded to 4096 bytes, require strict JSON with no duplicates or
unknown fields, and never recreate malformed/missing identity metadata.

This is a Windows native backend boundary, not an Obsidian Vault API guarantee.
It is not a sandbox from the same OS user or privileged OS actors, nor a
general-purpose directory permission repair tool. M7 write-back stays disabled.
It does not yet validate every native-generated Syncthing file or its config.

## API credential transaction

Initialization creates a separate OS-random, public 128-bit credential namespace
inside `profile.json`. It is independent of the directory leaf: identically
named profiles under different parents do not compete for one OS credential.
It is not a Brain/device identity, signing key or grant. The namespace is
persisted and read back; reopen does not replace it.

Labels reuse the REST adapter's single naming function:
`Cryptex[-Dev]-ManagedSyncthing` / `api:<credential namespace>`.
Enrollment accepts an existing native-generated API key from the backend only,
32–256 ASCII graphic bytes; it does not generate or import renderer keys.
The native config reader/provisioner remains a separate integration gate.

| State | Behavior |
| --- | --- |
| No binding, account absent | Flush immutable key digest binding, then write the identical key to SecureStore, read it back, flush readiness |
| No binding, account occupied | Refuse without writing binding or replacing the credential |
| Binding, no readiness, identical key | Complete the interrupted transaction; never substitute a different key |
| Binding, no readiness, different supplied/stored key | Conflict; refuse without repair |
| Ready, stored key matches binding | Load or repeat enrollment without another SecureStore write |
| Ready, credential missing/denied | Refuse; no replacement key or readiness reset |
| Corrupt/oversized/duplicate/unknown metadata | Refuse; retain evidence unchanged |

The binding is a BLAKE3 digest using the existing workspace implementation,
not an encryption scheme or authentication grant. The raw key is absent from
Cryptex metadata/diagnostics; Syncthing's own native private config is separately
protected runtime data, not a promise that native software has no private files.
Errors carry only fixed typed variants or operation labels/numeric OS codes.
Readiness is a local setup marker, not transport health or peer trust.

Writes use exclusive creation and `sync_all`; partial files fail closed.
Interrupted-transaction tests and process exit(97) prove restart behavior only,
not power-loss durability or hardware flush guarantees. No automatic rollback
deletes OS credentials after a production error; the separately scoped tests
delete only their unpredictable DEV synthetic accounts.

## Native verification scope and remaining controller gates

The opt-in native fixture variant creates a product-private profile, copies
only config/certificate/private-key files from the existing fresh synthetic
native fixture, enrolls that native-generated API key through the real Windows
Credential Manager, and uses the owned-process plus bounded REST boundaries.
It starts/stops twice, checks isolation, wrong-key rejection, log redaction and
clean exits, and explicitly verifies synthetic account deletion afterward.
It is not a product config generator, package installer or endpoint-owner proof.

Before production startup/IPC is wired, complete package/provenance and native
config provisioning/validation, established-endpoint ownership before sending
the API key, controller state/health/shutdown/restart integration, and relevant
negative tests. Before peer trust is activated, complete ADR-0019's reviewed
bootstrap/security-event/issuer-authority and reader-first compatibility gates.
No peer is authorized by these helpers. No Actions, merge, later milestone or
M8-complete claim follows from this implementation.
