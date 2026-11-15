import { test } from "node:test";
import assert from "node:assert/strict";
import { parseNativeProfileStatus as parse, nativeProfileMessage as message } from "./native-profile-status.mjs";
const observed = { schemaVersion: 1, protection: "local_only", incoming: "published", transportObserved: true, remoteDurabilityVerified: false };
test("local publication and a live transport never present verified remote durability", () => {
  assert.match(message(parse(observed)), /has not been verified/);
  for (const change of [{protection:"protected"}, {remoteDurabilityVerified:true}, {incoming:"verified_replica"}, {schemaVersion:2}, {extra:true}]) {
    assert.equal(parse({...observed, ...change}), null);
  }
});
test("held and unavailable observations keep attention explicit; stale success is not a fallback", () => {
  assert.equal(parse({...observed, incoming:"held"}), null);
  assert.match(message(parse({...observed, incoming:"held", protection:"attention"})), /changes are paused/);
  assert.match(message(null), /unavailable/);
  assert.equal(parse(null), null);
  assert.equal(parse([]), null);
});
