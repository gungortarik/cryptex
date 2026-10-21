# M5 Capture + Timeline — preparation plan

**Status:** PREPARATION ONLY; no M5 production implementation in this change.
**Authority:** Master Spec and `docs/milestones/M5_CAPTURE_TIMELINE.md`.
**Branch:** `codex/m5-capture-timeline`.
**Verified base:** normal M4 PR #10 merge `a6c32bd30f883e546e2eb976787a59057fae211b` on `origin/m0/foundation`.
M4 is COMPLETE within scope; its corrected required CI PASS and delegated review
are recorded under `evidence/m4/`. Production v2 activation remains CLOSED.

## Accepted scope

- Durable offline Quick Capture Inbox without a provider or LLM dependency.
- Raw capture becomes durable canonical memory before optional processing.
- Timeline/history/source reconstruction from accepted canonical events.
- All canonical writes and content retrieval remain daemon-controlled.

Existing v1 `memory.note` is the available durable raw-text event. Its payload
remains `{text}`; no Inbox/source/extraction fields or new event types may be
silently added to strict v1. Production structured v2 writing remains disabled.
Optional extraction proposals remain local and must never be auto-published
as confirmed canonical knowledge. No provider integration is required for M5.

## Reusable implementation

| Foundation | M5 use |
|---|---|
| `canonical_writer::append_memory_note` | Daemon-only signed v1 append, sequence allocation, synced frame, chain/tip/seal handling |
| Brain and local DeviceId enrollment | Explicit setup and OS SecureStore-bound signing identity; never silently replace or infer peer trust |
| M3 projection and catch-up | Derived note rows, FTS5, exact timestamp comparison and reconstruction |
| M4 verified materialization and RetrievalSafetyState | Complete-inventory verification, pending/unsupported holds and safe retrieval |
| Authenticated bounded Desktop IPC | Reuse peer identity and framing; a separate explicit capture operation needs M5 authorization and validation |
| Existing Desktop shell | Minimal capture/timeline surfaces; broad product navigation and final screen design remain M13 |

M4's IPC operation is read-only for its own scope. M5 may add daemon-owned
capture under the accepted single-writer architecture; the renderer still gets
no arbitrary filesystem/shell access, no private keys, and no direct SQLite or
canonical path access. Caller-supplied trust/device IDs are never authoritative.

## Reviewable slice order

1. **Capture service contract and daemon foundation.** Review current append
   failure semantics before adding a service. Specify request bounds, explicit
   identity requirements, classification handling and durable acknowledgement.
   Reuse v1 note limits and canonical writer; no new canonical schema.
2. **Retry and restart safety.** Distinguish rejection-before-commit,
   indeterminate outcome and committed capture with indexing pending. Review a
   safe request-replay/receipt design before wiring clients. Do not deduplicate
   intentional identical notes by text. Preserve unsent input when an unsafe
   chain or unavailable daemon blocks publication.
3. **Bounded Desktop capture IPC and minimal Inbox UI.** Reuse the actual
   authenticated Desktop peer boundary. Keep input until a durable outcome is
   known; never claim saved because a SQLite insert or AI request succeeded.
   Apply existing classification policy and render untrusted text safely.
4. **Derived catch-up and restart reconstruction.** Refresh/rebuild M3/M4
   derived state after canonical commit. Indexing failure must not erase a
   durable capture or falsely instruct a blind duplicate retry. Verify restart
   discovery of committed notes after loss of a response or derived database.
5. **Guarded timeline/history/sources.** Add bounded/paginated daemon retrieval
   over verified canonical-derived state. Timeline, detail and source snippets
   must honor the same Brain-wide holds as search. Use exact UTC instants for
   display ordering with deterministic ties, never as security authority.
6. **Cross-platform and DoD closeout.** Synthetic Golden Brain fixtures;
   focused tests/format/strict lint per behavior slice, Windows/macOS native
   parity for changed behavior, security/UI review and one final reviewed CI
   candidate. No speculative Actions runs during preparation.

## Contracts to settle before the affected slice

These are implementation review items, not new accepted architecture:

- Durable acknowledgement and lost-response replay: current append can fail
  after a frame is durable (for example tip/seal follow-up failure). Specify
  how the caller learns committed vs uncertain outcome before exposing Save.
- Capture transport size: the existing search IPC bounds may be smaller than
  the v1 512 KiB text ceiling. Review explicit bounded capture framing and
  canonical serialized-size limits, without widening all IPC indiscriminately.
- Classification defaults and handling of restricted capture must follow
  authoritative policy. Do not invent a default or AI disclosure policy.
- Initial source display is limited to existing canonical provenance (event,
  Brain, device, sequence, timestamp, classification, signing/hash identity).
  Rich import origins or attachments require their own accepted contracts;
  do not put hidden structured data into note text.
- Review an app-data-only draft/receipt path and failure handling if required;
  it must not become a second canonical store or a recovery prerequisite.
- Define bounded timeline cursor/order/filter and stale-data/hold behavior.
  Arbitrary filesystem paths must never enter UI requests.

If a necessary choice conflicts with accepted schema, crypto, identity,
privacy or storage decisions, stop that implementation and report the precise
conflict. Routine API design inside accepted scope does not automatically
require another architectural decision.

## Verification matrix

| Scenario | Required result |
|---|---|
| Offline / no provider credentials / unavailable LLM | Raw note capture and history work without a network dependency |
| Unicode, LF, empty/invalid/oversized input | Valid raw text preserved; invalid bounds rejected explicitly before unsafe writes |
| Concurrent captures and repeated requests | Valid ordered device chain; replay behavior defined; no content-based deletion of intentional duplicates |
| Failure before append | Input retained; no false saved state |
| Durable append followed by response/tip/index failure | No data loss or false pre-commit claim; restart resolves canonical outcome safely |
| Missing/corrupt/stale SQLite | Full derived reconstruction; canonical bytes unchanged |
| Unknown/unsupported/integrity boundary | Safe refusal/Brain-wide retrieval hold; original bytes and user's draft preserved |
| Unauthorized client or malformed request | No canonical write or content disclosure |
| Timeline, detail, sources under hold | No hits/snippets leak through alternate content-returning paths |
| Windows/macOS parity | Equivalent canonical acceptance and display/retrieval semantics at recorded SHAs |

## Scope exclusions

No M6/M7 Obsidian, M8 pairing/replication management, M9 attachments, M10
forget/purge product workflows, M11 backup/recovery product, M12 AI/MCP
integration, M13 broad UI or M14 updater/downgrade productization. No real
personal/Cortex memory. No production v2 writer or activation changes.

## Preparation outcome

The existing milestone authorizes capture/timeline productization. This
preparation does not accept new architecture, dependencies or request schemas,
and does not implement M5. The next authorized preparation task is a read-only
Cursor review of this plan and the contracts above, followed by a concrete
Slice 1 proposal. Production implementation needs explicit authorization.
