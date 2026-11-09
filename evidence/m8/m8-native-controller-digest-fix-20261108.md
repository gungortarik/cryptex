# Native controller request: digest representation mismatch

Date: 2026-11-08. Live transfer/publication remains unverified.

Returned request: `C:\Users\Tarik\Downloads\DENETLEYICI-ISTEK-d1e5c535-524ee335fd33.zip`.
SHA-256: `cc563847b39038004fceceef7507c87f75fdd41d717995e552400990b285fb9f`.
Exact two entries: request.json and one payload segment; total segment bytes 6557.
Session: `d1e5c535ae0e459fb699be8f7324c149`.
Receiver: `C6JGWXP-VFAK2BI-POEO7C5-RZWS3AD-MQTNII4-C5VDJT6-YAV7TQR-S7V52Q6`.
Segment SHA-256: `FBA9BED56257FE48C0C8C1B54159F264BF64936D51FAC50D54514E335EC49C4B`.

The existing trusted import probe emits uppercase hex. The new host validator
required lowercase, so ValidateOnly failed `segment_manifest_invalid`. The new
receiver test helper likewise returned lowercase and compared it exactly against
the uppercase descriptor. Correcting the host alone would leave the current VM
test waiting until timeout; no host native sender was started for this request.

The cfg(test) digest helper now matches the probe's uppercase format. Host relay
metadata accepts exactly 64 hex characters in either case and compares all digest
characters case-insensitively; signed event bytes and all crypto checks are unchanged.

Validation:
- Four focused Rust tests pass, zero failed; one live test remains explicitly ignored.
  New regression uses real prepare() descriptors and checks altered fixture bytes fail.
- Windows PowerShell runner checks pass for lowercase/uppercase hash text, altered
  digest refusal, ZIP safety/size, endpoint binding and result summary semantics.
- Returned request passes corrected ValidateOnly, native_launched=false.
- Strict all-target daemon Clippy, formatting and diff checks pass.
- New bounded cancellation helper tested against isolated fake LOCALAPPDATA roots:
  matching waiting run receives cancel flag and observed completion; wrong session
  or non-waiting checkpoint is refused without cancellation. No real VM run touched.

Replacement package embeds the exact source candidate and a launcher that first
cooperatively cancels run `524ee335fd33` under its known synthetic root/session.
The old runner's failed cancellation receipt is expected and is not the new result.
No existing owner profile/pairing or security setting is changed. Await a fresh
request from the replacement; retained host relay has never sent a payload.
