# M8 normal LAN settings and roster reconciliation

Status: ACCEPTED — owner chose “Dar LAN adresi istisnasını uygula (önerilen)”
on November 15, 2026. Implementation and acceptance evidence remain separate.

The accepted [controlled product integration](m8-product-flow-activation-20261115.md)
already permits derived peer configuration and canonical roster reconciliation.
The private replacement transaction is within that authorization. This proposal
does not reopen the transport architecture or request another fixture milestone.

## Exact conflict and proposed amendment

[ADR-0021](../adr/ADR-0021-disabled-macos-transport-helper.md), Decision, says:
“Renderer APIs receive no shell, paths, ports, credentials or helper controls.”

The explicit LAN setup needs the public endpoint of the other device. That
sentence presently also forbids displaying/supplying its transport port. Do not
interpret the earlier general product-integration approval as deleting this rule.

Recommended narrowly amended sentence:

> Renderer APIs receive no shell, filesystem paths, REST/API ports, credentials,
> process identifiers or helper controls. The authenticated owner settings route
> may exchange bounded public RFC1918 IPv4 transport endpoints only. A public
> endpoint is a connection preference, never device trust or a launch capability.

Keep all other ADR-0021 requirements. This is for the already authorized locally
signed development product, not public release or general discovery/relay policy.

## Concrete owner-facing result

In Settings / Devices, use the existing visual system and ordinary owner channel:

- **This device:** selected private LAN IPv4 address and a read-only public
  transport port from its verified enrolled profile. Local native ports remain
  immutable; this change does not migrate API or listen ports.
- **Trusted devices:** rows supplied by a fresh verified canonical snapshot, with
  device identity and its public `IPv4:transport-port` preference. No editable
  trust list, manual native identity override, folder selector or package path.
- **Save connection settings:** bounded validation and revision comparison;
  a successful save is distinct from an observed active transport.
- **Pause sync:** a local connection preference that revokes/drains the worker.
  It does not restrict a device, erase memory, or terminate the canonical daemon.
- **State:** Paused, Address needed, Applying, Ready locally, or Attention.
  These states never mean remote durable acceptance or Protected.

A newly approved device needs its own explicit endpoint before activation. A
removed/restricted device is omitted from the next derived active configuration
even if an old address preference remains. Unknown IDs submitted by the renderer
do not create peers. No shell commands or repeated JSON copy loops are the normal
settings UX.

## Implementation contract after acceptance

1. A closed, versioned, bounded preferences record is daemon-owned private derived
   data outside the repository. No keys, memory bodies, canonical authority or
   helper paths belong in it. Unknown schema/fields, duplicate keys, partial
   staging and stale settings revisions return typed errors without silent reset.
2. Authenticate the owner route on both Windows and Mac. Keep ordinary retrieval
   clients unable to update preferences. Mac uses its verified native channel;
   there is no Unix fallback for the settings mutation.
3. The daemon coordinator revokes the old epoch and unpublishes its ready status,
   then waits for exact owned cleanup. An uncertain shutdown blocks replacement.
   Old status handles and pending approvals cannot silently switch generations.
4. Recover/rebuild under the existing barrier, acquire exclusive admission before
   the canonical scan, derive peers from the fresh locked snapshot, and retain
   admission and namespace/profile pins through installation and readiness.
   Settings alone cannot supply the list of authorized peers.
5. A completed derived configuration can transition to the fresh target through
   the bounded private journal. A pending target is resumed only if it still
   equals current authority. A later policy mismatch stays stopped with Attention;
   do not finish or start stale intent merely to recover a journal.
6. Reconcile after pairing/restriction/retirement, incoming trust changes,
   settings changes and restart. No automatic security-hold clearance. No worker
   means local-only, never permission to use adapter-cached peers.
7. Preserve cached valid historical imports even after the final network peer is
   restricted: do not consume/reject their hints just because active roster is
   empty. The coordinator's zero-peer state must also preserve a separately
   verified local identity path for future pairing; do not reopen the old exchange
   through the bootstrap shortcut. Specify and test that local-only transition
   before enabling the coordinator.
8. Continue the existing sealed-export and real Windows–Mac acceptance work;
   completing this settings boundary alone cannot close M8.

## Required acceptance

Cross-platform request authentication and strict parsing; stale revision and
duplicate/unknown peer refusal; empty/restricted roster and later pairing;
stop/drain-before-replace ordering; stale readers/approvals; no stale-policy
restart after interrupted installation; settings persistence/readback; public
endpoint change across two real owned starts; import during roster change;
native window/settings UX; exact-source CI and independent source review.

The unselected alternative was to keep the no-ports rule unchanged and design a separate
daemon-only endpoint exchange/import workflow. That is a different owner workflow;
it must not be silently substituted for normal editable LAN settings.
