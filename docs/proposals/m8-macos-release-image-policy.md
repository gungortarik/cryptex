# Mac M8 release-image policy candidate

Status is a disabled implementation candidate under ADR-0020.
Publisher identity, signed inventory provenance and bundle stability remain open.

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
and architecture need their own implementation and review. The candidate has
no runtime inventory reader or trusted-inventory constructor.

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

Then implement authenticated inventory verification and bundle stability under
that reviewed policy, followed by the signed Desktop and daemon product-path
tests. This candidate leaves M8 open.
