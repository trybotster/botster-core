import { describe, expect, it } from "vitest";
import fc from "fast-check";
import {
  errorRegistry,
  inventory,
  kindRegistry,
  validateClientFrame,
  validateDescribe,
  validateDefinition,
  validateRow,
  validateServerFrame,
  type ClientFrame,
  type Req,
  type ServerFrame,
} from "../src/index.js";
import { schemas } from "../src/generated/schemas.js";
import { arbitraryOf } from "./walker.js";

const ok = { ok: true };
const envelope = schemas["hub-client/envelope.schema.json"] as Record<string, unknown>;
const defs = envelope.$defs as Record<string, Record<string, unknown>>;

describe("the envelope schema (HC EN-1, EN-2, A4 EN-1a)", () => {
  it("accepts the client frames of the clauses", () => {
    const hello: ClientFrame = {
      type: "hello",
      protocol: "botster-hub",
      versions: { min: 1, max: 1 },
      client: { name: "web", version: "0.1.0" },
      features: [],
      terminal_formats: ["ghostsnp/2+raw"],
    };
    expect(validateClientFrame(hello)).toEqual(ok);
    expect(validateClientFrame({ type: "cancel", id: "7" })).toEqual(ok);
    expect(validateClientFrame({ type: "req", id: "1", verb: "invoke", kind: "session", target: "s", action: "stop", args: {} })).toEqual(ok);
  });

  it("requires `action` exactly when the verb is `invoke` (A4 EN-1a, R-8)", () => {
    const base = { type: "req", id: "1", kind: "session", target: "s" } as const;
    expect(validateClientFrame({ ...base, verb: "invoke" }).ok).toBe(false);
    expect(validateClientFrame({ ...base, verb: "get", action: "stop" }).ok).toBe(false);
    expect(validateClientFrame({ ...base, verb: "get" })).toEqual(ok);
    // The typed frame has the field, so a client can set it.
    const req: Req = { id: "1", verb: "invoke", kind: "session", action: "stop" };
    expect(req.action).toBe("stop");
  });

  it("refuses an id that is not a decimal u64 string, an unknown frame type and a null optional member", () => {
    expect(validateClientFrame({ type: "cancel", id: 7 }).ok).toBe(false);
    expect(validateClientFrame({ type: "cancel", id: "07" }).ok).toBe(false);
    expect(validateClientFrame({ type: "ping" }).ok).toBe(false);
    expect(validateClientFrame({ type: "req", id: "1", verb: "get", kind: "session", target: null }).ok).toBe(false);
  });

  it("accepts an unknown verb and an unknown optional field, which the Hub answers or ignores (EN-6)", () => {
    expect(validateClientFrame({ type: "req", id: "1", verb: "from_a_later_version", kind: "k", from_a_later_version: 1 })).toEqual(ok);
  });

  it("accepts the server frames of the clauses", () => {
    const frames: ServerFrame[] = [
      { type: "res", id: "1", ok: {} },
      { type: "res", id: "1", err: { code: "busy", category: "capacity", retryable: true, message: "m", retry_after_ms: 5 } },
      { type: "close", reason: "auth_failed" },
      { type: "close", reason: { busy: { retry_after_ms: 5 } } },
      { type: "close", reason: { hub_shutdown: { lock_released: true } } },
      { type: "watch", frame: "snapshot_begin", watch_id: "w" },
      { type: "watch", frame: "watch_closed", watch_id: "w", reason: "owner_unloaded" },
      { type: "event", frame: "gap", stream_id: "s", owner: "hub", name: "n", count: 1, recovery: "unavailable" },
    ];
    for (const frame of frames) expect(validateServerFrame(frame), JSON.stringify(frame)).toEqual(ok);
    expect(validateServerFrame({ type: "res", id: "1" }).ok).toBe(false);
    // EN-6: an unknown error code is valid, and a client maps it to its category.
    expect(validateDefinition("Error", { code: "from_a_later_version", category: "state", retryable: true, message: "m" })).toEqual(ok);
  });

  it("validates a describe result (DS-1)", () => {
    const result = {
      kinds: [
        {
          kind: "session",
          owner: "hub",
          descriptor_id: "d",
          schema_version: "1.0",
          schema: {},
          core_fields: ["id"],
          verbs: { get: { effect: "read", permission: "session.read", allowed: true } },
          actions: {},
          watchable: true,
          durable: true,
          projection_allowlist: ["id"],
        },
      ],
    };
    expect(validateDescribe(result)).toEqual(ok);
    expect(validateDescribe({ kinds: [{ ...result.kinds[0], verbs: { get: { effect: "read" } } }] }).ok).toBe(false);
  });
});

