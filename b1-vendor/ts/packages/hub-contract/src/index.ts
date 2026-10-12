// @trybotster/hub-contract: the Hub→client contract (frozen/current, HC parity45 with A1 to A4).
// The types and schemas are generated from the Rust crate botster-hub-contract (design section 8). Nothing here is written by hand
// except transport.ts and validate.ts.
export * from "./generated/hub.js";
export { inventory } from "./generated/inventory.js";
export { errorRegistry, kindRegistry } from "./generated/registry.js";
export * from "./transport.js";
export * from "./validate.js";

/** The protocol name of the first client frame (HC HS-1). */
export const PROTOCOL = "botster-hub";
