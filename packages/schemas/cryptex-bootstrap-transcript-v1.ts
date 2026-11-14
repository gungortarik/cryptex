/* eslint-disable */
/**
 * Generated from cryptex-bootstrap-transcript-v1.schema.json. Do not edit by hand.
 * Regenerate: cargo run -p cryptex-events --bin cryptex-events-schema-gen
 *             pnpm --filter @cryptex/schemas generate:ts
 * Unknown-field rejection comes from schema additionalProperties:false.
 */

/**
 * @minItems 4
 * @maxItems 4
 */
export type CryptexBootstrapTranscriptV1 = [
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
