// A consumer smoke test with plain `node --test`, no vitest (Web's test shape): the built package loads under Node through its `exports` map
// and its validators work. Run after `npm run build`.
import assert from "node:assert/strict";
import test from "node:test";
import * as contract from "@trybotster/hub-contract";
import * as route from "@trybotster/hub-contract/route";
import * as ui from "@trybotster/hub-contract/ui";

test("the package loads and exports the runtime surface", () => {
  assert.equal(contract.PROTOCOL, "botster-hub");
  assert.equal(typeof contract.validateClientFrame, "function");
  assert.ok(contract.inventory.ErrorCode.includes("stale_descriptor"));
  assert.equal(Object.keys(contract.kindRegistry).length, 16);
  assert.ok(route.routeInventory);
  assert.ok(ui);
});

test("a request frame is validated against the envelope schema (HC EN-1, A4 EN-1a)", () => {
  assert.equal(contract.validateClientFrame({ type: "req", id: "1", verb: "invoke", kind: "session", target: "s", action: "stop" }).ok, true);
  assert.equal(contract.validateClientFrame({ type: "req", id: "1", verb: "invoke", kind: "session", target: "s" }).ok, false);
});
