export function normalizeSearchText(value: string): string | null;

export type SearchRequestGate = {
  begin(): number;
  invalidate(): void;
  isCurrent(requestGeneration: number): boolean;
};

export function createSearchRequestGate(): SearchRequestGate;
