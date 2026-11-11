const completionCodes = new Set([
  "invalid_transcript", "approval_journal_unavailable", "approval_transaction_conflict",
  "approval_anchor_held", "approval_request_rejected", "platform_peer_identity_unavailable",
  "desktop_authentication_required",
]);

export function pairingRejectionStatus(code) {
  const detail = completionCodes.has(code) ? ` (${code})` : "";
  return `Cryptex could not safely record this pairing${detail}. Keep the complete transcript for review.`;
}

export function completionMessages(text) {
  const messages = text.split(/\r?\n/).map(line => line.trim()).filter(Boolean);
  return messages.length === 4 ? messages : [];
}

export function reviewMessages(role, offer, proof, initiatorApproval) {
  const fields = role === "responder" ? [offer, proof, initiatorApproval] : [offer, proof];
  const messages = fields.map(field => field.trim());
  return messages.every(message => message && !/[\r\n]/.test(message)) ? messages : [];
}

// Recognize a common paste mix-up. The daemon still verifies both documents.
export function swappedPairingField(text, field) {
  try {
    const value = JSON.parse(text);
    if (field === "offer" && value?.schema_version === 1 && typeof value.brain_id === "string" && !value.message_type) {
      return "Move this Brain identity to the field below and paste the signed offer here.";
    }
    if (field === "identity" && value?.message_type === "offer") {
      return "Move this signed offer to the field above and paste the public Brain identity here.";
    }
  } catch {
    // While typing or pasting, an incomplete document has no identifiable type.
  }
  return "";
}
