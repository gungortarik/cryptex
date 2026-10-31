export function normalizeSearchText(value) {
  const normalized = value.trim();
  return normalized.length > 0 ? normalized : null;
}

export function createSearchRequestGate() {
  let generation = 0;
  return {
    begin() {
      generation += 1;
      return generation;
    },
    invalidate() {
      generation += 1;
    },
    isCurrent(requestGeneration) {
      return requestGeneration === generation;
    },
  };
}
