# M6 Slice 1 — deterministic v1 note renderer

**Status:** Implemented locally; not committed or pushed. This is one M6 component, not M6 completion.

## Scope

`cryptex-projection::vault::render_vault_note` accepts a semantically valid v1 `memory.note` event and returns a deterministic `notes/<event-id>.md` path plus Markdown. Frontmatter uses JSON-quoted YAML scalar values for provenance and classification. The note body remains unchanged after the frontmatter separator. Malformed event IDs fail before a path is returned.

This pure renderer performs no filesystem writes. Its caller must supply already signature-verified canonical events and enforce the current RetrievalSafetyState. Atomic rebuild/publication, ownership manifest, retrieval gate integration, attachment copying, and Open Vault/Graph are not implemented yet.

## Verification

- `cargo test --offline -p cryptex-projection vault::`: **3/3 PASS**
- `cargo fmt --all -- --check`: PASS after formatting
- `git diff --check`: PASS
- `cargo clippy --offline -p cryptex-projection --all-targets -- -D warnings`: **not run to completion**; Windows Application Control blocked the `cargo-clippy` executable (OS error 4551).

No Actions, commit, push, or merge.
