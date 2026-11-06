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
