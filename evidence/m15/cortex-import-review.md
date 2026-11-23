# M15 disposable Cortex import candidate

Candidate `5e86ef9` covers a synthetic export and a fresh disposable Brain. A free-text Markdown file directly in `Notes/` maps to one v1 memory note. The operator chooses its classification. Other categories remain inventory entries for later review. The personal onboarding gate remains at Private Beta / Release Candidate.

## Acceptance and privacy review

| Check | What the candidate verifies |
| --- | --- |
| Source access | Inventory and preview use bounded traversal and relative names. Note extraction requires an ordinary single-link UTF-8 file. The daemon hashes the exact bytes it submits to capture. |
| Duplicates and conflicts | Exact byte matches and differing notes with the same leading title appear for review. An incomplete scan or stale review prevents a fresh capture. |
| Write authority and retries | Only the daemon writes canonical notes. The disposable command keeps its enrolled signing key under a trial-specific credential label. Retry after commit resolves the original capture ID. |
| Owner decision | The operator supplies a separate selection. Acceptance binds a nonempty exact match to a signed decision. Cancellation persists, and reopening that batch for cutover is denied. |
| Read after acceptance | With the legacy export offline, the daemon rebuilds the derived projection and checks the identity, class and text digest of each result through the guarded local reader. Its report contains the verified count. |
| Encrypted recovery | A signed v3 inventory covers the original source receipts and decision inside the encrypted generation. The independent recovery selection pins the decision digest. Reconstruction checks the signatures, linked note bytes and approved selection using the restored history. |

## Evidence and limits

The guarded reopening tests passed on both platforms at `881ceae`. On Windows, the focused inventory test and strict Clippy checks passed locally on `5e86ef9`. The inventory test exercises both formats, a swapped checkpoint, a pending decision, a signed cancellation, altered decision bytes and removal of receipt evidence after reconstruction. The installed test removes the disposable source and its credentials before opening the manual and scheduled encrypted generations. It compares the restored receipts to the original signed bytes.

- [Guarded reopening on Windows and macOS](https://github.com/gungortarik/cryptex/actions/runs/45115234318)
- [Candidate import tests](https://github.com/gungortarik/cryptex/actions/runs/45116436708)
- [Candidate encrypted backup and installed restore tests](https://github.com/gungortarik/cryptex/actions/runs/45116433964)
- [Candidate cross-platform restore suite](https://github.com/gungortarik/cryptex/actions/runs/45116486934)
- [Candidate general CI](https://github.com/gungortarik/cryptex/actions/runs/45116433856)

The backup checks apply to an accepted batch and the retained recovery selection. If a linked note has subsequently been purged, the original acceptance can no longer validate all its notes and publication of that import profile's backup stops. If **all** local import evidence is lost before a recovery selection is created, the ordinary note history alone cannot identify its Cortex origin. Preserve the separately reviewed selection and verify the encrypted backup before using it for recovery. The scope reviewed here ends at the disposable cutover rehearsal. A future personal cutover requires a reviewed real source mapping, an intact recovery baseline, and separate owner authorization.
