// Boundary and forward-compatibility cases of the validators: the schema says what the Rust reader says (HC A.1, A.1b, A.2, A.7b, A.9, EN-2, EN-6).
import { describe, expect, it } from "vitest";
import {
  kindRegistry,
  validateClientFrame,
  validateDefinition,
  validateRow,
  validateServerFrame,
  validateWith,
  type BulkPackagesResult,
  type PackageRollbackResult,
  type PackageUpgradeResult,
  type RescanResult,
  type UpdateResult,
  type WriteResult,
} from "../src/index.js";

const err = { code: "internal", category: "fault", retryable: false, message: "x" };
const event = (at: number) => ({ type: "event", stream_id: "s", owner: "o", name: "n", priority: "low", source: { kind: "hub", instance: "i" }, payload: {}, at });

describe("integer bounds (HC EN-1, A.1)", () => {
  it.each([
    ["0", true],
    ["18446744073709551615", true],
    ["18446744073709551616", false],
    ["99999999999999999999", false],
    ["00", false],
  ])("request id %s", (id, ok) => {
    expect(validateClientFrame({ type: "cancel", id }).ok).toBe(ok);
  });

  it("a JSON integer is within ±(2^53 − 1)", () => {
    expect(validateServerFrame(event(2 ** 53 - 1)).ok).toBe(true);
    expect(validateServerFrame(event(2 ** 53)).ok).toBe(false);
  });
});

describe("open data sets accept a later version's values (HC EN-6, A.1b)", () => {
  it.each([
    [{ type: "close", reason: { future_reason: {} } }, true],
    [{ type: "close", reason: "future_reason" }, true],
    [{ type: "close", reason: { busy: {} } }, false],
    [{ type: "close", reason: { future_reason: 1 } }, false],
  ])("close %j", (frame, ok) => {
    expect(validateServerFrame(frame).ok).toBe(ok);
  });

  it("a package status", () => {
    const status = (s: unknown) => validateWith("hub-client/kinds/package.schema.json#/$defs/PackageStatus", s).ok;
    expect(status({ future_status: {} })).toBe(true);
    expect(status("live")).toBe(true);
    expect(status({ degraded: { interface: "x@1" } })).toBe(true);
    expect(status({ degraded: {} })).toBe(false);
  });
});

describe("operation results (HC A.7b)", () => {
  it("lists every kind with its states, reasons and result type", () => {
    const kinds = (kindRegistry.operation as unknown as { operation_kinds: Record<string, { states: readonly string[]; result?: string }> }).operation_kinds;
    expect(kinds["hub.update"]?.result).toBe("UpdateResult");
    expect(kinds["spawn"]?.states).toEqual(["starting", "installed", "failed", "cancelled"]);
    expect(Object.keys(kinds)).toContain("spawn_target.rescan_packages");
  });

  it("types the results", () => {
    const update: UpdateResult = { version: "1.0.0" };
    const bulk: BulkPackagesResult = { packages: [{ package: "p", state: "failed", reason: "r" }] };
    const upgrade: PackageUpgradeResult = { package: "p", version: "1", resolved_commit: "c" };
    const rollback: PackageRollbackResult = { package: "p", ref: "r", resolved_commit: "c" };
    const rescan: RescanResult = { found: ["a@t"], removed: [] };
    for (const [name, value] of [["UpdateResult", update], ["BulkPackagesResult", bulk], ["PackageUpgradeResult", upgrade], ["PackageRollbackResult", rollback], ["RescanResult", rescan]] as const) {
      expect(validateWith(`hub-client/kinds/operation.schema.json#/$defs/${name}`, value).ok, name).toBe(true);
    }
    expect(validateWith("hub-client/kinds/operation.schema.json#/$defs/RescanResult", { found: [] }).ok).toBe(false);
    // EN-6: an operation of an unknown kind is a valid row.
    expect(validateRow("operation", { id: "o", kind: "from_a_later_version", state: "from_a_later_state", terminal: false, result: { x: 1 } }).ok).toBe(true);
  });
});

