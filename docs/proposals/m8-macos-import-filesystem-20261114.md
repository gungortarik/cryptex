# Mac incoming filesystem and product integration

Scope: implement the owner's requested Mac filesystem adapter and connect the
existing verified native/domain seams. ADR-0020's ordinary-product activation
gate remains separate from code integration and exact-source native acceptance.
No event format, authority rule, signer choice or Windows mechanism changes.

## Filesystem capability

All incoming snapshot, prepared/committed journal, R4 review/held evidence and
cleanup operations must use daemon-owned directory descriptors on Mac. Validate
absolute traversal-free paths, each no-follow parent, effective-UID/root-owned
ancestors with no untrusted write permission, and the current-owned selected
directory. Reject extended allow ACLs; retain deny ACLs. New private directories
and files use0700/0600 at creation; do not repair/adopt unsafe existing objects.
Only the OS-owned `/var` and `/tmp` aliases may translate to `/private/...`;
untrusted/user symlinks remain refused. No `.git` ancestry or runtime in a repo.

Directory iteration uses a duplicate of the checked directory descriptor.
Open/create/stat/mkdir/unlink operate relative to checked descriptors and single
component leaves. Retained descriptors plus parent/child dev/inode revalidation
bind the original objects; POSIX descriptors do **not** prevent rename. Observed
namespace drift before/after an operation is uncertainty/hold, never clean
rollback or permission to follow a replacement. Supported private staging writers
remain serialized by the existing import slot and runtime/canonical locks.

Retain the original canonical/events, journal, staging/transaction and affected
device directory capabilities through the import, not just the runtime root.
Canonical inventory/device locks also retain their original parent and leaf
identity; a proof bound to an old lock tree cannot publish into a replacement.
An observed namespace refusal must skip eager staging rollback. Intentional
bounded cleanup may forget only the successfully removed private subtree;
remaining original capabilities are never refreshed to a new namespace.

Publication uses Darwin `renameatx_np(..., RENAME_EXCL)` between checked parents,
single-link regular sources, same-volume validation and destination identity
readback. Refuse an existing destination even for same-object/case aliases. No
hardlink/copy/overwrite fallback. Flush regular data before promotion, then both
parents; failures preserve journals and shared admission hold. Process interruption
tests are not physical power-loss certification.

Apple primary references:
[Darwin syscall](https://github.com/apple-oss-distributions/xnu/blob/main/bsd/kern/syscalls.master),
[exclusive-rename cases](https://github.com/apple-oss-distributions/xnu/blob/main/tests/rename_excl.c),
[fcntl definitions](https://github.com/apple-oss-distributions/xnu/blob/main/bsd/sys/fcntl.h).

## Integration order and acceptance

First prove the descriptor adapter, failure paths and actual import/restart/R4
suite on Mac; preserve the Windows suite. Then connect owned helper transport,
bounded incoming hints, shared admission and protection observations to the
existing kernel-attributed native startup/request/session path. Product context
must use an existing OS-backed profile, never seed synthetic data or authorize
from an incoming JSON flag. Enforce the already selected macOS26 floor and
configured build/image policy. Protection is derived from verified live state;
pairing declarations or mere process liveness never mean protected replication.

Keep ordinary activation closed until the required reviewed configured image
policy and native/product acceptance are satisfied. Prepare a concrete native
Desktop/daemon package and acceptance evidence; do not count an isolated helper
or a source-only integration as full installed-product/M8 completion.

## Default-off existing-profile integration candidate

`ExistingProfile` is a distinct, explicitly selected local-development mode. The
normal product gate stays disabled. It uses the independently pinned signer,
CMS inventory, protected frontend and per-message native allocation seal, with
an explicit SDK/runtime macOS26 refusal before profile preparation. The daemon
opens an already initialized, private, outside-repository Keychain profile; it
never enrolls/replaces a Brain, generates a signing key, seeds a note, trusts an
incoming role/PID, recognizes a fixture fault request ID or falls back to Unix.
The shared dispatcher owns connection-local pending sessions and retains the
actual admission permit through the bounded send. The isolated test driver is
only a wrapper and keeps its seeding/status/fault behavior outside that core.

One mutable instance owner serializes domain work and incoming publication.
Before native readiness, authenticated peer identities establish empty canonical
lock slots and full R4 recovery plus M3/M4 rebuild establishes derived watermarks.
The original namespace is retained; startup failure never enables a reader.
Already enrolled transport helpers can reopen outside the temporary directory
only with private no-follow ancestor/ACL checks, the original directory capability,
image/native ownership and exact active-file cleanup identity. No implicit helper
profile creation or peer/folder configuration is added by the product mode.

The real helper StatusReader allocation is runtime-bound; fresh observations and
canonical local transport bindings precede exclusive incoming publication.
Bounded, descriptor-based inbox discovery produces hints only. A process-local
ctime/inode/size/mtime cache suppresses unchanged completed/rejected hints, never
supplies trust or bytes, and is discarded on restart. Publisher verification and
idempotent replay remain mandatory. Waiting/deferred/held hints stay pending;
unknown or overflowing discovery remains explicit attention. Ordinary admission
is released before exclusive import. Native backend heartbeat and owner requests
run on one serialized owner, not a second instance or a cloned writer lease.

Native status has no protected/replicated success state and always reports remote
durability as unverified. Held/rejected/unavailable outcomes are attention; loss
of status replaces earlier success with unavailable. The normal legacy CLI
metadata diagnostic is unchanged. No renderer command supplies import hints,
paths, native content binding, signing keys or filesystem access.

The extended signed pairing harness must open this existing-profile mode, copy
only untrusted transport bytes, publish through the real owned controller,
restart/replay without event changes, and reject an old on-wire confirmation and
Desktop-local handle. This is a local two-engine integration receipt, not another
physical Mac/device transfer or power-loss certification. Full installed-product
activation, actual peer/folder synchronization and remote durability acceptance
remain distinct gates.
