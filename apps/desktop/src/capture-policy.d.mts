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

export function createCaptureSubmissionState(): any;
export function hasUnresolvedSubmission(state: any): boolean;
export function isCaptureBusy(state: any): boolean;
export function evaluateOrdinarySave(state: any, input?: { text?: string; classification?: string }): {
  allowed: boolean;
  code: string;
  message?: string;
  pendingCaptureId?: string;
  canResolve?: boolean;
  receiptMissing?: boolean;
};
export function evaluateDeliberateNewSave(
  state: any,
  input?: { acknowledgeDuplication?: boolean; text?: string; classification?: string },
): {
  allowed: boolean;
  code: string;
  message?: string;
  archivePrevious?: boolean;
  previousCaptureId?: string;
  pendingCaptureId?: string;
};
export function reduceCaptureSubmission(state: any, event: any): any;
export function shouldPromptBeforeNativeQuit(input: {
  draftDirty: boolean;
  hasUnresolved: boolean;
}): boolean;
export const NATIVE_QUIT_WARNING: string;
export function redactedCaptureTransportMessage(code: string): string;
