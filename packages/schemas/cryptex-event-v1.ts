/* eslint-disable */
/**
 * Generated from cryptex-event-v1.schema.json. Do not edit by hand.
 * Regenerate: cargo run -p cryptex-events --bin cryptex-events-schema-gen
 *             pnpm --filter @cryptex/schemas generate:ts
 * Unknown-field rejection comes from schema additionalProperties:false.
 */

/**
 * Production classification values.
 */
export type Classification = "public" | "internal" | "private" | "restricted";
/**
 * 64-char lowercase hex content hash.
 */
export type ContentHashHex = string;
/**
 * Per-device sequence encoded as a canonical decimal JSON string covering full `u64`.
 */
export type DeviceSeq = string;
/**
 * Production `event_type` values for M2 (fixture-only types are not included).
 */
export type EventType = "memory.note";
/**
 * Type-specific payload object. Production M2 includes only `memory.note`.
 */
export type EventPayload = MemoryNotePayload;
/**
 * 128-char lowercase hex Ed25519 signature.
 */
export type SignatureHex = string;
/**
 * 32-char lowercase hex signing key id.
 */
export type SigningKeyId = string;

/**
 * Accepted on-disk record: signable fields plus integrity fields.
 */
export interface CryptexAcceptedEventV1 {
  brain_id: string;
  classification: Classification;
  content_hash: ContentHashHex;
  created_at: string;
  device_id: string;
  device_seq: DeviceSeq;
  event_id: string;
  event_type: EventType;
  payload: EventPayload;
  prev_content_hash: ContentHashHex;
  schema_version: number;
  signature: SignatureHex;
  signing_key_id: SigningKeyId;
}
/**
 * Payload for `memory.note`.
 */
export interface MemoryNotePayload {
  text: string;
}