describe("write result and open_asset arguments (HC A.9, A2 A.9b)", () => {
  const writeResult = (v: unknown) => validateWith("hub-client/kinds/session.schema.json#/$defs/WriteResult", v).ok;
  it("write results carry the counts of their state", () => {
    const ok: WriteResult[] = [
      { state: "written", accepted_bytes: 3, written_bytes: 3 },
      { state: "partial", accepted_bytes: 3, written_bytes: 1 },
      { state: "unknown", max_written_bytes: 3 },
    ];
    for (const v of ok) expect(writeResult(v)).toBe(true);
    for (const v of [{ state: "written" }, { state: "partial", written_bytes: 1 }, { state: "unknown" }, { state: "future" }]) expect(writeResult(v)).toBe(false);
  });

  it("open_asset takes exactly one selector", () => {
    const args = (v: unknown) => validateWith("hub-client/kinds/package.schema.json#/$defs/OpenAssetArgs", v).ok;
    expect(args({ asset_id: "a" })).toBe(true);
    expect(args({ name: "n" })).toBe(true);
    expect(args({ asset_id: "a", name: "n" })).toBe(false);
    expect(args({})).toBe(false);
  });
});

describe("a res has exactly one outcome (HC EN-2, EN-3)", () => {
  it.each([
    [{ type: "res", id: "1", ok: {} }, true],
    [{ type: "res", id: "1", err }, true],
    [{ type: "res", id: "1", ok: {}, err }, false],
    [{ type: "res", id: "1" }, false],
    [{ type: "res", id: "1", ok: null }, false],
  ])("%j", (frame, ok) => {
    expect(validateServerFrame(frame).ok).toBe(ok);
    expect(validateDefinition("Res", { id: frame.id, ...("ok" in frame ? { ok: frame.ok } : {}), ...("err" in frame ? { err: frame.err } : {}) }).ok).toBe(ok);
  });
});

describe("review round 2", () => {
  const row = (kind: string, state: unknown, result?: unknown) => ({ id: "o", kind, state, terminal: false, ...(result === undefined ? {} : { result }) });

  it("an operation row applies the result type of its kind (HC A.7b)", () => {
    expect(validateRow("operation", row("hub.update", "succeeded", { version: "1" })).ok).toBe(true);
    expect(validateRow("operation", row("hub.update", "succeeded", { ver: "1" })).ok).toBe(false);
    expect(validateRow("operation", row("spawn_target.rescan_packages", "succeeded", { found: [] })).ok).toBe(false);
    expect(validateRow("operation", row("spawn", "installed", { hub_id: "h", session_id: "s" })).ok).toBe(true);
    // EN-6: an unknown kind keeps an opaque result, and a kind without a result type keeps whatever it carries.
    expect(validateRow("operation", row("from_a_later_version", "succeeded", { ver: "1" })).ok).toBe(true);
    expect(validateRow("operation", row("hub.shutdown", "succeeded", { anything: 1 })).ok).toBe(true);
  });

  it("an unknown operation state with data is valid, a known one needs its reason (EN-6, A.1b)", () => {
    expect(validateRow("operation", row("future", { future_state: {} })).ok).toBe(true);
    expect(validateRow("operation", row("future", { future_state: { progress: 1 } })).ok).toBe(true);
    expect(validateRow("operation", row("future", { future_state: 1 })).ok).toBe(false);
    expect(validateRow("operation", row("future", { failed: {} })).ok).toBe(false);
    expect(validateRow("operation", row("future", { failed: { reason: "r" } })).ok).toBe(true);
  });

  it("narrow integer types keep their range (A.1 and the Rust type)", () => {
    const session = {
      id: "s", hub_id: "h", label: "l", state: "running", lifecycle_class: "current", exit: {}, size: { rows: 24, cols: 80 },
      session_type: { id: "t", source: "device" }, traits: [], restartable: true, activity: "idle", presentation: "default", metadata: {},
    };
    expect(validateRow("session", { ...session, size: { rows: 4294967295, cols: 80 } }).ok).toBe(true);
    expect(validateRow("session", { ...session, size: { rows: 4294967296, cols: 80 } }).ok).toBe(false);
    expect(validateRow("session", { ...session, port: 65535 }).ok).toBe(true);
    expect(validateRow("session", { ...session, port: 65536 }).ok).toBe(false);
    const welcome = (version: number) => ({
      type: "welcome", hub_id: "h", incarnation: "i", version, features: [],
      limits: { max_frame_bytes: 1, max_chunk_bytes: 1, max_outstanding: 1, max_watches: 32, route_connect_seconds: 1, idempotency_window_seconds: 1, cursor_idle_seconds: 1, operation_retention_seconds: 1, max_paste_bytes: 1, max_file_bytes: 1, query_client_deadline_ms: 1 },
    });
    expect(validateServerFrame(welcome(65535)).ok).toBe(true);
    expect(validateServerFrame(welcome(65536)).ok).toBe(false);
  });
});
