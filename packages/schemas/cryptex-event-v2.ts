/* eslint-disable */
/**
 * Generated from cryptex-event-v2.schema.json. Do not edit by hand.
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
export type KnowledgeAssertionEventType = "knowledge.assertion";
export type KnowledgeAssertionPayload =
  | {
      entity_id: string;
      entity_kind: EntityKind;
      kind: "entity.declare";
      label: string;
      publication_authorization: PublicationAuthorization;
    }
  | {
      alias: string;
      entity_id: string;
      kind: "entity.alias";
      publication_authorization: PublicationAuthorization;
    }
  | {
      confidence: Confidence;
      kind: "fact.assert";
      predicate: string;
      publication_authorization: PublicationAuthorization;
      subject_entity_id: string;
      value: FactValue;
    }
  | {
      confidence: Confidence;
      kind: "relation.assert";
      object_entity_id: string;
      predicate: string;
      publication_authorization: PublicationAuthorization;
      subject_entity_id: string;
    }
  | {
      kind: "assertion.correct";
      publication_authorization: PublicationAuthorization;
      replacement: RequiredNullable;
      target_event_id: string;
    }
  | {
      /**
       * @minItems 2
       * @maxItems 256
       */
      entity_ids: [string, string, ...string[]];
      kind: "entity.merge";
      publication_authorization: PublicationAuthorization;
    }
  | {
      kind: "entity.merge_revoke";
      publication_authorization: PublicationAuthorization;
      target_merge_event_id: string;
    }
  | {
      /**
       * @minItems 2
       * @maxItems 4096
       */
      decision_event_ids: [string, string, ...string[]];
      kind: "assertion.resolve_conflict";
      outcome: ConflictOutcome;
      publication_authorization: PublicationAuthorization;
      target_event_id: string;
    };
export type EntityKind = "person" | "project" | "course" | "device" | "organization" | "concept" | "other";
export type AuthorizationBasis = "direct_user" | "user_approved_ai_capture";
export type Confidence = "confirmed" | "inferred" | "uncertain";
/**
 * Fact values are tagged, exact-shape unions; no floating-point values exist.
 */
export type FactValue =
  | {
      kind: "text";
      value: string;
    }
  | {
      kind: "decimal";
      value: string;
    }
  | {
      kind: "boolean";
      value: boolean;
    }
  | {
      kind: "date";
      value: string;
    }
  | {
      kind: "entity_ref";
      value: string;
    };
/**
 * Required JSON property whose value is either `null` or a typed assertion.
 * Unlike `Option<T>`, this wrapper remains a required struct property.
 */
export type RequiredNullable = SemanticAssertion | null;
/**
 * Semantic replacement data for correction and conflict-resolution events.
 * `kind` distinguishes the assertion's semantic shape, not a new event type.
 */
export type SemanticAssertion =
  | {
      confidence: Confidence;
      kind: "fact";
      predicate: string;
      subject_entity_id: string;
      value: FactValue;
    }
  | {
      confidence: Confidence;
      kind: "relation";
      object_entity_id: string;
      predicate: string;
      subject_entity_id: string;
    };
export type ConflictOutcome =
  | {
      decision_event_id: string;
      kind: "select";
    }
  | {
      kind: "replace";
      value: SemanticAssertion;
    }
  | {
      kind: "withdraw";
    };
/**
 * 128-char lowercase hex Ed25519 signature.
 */
export type SignatureHex = string;
/**
 * 32-char lowercase hex signing key id.
 */
export type SigningKeyId = string;

export interface CryptexKnowledgeAssertionEventV2 {
  brain_id: string;
  classification: Classification;
  content_hash: ContentHashHex;
  created_at: string;
  device_id: string;
  device_seq: DeviceSeq;
  event_id: string;
  event_type: KnowledgeAssertionEventType;
  payload: KnowledgeAssertionPayload;
  payload_schema_version: number;
  prev_content_hash: ContentHashHex;
  schema_version: number;
  signature: SignatureHex;
  signing_key_id: SigningKeyId;
}
export interface PublicationAuthorization {
  basis: AuthorizationBasis;
  receipt_id: string;
}
