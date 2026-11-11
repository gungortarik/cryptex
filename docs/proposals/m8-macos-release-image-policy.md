# Mac M8 release-image policy candidate

Status is a disabled implementation candidate under ADR-0020.
Product publisher provisioning and live bundle integration remain open.

## Implemented disabled inventory and content snapshot

The private Mac candidate now verifies an attached CMS signed-data inventory
with Apple's Security framework. The exact signer certificate DER is supplied
by independent release policy. The verifier requires one signer, exact equality
between signed content and the supplied inventory bytes, signature success and
offline CodeSigning trust anchored solely to that pinned certificate. It refuses
encrypted or non-data wrappers before invoking the native decoder. No signer
certificate is imported into a Keychain. Product provisioning remains absent.
[Apple CMS signer status](https://developer.apple.com/documentation/security/cmsdecodercopysignerstatus(_:_:_:_:_:_:_:)).

Schema 1 binds the purpose, expected release ID and architecture to Desktop and
daemon paths and CodeDirectory hashes. Every listed member has an exact byte
length and SHA-256 digest. Duplicate JSON keys, unknown fields, duplicate or
case-colliding paths, traversal and ambiguous image roles fail closed. Candidate
limits are 1 MiB inventory, 2 MiB CMS, 4,096 files, 64 MiB per member and 256 MiB
aggregate member bytes.

Snapshot reads walk relative components through owned directory descriptors
with no-follow flags. Regular singly linked files must match the authenticated
length, metadata before/after the read and expected digest. The completed
snapshot owns immutable byte buffers. Later source-path replacement cannot
substitute the contents returned from those buffers. A partial or failed read
returns no snapshot. Image expectations require this completed snapshot.

This protects the authenticated content view. Extra unlisted source files are
excluded from that view. The candidate does not issue a verified executable path,
prevent source-tree writes, or establish the bytes used by an existing webview.
An enabled product must consume the protected content and bind running images
to its inventory expectations. Static on-disk bundle validation and a live
product-path grant remain outside this snapshot API.

The fixture CMS signatures use a disposable self-signed CodeSigning certificate.
Only public certificate, content and signatures are committed. The private key
was deleted after fixture generation. The fixture certificate is valid from
2026-11-11 through 2036-11-08. It supplies test trust only.

The candidate code in `m8_owner_channel/image_policy.rs` builds a requirement
from a validated Team ID and a role-specific identifier. Its proposed certificate
constraint requires the Apple Developer ID intermediate and Application leaf.
The native adapter also receives the exact expected CodeDirectory hash.
This certificate class is a proposed distribution policy. The product has no
configured publisher or image pair.
[Apple code-signing requirement implementation](https://github.com/apple-oss-distributions/Security/blob/main/OSX/libsecurity_codesigning/lib/StaticCode.cpp).

Desktop and daemon must have distinct identifiers and image hashes. The policy
validates the whole pair before selecting either role. Unknown or malformed
identity fields, missing image hashes and role ambiguity fail before native
allocation. The native adapter borrows the selected expectation and copies it
while configuring a process pin.

## Release inventory provenance remains a prerequisite

Both binaries cannot embed each other's final CodeDirectory hashes as compiled
executable policy bytes. Changing
either binary's embedded peer hash changes its own signed image and creates a
circular signing dependency. The eventual build must provide an independently
authenticated inventory of the final signed components, with its publisher
anchored to reviewed product policy. Its verification and binding to one release
and architecture are checked by the private verifier described above. Product
configuration has no signer or trusted inventory factory yet.

The test fixture uses fabricated identifiers, team and hashes. It checks policy
validation and role mapping. It does not provide a real publisher credential.

## Bundle stability remains a separate prerequisite

The inventory and expected process hashes alone do not establish a stable
observation of the whole application bundle. A path check followed by a second
path check, or keeping an ordinary descriptor open, cannot be claimed as an
atomic bundle snapshot. Apple limits static validation to code protected from
concurrent modification.
[Apple static validation contract](https://developer.apple.com/documentation/security/secstaticcodecheckvalidity(_:_:_:)).

Before native product authorization, specify how the verified release inventory,
both running signed images and the sealed resource tree remain consistent while
bundles are replaced or modified. Refuse unresolved identity or modification
outcomes and discard the session's pending grants. The current native backend
checks sender identity on every received frame, while the product gate refuses
all M8 owner operations.

## Next decisions

The owner needs to identify the intended distribution publisher and approve
product bundle identifiers. Only public Team ID and identifiers are inputs to
that decision. Certificate private keys and account credentials stay in their
existing signing storage. An existing development-certificate experiment does
not establish the proposed Developer ID release policy.

Then provision the reviewed signer, route consumers through the protected bytes,
and bind the live Desktop and daemon to that release inventory. Complete signed
product-path tests before activation. This candidate leaves M8 open.
