# M8 controlled product-flow activation and two-device acceptance

Status: PROPOSED; owner selection required before changing the disabled boundary.
Source audit: `8cf54e4`, November 15, 2026. No activation code is changed here.

## Verified starting point

The owner's `MAC-AKTARIM-SONUC-5ca78781.zip` matches `7fc6d44` and passes the
isolated signed local pairing/import/restart fixture at a 256-descriptor limit.
It does not send canonical records over a real connection between two machines.
See `evidence/m8/m8-macos-descriptor-owner-acceptance-20261115.json`.

The following are current implementation gaps, not requests to repeat that test:

| Boundary | Current source | Required change |
| --- | --- | --- |
| Ordinary Mac owner routes | `m8_owner_channel::require_candidate` returns Disabled; `image_policy::configured_image` returns Unconfigured | Explicitly provisioned development product policy and native routing; no Unix fallback |
| Actual application entry | `candidate_ui::entry` handles special feature-gated arguments; ordinary `main` uses the existing launcher | Normal launch of the selected development build uses verified content and supervised native ExistingProfile sessions |
| Real peers/folders | `managed_profile/native_xml.rs::inspect` accepts one self device and no active folders; `configure_transfer_test` is Windows test-only | A separate production policy derived from verified Cryptex trust, with strict configuration readback and restart binding |
| Incoming work | Mac `ipc/native_profile.rs` connects the retained daemon instance to inbox discovery and IncomingController | Feed this existing path from the actual managed transport folder, on both targets |
| Outgoing work | Canonical writer seals immutable segments; no ordinary transport exporter is connected | Daemon-only verified sealed-segment export, isolated from canonical storage |
| Protection UI | Native status deliberately reports LocalOnly/Attention and remote durability false | Preserve truthful status; never promote transport completion to remote durable acceptance |

## Decision requested

Select a controlled local development product integration, followed by one
two-device acceptance package. This is a scoped amendment to the disabled-only
development boundary of ADR-0020/ADR-0021, not approval to distribute publicly,
enable all builds, purchase Apple membership, or declare M8 complete.

The owner already selected macOS 26+ and local development signing. Keep those
decisions. Use an independently selected local signing certificate's public DER
as a build-time pin and an authenticated inventory for the exact release and
architecture. Keep its private key on the signing Mac in OS secure storage.
Do not adopt a certificate discovered beside the app, ship a private signing key,
turn a disposable CI identity into the owner's persistent publisher, or allow an
environment variable to unlock a missing policy. A missing certificate, inventory
or existing initialized profile produces a clear refusal.

The selected build's normal application entry may use the native owner adapter
only after these checks. Other builds remain disabled. Mac health/other accepted
Unix operations and Windows pipe authentication retain their existing policies.
Extract only reusable verified startup/session code from the fixture entry; no
fixture seeding, synthetic identities, fault IDs, or test-only trust override may
be reachable from normal entry. First-run profile initialization is a separate
existing daemon operation, not an excuse to seed a profile from the UI.

## Concrete transport direction proposed for review

Keep the approved isolation boundary: the managed shared tree is
`transport/incoming`, never `canonical/events`. For bidirectional operation,
propose a strictly configured send/receive exchange tree. The daemon exports
verified immutable sealed segments into that untrusted exchange tree as copies;
the same verified IncomingController admits received segments to canonical
storage. This extends the previously approved receive-only implementation slice
in ADR-0019 section 5.1.1 and must be accepted and reviewed before implementation.

- Derive admitted device IDs and transport associations from current verified
  canonical trust under the existing admission/locking rules. A successful
  Syncthing connection, pairing metadata file or renderer-supplied ID is not trust.
- Preserve `<device-id-hex>/<20-digit-segment-index>.seg` naming and original
  signed event bytes. No master, sequence rewrite or re-signing of remote events.
- Only cryptexd exports validated sealed records. Never share an active tail,
  SQLite/index, OS credential, import journal, private key or arbitrary UI path.
  Add an admitted bounded local-writer sealing checkpoint if normal capture would
  otherwise leave new records solely in the active tail; never lower production
  sealing rules merely to make a fixture pass.
- Verify existing exchange bytes before treating an export as present. Never
  overwrite a mismatched destination or follow links. Stage copies and publish
  with the existing platform no-replace/flush rules. A crash/retry is idempotent.
- Replayed local/forwarded segments must be a verified no-op at canonical
  publication. Remote removal/conflict in the exchange tree cannot remove or
  replace canonical memory. Syncthing conflict files are not canonical input.
- Reconcile transport admission after restrictions, retirement, integrity holds,
  peer changes and restart. Stop new export/admission when the shared policy gate
  is held; do not claim erasure of bytes already sent to a peer.
- Persist a versioned derived configuration binding and validate the complete
  allowed peer/folder/path set before start and after configuration writes.
  Reject extra folders, automatic introductions, unapproved peers and canonical
  aliases. Do not promote the synthetic TransferTestPolicy to product authority.
- Begin the acceptance run on an explicit LAN connection between the owner's
  Windows machine and Mac, with discovery/relays disabled for that run. Later
  network/discovery policy is not silently selected by this proposal.

The exchange tree remains disposable transport data. Export/repopulation limits,
full-disk handling and safe restart must be fixed and tested in the implementation
contract before enabling the exporter. No automatic conflict repair, purge,
canonical rewrite or protocol weakening is authorized by this direction.

## Implementation and acceptance order

1. Provision the selected local release policy; wire normal development entry to
   protected content, exact native process supervision and fresh owner sessions.
   Test missing/wrong signer, changed inventory/content, stale sessions, launch
   failure, both shutdown directions and existing-profile persistence.
2. Implement trust-derived peer/folder policy and bounded daemon export under the
   existing admission barrier. Test unapproved/mismatched transport association,
   configuration drift, link/path aliasing, changed source/destination, replay,
   interrupted export and restart; preserve Windows and Mac containment rules.
3. Connect actual managed transport discovery to the same live daemon's incoming
   controller. Verify captures and owner-operation reconciliation while incoming
   work is active. Re-run affected native CI and source review before packaging.
4. Deliver one coordinated Windows/Mac package with fresh isolated persistent
   profiles and normal application paths. The package gathers only allowlisted
   aggregates and public pairing material; never export private keys or raw notes.
   User interaction remains launch, explicit pairing review and result ZIP upload.
5. Acceptance: capture while disconnected on both devices; reconnect; verify both
   receive the same signed canonical set and derived results; stop/restart both;
   repeat the delivery with no duplicate append; interrupt actual transport and
   recover; propagate the selected cutoff/restriction and verify refusal beyond
   it. No copied `.seg` shortcut may substitute for the network leg.

Cross-device durable acceptance requires matching daemon-verified canonical
digests on both endpoints after restart, bound to this run and their expected
identities. Syncthing's synced counter alone is insufficient. Do not add a new
signed remote receipt protocol or display Protected without an accepted evidence
contract. Keep LocalOnly/Attention if that product evidence is unavailable.

These are three implementation boundaries (launch policy, peer/export control,
live incoming integration) followed by one combined device test. They are not a
claim that M8 needs only one more test. Final installed UX, protocol/security
review and the milestone completion ledger remain required.

## Alternative

Keep ADR-0020/ADR-0021 disabled-only and run another explicitly isolated network
fixture. It can prove the network leg but cannot close the normal product-flow
gap requested by the owner. Do not label that alternative product integration.
