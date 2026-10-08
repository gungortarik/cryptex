import assert from "node:assert/strict";
import test from "node:test";

import { createSearchRequestGate, normalizeSearchText } from "./search-policy.mjs";

test("normalizes a search phrase and rejects whitespace-only input", () => {
  assert.equal(normalizeSearchText("  project notes  "), "project notes");
  assert.equal(normalizeSearchText("  \t\n "), null);
});

test("query edits invalidate in-flight search responses", () => {
  const gate = createSearchRequestGate();
  const first = gate.begin();

  assert.equal(gate.isCurrent(first), true);
  gate.invalidate();
  assert.equal(gate.isCurrent(first), false);

  const second = gate.begin();
  assert.equal(gate.isCurrent(first), false);
  assert.equal(gate.isCurrent(second), true);
});
