/* eslint-disable */
/**
 * Generated from cryptex-event-v3.schema.json. Do not edit by hand.
 * Regenerate: cargo run -p cryptex-events --bin cryptex-events-schema-gen
 *             pnpm --filter @cryptex/schemas generate:ts
 * Unknown-field rejection comes from schema additionalProperties:false.
 */

export type CryptexSecurityEventV3 =
  | {
      brain_id: string;
      classification: "internal";
      /**
       * 64-char lowercase hex content hash.
       */
      content_hash: string;
      created_at: string;
      device_id: string;
      device_seq: string;
      event_id: string;
      event_type: "security.peer_approved";
      payload: {
        transaction_id: string;
        /**
         * @minItems 4
         * @maxItems 4
         */
        transcript: [
          {
            brain_id: string;
            message_type: "offer";
            payload: {
              initiator_nonce: string;
            };
            peer_device_id: null;
            peer_public_key: null;
            peer_signing_key_id: null;
            peer_transport_device_id: null;
            previous_hash: "0000000000000000000000000000000000000000000000000000000000000000";
            protocol_version: 1;
            sender_device_id: string;
            sender_public_key: string;
            sender_signing_key_id: string;
            sender_transport_device_id: string;
            signature: string;
            transaction_id: string;
          },
          {
            brain_id: string;
            message_type: "proof";
            payload: {
              offer_hash: string;
              responder_nonce: string;
            };
            peer_device_id: string;
            peer_public_key: string;
            peer_signing_key_id: string;
            peer_transport_device_id: string;
            previous_hash: string;
            protocol_version: 1;
            sender_device_id: string;
            sender_public_key: string;
            sender_signing_key_id: string;
            sender_transport_device_id: string;
            signature: string;
            transaction_id: string;
          },
          {
            brain_id: string;
            message_type: "approval";
            payload: {
              agreement_hash: string;
              role: "initiator";
            };
            peer_device_id: string;
            peer_public_key: string;
            peer_signing_key_id: string;
            peer_transport_device_id: string;
            previous_hash: string;
            protocol_version: 1;
            sender_device_id: string;
            sender_public_key: string;
            sender_signing_key_id: string;
            sender_transport_device_id: string;
            signature: string;
            transaction_id: string;
          },
          {
            brain_id: string;
            message_type: "approval";
            payload: {
              agreement_hash: string;
              role: "responder";
            };
            peer_device_id: string;
            peer_public_key: string;
            peer_signing_key_id: string;
            peer_transport_device_id: string;
            previous_hash: string;
            protocol_version: 1;
            sender_device_id: string;
            sender_public_key: string;
            sender_signing_key_id: string;
            sender_transport_device_id: string;
            signature: string;
            transaction_id: string;
          }
        ];
      };
      /**
       * 64-char lowercase hex content hash.
       */
      prev_content_hash: string;
      schema_version: 3;
      /**
       * 128-char lowercase hex Ed25519 signature.
       */
      signature: string;
      /**
       * 32-char lowercase hex signing key id.
       */
      signing_key_id: string;
    }
  | {
      brain_id: string;
      classification: "internal";
      /**
       * 64-char lowercase hex content hash.
       */
      content_hash: string;
      created_at: string;
      device_id: string;
      device_seq: string;
      event_id: string;
      event_type: "security.device_restricted";
      payload: {
        cutoff_content_hash: string;
        cutoff_seq: string;
        reason: "retired" | "compromised";
        target_device_id: string;
        target_public_key: string;
        /**
         * 32-char lowercase hex signing key id.
         */
        target_signing_key_id: string;
        transaction_id: string;
      };
      /**
       * 64-char lowercase hex content hash.
       */
      prev_content_hash: string;
      schema_version: 3;
      /**
       * 128-char lowercase hex Ed25519 signature.
       */
      signature: string;
      /**
       * 32-char lowercase hex signing key id.
       */
      signing_key_id: string;
    }
  | {
      brain_id: string;
      classification: "internal";
      /**
       * 64-char lowercase hex content hash.
       */
      content_hash: string;
      created_at: string;
      device_id: string;
      device_seq: string;
      event_id: string;
      event_type: "security.device_self_retired";
      payload: {
        last_authorized_content_hash: string;
        last_authorized_seq: string;
        transaction_id: string;
      };
      /**
       * 64-char lowercase hex content hash.
       */
      prev_content_hash: string;
      schema_version: 3;
      /**
       * 128-char lowercase hex Ed25519 signature.
       */
      signature: string;
      /**
       * 32-char lowercase hex signing key id.
       */
      signing_key_id: string;
    };