describe("the session row (HC A.8, steward ruling R-11)", () => {
  const row = {
    id: "s",
    hub_id: "h",
    label: "l",
    state: "running",
    lifecycle_class: "current",
    exit: {},
    size: { rows: 24, cols: 80 },
    session_type: { id: "t", source: "device" },
    traits: [],
    restartable: true,
    activity: "idle",
    presentation: "default",
    metadata: {},
  };
  const required = Object.keys(row);

  it("requires exactly the thirteen fields of R-11", () => {
    expect(validateRow("session", row)).toEqual(ok);
    const schema = schemas["hub-client/kinds/session.schema.json"] as { required: string[] };
    expect([...schema.required].sort()).toEqual([...required].sort());
    for (const field of required) {
      const without: Record<string, unknown> = { ...row };
      delete without[field];
      expect(validateRow("session", without).ok, field).toBe(false);
    }
  });

  it("has optional fields that are absent, never null, and a typed `attention`", () => {
    const withAttention = { ...row, attention: { since: 5, source: "osc9", title: "t" } };
    expect(validateRow("session", withAttention)).toEqual(ok);
    for (const field of ["failure_reason", "role", "interaction", "port", "owner", "worktree_id", "branch", "title", "task", "attention"]) {
      expect(validateRow("session", { ...row, [field]: null }).ok, field).toBe(false);
    }
    expect(validateRow("session", { ...row, attention: { since: 5 } }).ok).toBe(false);
  });
});

describe("the registries share one inventory (design 8, vault: one variant inventory)", () => {
  it("lists the error codes of the generated type, the schema and the registry in one order", () => {
    const codes = errorRegistry.codes.map((c) => c.code);
    expect(inventory.ErrorCode).toEqual(codes);
    expect(inventory.Category).toEqual(errorRegistry.categories);
  });

  it("gives every kind a schema, a version and an effect from the closed set of EN-7", () => {
    const effects = inventory.EffectClass;
    expect(Object.keys(kindRegistry).length).toBe(16);
    for (const [kind, spec] of Object.entries(kindRegistry)) {
      expect(schemas[`hub-client/kinds/${kind}.schema.json`], kind).toBeDefined();
      expect(spec.schema_version, kind).toMatch(/^\d+\.\d+$/);
      for (const verb of Object.values(spec.verbs)) expect(effects).toContain(verb.effect);
      for (const action of Object.values(spec.actions)) expect(effects).toContain(action.effect);
    }
  });
});

describe("property: a value made from the schema validates, and survives JSON", () => {
  const options = { numRuns: 60, seed: 20260930 };
  const roundTrip = (v: unknown): unknown => JSON.parse(JSON.stringify(v));

  it("every client frame", () => {
    const arb = arbitraryOf(envelope, { $ref: "#/$defs/ClientFrame" });
    fc.assert(
      fc.property(arb, (frame) => {
        expect(validateClientFrame(frame)).toEqual(ok);
        expect(roundTrip(frame)).toEqual(frame);
      }),
      options,
    );
  });

  it("every server frame", () => {
    const arb = arbitraryOf(envelope, { $ref: "#/$defs/ServerFrame" });
    fc.assert(
      fc.property(arb, (frame) => {
        expect(validateServerFrame(frame)).toEqual(ok);
        expect(roundTrip(frame)).toEqual(frame);
      }),
      options,
    );
  });

  it("removing a required member of a client frame makes it invalid", () => {
    const arb = arbitraryOf(envelope, { $ref: "#/$defs/ClientFrame" });
    fc.assert(
      fc.property(arb, fc.nat(), (frame, pick) => {
        const members = Object.keys(frame as object);
        const variant = (frame as { type: string }).type;
        const requiredKeys = ["type", ...Object.keys(defs[{ hello: "Hello", req: "Req", cancel: "Cancel" }[variant] as string]!.properties as object)].filter(
          (k) => ((defs[{ hello: "Hello", req: "Req", cancel: "Cancel" }[variant] as string]!.required as string[]) ?? []).concat("type").includes(k),
        );
        const key = requiredKeys[pick % requiredKeys.length]!;
        expect(members).toContain(key);
        const broken: Record<string, unknown> = { ...(frame as object) };
        delete broken[key];
        expect(validateClientFrame(broken).ok).toBe(false);
      }),
      options,
    );
  });

  // The `view` kind embeds the ui view document, which is recursive. The walker does not handle a cycle.
  const kinds = Object.keys(kindRegistry).filter((k) => k !== "view");
  it.each(kinds)("a %s row", (kind) => {
    const schema = schemas[`hub-client/kinds/${kind}.schema.json`] as Record<string, unknown>;
    const arb = arbitraryOf(schema);
    fc.assert(
      fc.property(arb, (row) => {
        expect(validateRow(kind, row)).toEqual(ok);
        expect(roundTrip(row)).toEqual(row);
      }),
      { numRuns: 25, seed: 20260930 },
    );
  });
});
