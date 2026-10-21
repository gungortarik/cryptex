# M6 P1 — macOS cross-volume follow-up

**Date:** 2026-10-14  
**Host:** macOS 27.0.1 arm64, Python 3.13.7  
**Harness:** `scripts/m6_attachment_projection_spike.py`

## Cross-volume result

The harness used a temporary 256 MB APFS disk image named `CryptexM6P1Spike`, mounted under a unique run parent.

| Check | Result |
|---|---|
| `cross_volume.same_volume` | `false` |
| Hard link | unavailable, `OSError` errno 18 (`EXDEV`) |
| Copy fallback | PASS; size and SHA-256 matched |
| Same-volume fixtures through 64 MiB | PASS |
| Hard-link mutation probe | `source_changed_through_link: true` |

After verifying the exact run paths, the operator detached the image and removed only `…/cryptex-m6-p1-apfs-run-mqBeX71B` (image, mount, and spike root). Checkout was untouched. No Actions, commit, push, or merge.

This supports the accepted copy-only M6 projection policy. It is a synthetic local filesystem probe, not a production attachment-backend or performance claim.
