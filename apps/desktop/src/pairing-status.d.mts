export function pairingRejectionStatus(code: string | null): string;
export function completionMessages(text: string): string[];
export function reviewMessages(role: "initiator" | "responder", offer: string, proof: string, initiatorApproval: string): string[];
export function swappedPairingField(text: string, field: "offer" | "identity"): string;
