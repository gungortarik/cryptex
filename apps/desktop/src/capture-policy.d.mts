export const CLASSIFICATIONS: readonly string[];
export const MAX_CAPTURE_TEXT_BYTES: number;

export function createCaptureSubmissionGate(): {
  begin(): number;
  invalidate(): void;
  isCurrent(requestGeneration: number): boolean;
};

export function validateCaptureDraft(input: {
  text: string;
  classification: string;
}): string | null;

export function utf8ByteLength(text: string): number;

export function shouldClearDraftAfterCommit(input: {
  pendingSnapshot: { captureId: string; text: string; classification: string } | null;
  liveText: string;
  liveClassification: string;
  responseCaptureId?: string | null;
}): boolean;

export function formatCaptureOutcomeMessage(input?: {
  outcome?: string | null;
  rejectCode?: string | null;
  uncertainReason?: string | null;
  error?: string | null;
}): string;

export function interpretCaptureWire(response: {
  outcome?: string | null;
  rejectCode?: string | null;
  reject_code?: string | null;
  uncertainReason?: string | null;
  uncertain_reason?: string | null;
  error?: string | null;
}): {
  outcome: string | null;
  rejectCode: string | null;
  uncertainReason: string | null;
  isCommitted: boolean;
  isReceiptMissing: boolean;
  isCaptureIdConflict: boolean;
  isRejected: boolean;
  isUncertain: boolean;
  message: string;
};

export function newCaptureId(randomUuid?: () => string): string;
