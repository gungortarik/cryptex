# M15 — Cortex Fresh Re-ingestion

**Status:** ACCEPTED  
**Baseline:** 2026-09-28

## Purpose

M15 performs controlled fresh re-ingestion from legacy Cortex into a new Cryptex Brain without importing Cortex runtime architecture.

## Authority

Follow `AGENTS.md`, `docs/00_MASTER_SPEC.md`, `docs/16_CORTEX_FRESH_REINGESTION.md`, `docs/17_TEST_AND_FAILURE_MATRIX.md`, `docs/21_DEFINITION_OF_DONE.md` and this contract.

## Scope

M15 includes:

- read-only inventory
- category extraction/import
- dedup/conflict review
- provenance
- acceptance/cutover

## Required invariants

- Cortex remains a read-only legacy source during transition.
- Legacy runtime/database structure does not become Cryptex architecture authority.
- Imported content enters through supported Cryptex canonical paths.
- Provenance is preserved.
- Dedup/conflict review is explicit.
- Real personal data is introduced only after required backup/recovery confidence exists.

## Verification

### First step: read-only source inventory

Run `cargo run --locked -p cryptex-cortex-inventory -- SOURCE_DIRECTORY` to inspect a selected legacy directory. Its JSON report on stdout is intended for local review and may contain private filenames. Regular files carry a relative path, byte length and a lowercased filename extension when present. Directory entries include empty directories. Extensions are hints for later extraction review. The tool reads filesystem metadata only. Content readability, classification and content identity must be checked during extraction.

Exit status 0 indicates a complete listing. Status 2 signals findings such as inaccessible directories, indirect or special entries, or a name that cannot be represented in UTF-8. A finding uses the containing directory when the item's name is unavailable. An empty path refers to the selected root. The traversal reports a depth limit beyond 64 nested directories. Above 100,000 visited entries it returns a root-level limit finding and discards the partial list. Use a stable source snapshot when comparing inventories across runs.

### First category mapping for synthetic sources

An explicitly selected free-text Markdown note directly inside the synthetic export's `Notes/` directory maps to the existing Cryptex v1 `memory.note` payload. One source file contributes its complete UTF-8 text to one note. Markdown headings, front matter, links, line endings and whitespace within that text are preserved. The source convention comes from the inventory fixture and awaits comparison with a reviewed real export.

Use `cargo run --locked -p cryptex-cortex-inventory -- --preview-note SOURCE_DIRECTORY Notes/example.md`. Supply the exact relative spelling from the inventory. The extension accepts any ASCII case. The preview validates the text with the production `MemoryNotePayload` constructor and returns the source path, SHA-256 of the exact text bytes, byte count and target event type for local review. Classification requires a separate owner decision. Source text cannot grant publication or select a classification. The preview omits the body and performs no canonical write.

Unmapped paths, unreadable or indirect items, multiply linked files, whitespace-only text, invalid UTF-8, embedded NUL and text above the event byte limit return a review finding (exit 2). Use a stable source snapshot. A later import must bind the reviewed content and classification to the supported daemon write path.

### Milestone verification

The `m15-disposable-import` Cargo feature adds `cryptexd cortex-import-disposable SOURCE_DIRECTORY` for the first synthetic note trial. It allocates a fresh temporary Brain and uses an in-memory development key store. Each local stdin line supplies one explicit decision with `capture_id`, `source_path`, `source_sha256` from the preview, and a Cryptex `classification`. The daemon rereads the selected item once, compares the digest and submits those same bytes to its normal capture service.

One profile retains one approval for this trial. The source provenance and classification in that approval link to the daemon capture receipt by capture ID. Retrying the same decision resolves the same event. Reusing it with a different source path or classification is refused. At EOF the daemon reopens the packs, verifies the signature and chain, and reports the event identity, classification and digest of the stored text. The temporary profile is removed on normal exit. This trial covers on-disk readback in one daemon session. Real-profile import and durable cutover remain later work.

M15 must verify relevant inventory/import failure behavior, provenance, dedup/conflict review, interrupted import/retry behavior and acceptance/cutover against the real target Cryptex model.

The restart rehearsal uses two separate invocations of the feature-gated daemon. The first creates a trial profile and a fresh signing key in a dedicated Windows Credential Manager or macOS Keychain namespace. After it exits, the second opens the same temporary profile, loads the enrolled key, checks the saved approval and resolves the prior capture receipt before retrying. The harness compares both processes' identities, receipts, verified notes and canonical pack bytes. It removes the trial credential and profile at teardown. This exercises an orderly process exit between commit and retry.

### Comparing synthetic notes before capture

Run `cryptex-cortex-inventory --review-notes SOURCE_DIRECTORY` against a stable snapshot. The read-only JSON report lists the relative path and exact content SHA-256 for each mapped Markdown file directly under `Notes/`. Identical byte digests form duplicate groups. Files with different digests and the same leading level-one Markdown title form possible conflict groups. Title comparison lowercases text and collapses whitespace. A leading front matter block is skipped. The report omits titles and bodies. This heuristic can miss contradictions expressed under different titles.

