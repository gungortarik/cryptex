/* eslint-disable */
/**
 * Generated from cryptex-event-v1.schema.json. Do not edit by hand.
 * Regenerate: cargo run -p cryptex-events --bin cryptex-events-schema-gen
 *             pnpm --filter @cryptex/schemas generate:ts
 */

/**
 * Production classification values.
 */
export type Classification = "public" | "internal" | "private" | "restricted";
export type ContentHashHex = string;
/**
 * Canonical decimal string of a u64 device sequence in [1, 2^64-1]
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
export type SignatureHex = string;
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