Exit code 0 means the scan finished with no candidates. Exit code 2 means there are groups to review or files that could not be checked. Inspect the listed originals to decide which file to keep. To capture a member of a group in the disposable daemon, include the current report's `review_sha256` alongside that file's path, digest, classification and capture ID in the approval. This selects one file for this trial. The daemon recomputes the review before writing. An outdated choice returns the current report. An incomplete scan reports the affected paths and issues and withholds the review digest.

### Reviewing the origin of multiple notes

On Windows and macOS, `cryptexd cortex-import-disposable SOURCE_DIRECTORY --batch-create TRIAL_UUID` accepts separate reviewed approvals in a fresh temporary profile. For each committed note the daemon signs a local source receipt with its enrolled event-signing key. The receipt binds the Brain, device, event ID and event content hash to the approved relative source path, original byte digest, classification, capture ID and optional review digest. A repeated approval returns the same note. A second capture ID for a previously selected path or byte digest is refused within the batch.

In the same temporary directory and login account, run `cryptexd cortex-import-disposable --provenance TRIAL_UUID` after the original export is disconnected. The report verifies the signed event chain, every existing source receipt signature, each matched note's content and classification, and the number of receipts. It emits local source metadata only after the whole batch verifies.

Before capture the daemon durably signs the reviewed decision for the current Brain and device. It clears this pending decision after publishing the matching source receipt. If the process exits between those steps, the next provenance check resolves the original capture ID and verifies the note against the signed decision. It can then publish the missing receipt without reopening the export or appending an event. An existing matching receipt survives a second interruption during cleanup. An unverifiable commit, an unexplained missing receipt, or altered, misplaced or extra evidence blocks the report. This recovery covers committed notes in a disposable batch. Use `--remove-trial-key TRIAL_UUID` with the same disposable command for credential cleanup when the trial is finished.

### Comparing a selection before owner acceptance

Keep the reviewed approvals separately from the import results. For the disposable Brain, run `cryptexd cortex-import-disposable --compare-selection TRIAL_UUID` and supply one UTF-8 JSON document on standard input (up to 8 MiB). Its `schema` is `cryptex.cortex.selection.v1`, its `brain_id` is the trial UUID, and its `selected` array holds the original approvals sent to the batch importer, including their capture IDs and any review digests. The original export may be offline. The caller supplies the intended selection; building it from the output receipts would conceal missing notes.

The daemon verifies the whole batch using the same checks and interrupted-receipt recovery as `--provenance`. The comparison lists exact matches, selected approvals with no corresponding capture ID, and verified receipts outside the selection. For an existing capture ID with different metadata, `changed` shows the selected and verified values together. The report contains local source metadata and event IDs. It omits note bodies. Invalid or ambiguous selections and unverifiable evidence produce a diagnostic and no report.

Exit code zero means a comparison report is available. Check `ready_for_owner_review` before presenting the batch for final approval. It is true only for a nonempty selection whose receipts match exactly. Running the comparison leaves final acceptance pending.

### Accepting or cancelling a disposable batch

First run `--compare-selection` with the intended selection. Inspect the matched notes, their classifications and the receipt identities. A nonempty exact match returns `review_sha256` for that comparison. While the decision is pending, `batch_decision` is null. Keep this digest from the review you actually inspected.

Choose one of the following commands and supply the same selection JSON on standard input. Replace `TRIAL_UUID` and `REVIEW_SHA256` with the values from your review.

```text
cryptexd cortex-import-disposable --accept-selection TRIAL_UUID REVIEW_SHA256
cryptexd cortex-import-disposable --cancel-selection TRIAL_UUID REVIEW_SHA256
```

Under the trial's instance lock, the daemon reopens the signed history, verifies the receipts and recomputes the comparison. Both commands require the reviewed digest and a current exact match. The daemon durably signs one decision for that disposable Brain and returns it in `batch_decision`. Running the comparison again displays the saved decision, including the digest it applies to. Repeating the same choice is idempotent. A conflicting choice requires a new trial. Cancellation ends the trial's approval process and preserves its staged notes and the legacy export. This rehearsal does not activate a personal Brain.

### Opening the accepted disposable Brain

Run `cryptexd cortex-import-disposable --open-accepted TRIAL_UUID` with the same selection JSON on standard input. This reopens the enrolled trial identity, verifies the signed events and source receipts, and requires the signed acceptance of that exact batch. The daemon rebuilds the local projection from canonical packs, reads the complete paginated timeline through the guarded product reader, and compares each returned event identity, content hash, classification and text digest with its receipt. The result contains the Brain ID and verified note count. This verifies local usability after restarting or losing the search index with the original export offline.

## Completion gate

M15 is complete only when scope items and relevant recovery/privacy/failure tests pass, required CI is green, Definition of Done is satisfied and human review is complete.

Only then may M16 begin.
