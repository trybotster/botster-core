// A small schema-to-fast-check walker over the keywords that schemars emits for this contract.
//
// Hand-rolled, with reason (docs/tooling.md): json-schema-faker does not shrink or follow fast-check's seed, and json-schema-fast-check is unmaintained.
// It covers: `$ref` into `$defs`, `type`, `enum`, `const`, `pattern`, `minimum`/`maximum`, `minLength`/`maxLength`, `items`, `minItems`, `properties`/`required`,
// `additionalProperties` (as a value schema), `minProperties`/`maxProperties`, `anyOf`, `oneOf` and `allOf`.
// A recursive schema (the ui view document) is out of scope: a `$ref` cycle throws, and the caller picks schemas without one.
import fc from "fast-check";

type Schema = boolean | { [key: string]: unknown };
type Obj = { [key: string]: unknown };

export function arbitraryOf(root: Obj, schema: Schema = root, seen: readonly string[] = []): fc.Arbitrary<unknown> {
  if (schema === true) return fc.oneof(fc.string({ maxLength: 8 }), fc.integer({ min: 0, max: 1000 }), fc.boolean());
  if (schema === false) throw new Error("a false schema has no value");
  const s = schema as Obj;
  if (typeof s.$ref === "string") {
    const ref = s.$ref;
    if (seen.includes(ref)) throw new Error(`recursive schema at ${ref}`);
    const name = ref.replace("#/$defs/", "");
    const defs = root.$defs as Record<string, Schema>;
    const target = defs[name];
    if (target === undefined) throw new Error(`no definition ${name}`);
    return arbitraryOf(root, target, [...seen, ref]);
  }
  if ("const" in s) return fc.constant(s.const);
  if (Array.isArray(s.enum)) return fc.constantFrom(...(s.enum as unknown[]));
  if (Array.isArray(s.allOf)) {
    const own = s.properties === undefined ? [] : [{ type: "object", properties: s.properties, required: s.required ?? [] } as Schema];
    return allOf(root, [...own, ...(s.allOf as Schema[])], seen);
  }
  for (const key of ["anyOf", "oneOf"] as const) {
    const options = s[key];
    if (Array.isArray(options) && s.properties !== undefined) {
      // A struct that says "exactly one of these members" (`oneOf` of `required` sets): pick a set, keep its members, drop the members of the others.
      const sets = (options as Obj[]).filter((o) => Array.isArray(o.required) && o.type === undefined);
      if (sets.length === options.length) {
        const own = object(root, { type: "object", properties: s.properties, required: s.required }, seen);
        return fc.tuple(own, fc.nat(sets.length - 1)).map(([made, pick]) => {
          const chosen = sets[pick]!.required as string[];
          const out: Obj = { ...(made as Obj) };
          for (const other of sets) for (const key of other.required as string[]) delete out[key];
          const withValues = fc.sample(object(root, { type: "object", properties: s.properties, required: chosen }, seen), { numRuns: 1, seed: pick }) as Obj[];
          for (const key of chosen) out[key] = withValues[0]![key];
          return out;
        });
      }
      // A struct with a flattened untagged enum: its own members, and the members of one branch.
      const own = object(root, { type: "object", properties: s.properties, required: s.required }, seen);
      const branch = fc.oneof(...(options as Schema[]).map((o) => arbitraryOf(root, o, seen)));
      return fc.tuple(own, branch).map(([a, b]) => ({ ...(a as Obj), ...(b as Obj) }));
    }
    if (Array.isArray(options)) return fc.oneof(...(options as Schema[]).map((o) => arbitraryOf(root, o, seen)));
  }
  const type = Array.isArray(s.type) ? (s.type as string[])[0] : s.type;
  switch (type) {
    case "string": {
      if (typeof s.pattern === "string") return fc.stringMatching(new RegExp(s.pattern));
      return fc.string({ minLength: Number(s.minLength ?? 0), maxLength: Number(s.maxLength ?? 12) });
    }
    case "integer":
      return fc.integer({ min: Number(s.minimum ?? 0), max: Math.min(Number(s.maximum ?? 100_000), Number.MAX_SAFE_INTEGER) });
    case "number":
      return fc.double({ min: Number(s.minimum ?? 0), max: Number(s.maximum ?? 1e6), noNaN: true, noDefaultInfinity: true });
    case "boolean":
      return fc.boolean();
    case "array": {
      const items = (s.items ?? true) as Schema;
      return fc.array(arbitraryOf(root, items, seen), { minLength: Number(s.minItems ?? 0), maxLength: 3 });
    }
    case "object":
    case undefined:
      return object(root, s, seen);
    default:
      throw new Error(`unsupported type ${String(type)}`);
  }
}

function object(root: Obj, s: Obj, seen: readonly string[]): fc.Arbitrary<unknown> {
  const properties = (s.properties ?? {}) as Record<string, Schema>;
  const required = (s.required ?? []) as string[];
  const names = Object.keys(properties);
  if (names.length === 0 && s.additionalProperties !== undefined && s.additionalProperties !== false) {
    const value = arbitraryOf(root, s.additionalProperties as Schema, seen);
    return fc.dictionary(fc.stringMatching(/^[a-z][a-z_]{0,6}$/), value, {
      minKeys: Number(s.minProperties ?? 0),
      maxKeys: Number(s.maxProperties ?? 3),
    });
  }
  const model: Record<string, fc.Arbitrary<unknown>> = {};
  for (const name of names) model[name] = arbitraryOf(root, properties[name] as Schema, seen);
  return fc.record(model, { requiredKeys: names.filter((n) => required.includes(n)) });
}

function allOf(root: Obj, parts: Schema[], seen: readonly string[]): fc.Arbitrary<unknown> {
  // A conditional part (`if`/`then`/`else`) is the rule of HC A4 EN-1a: `action` exactly when `verb` is `invoke`. The walker applies it after the object is made.
  const plain = parts.filter((p) => typeof p === "object" && !("if" in p));
  const conditional = parts.filter((p) => typeof p === "object" && "if" in p) as Obj[];
  return fc.tuple(...plain.map((p) => arbitraryOf(root, p, seen))).chain((made) => {
    const merged: Obj = {};
    for (const part of made) Object.assign(merged, part as Obj);
    // Only the rule of EN-1a is applied. Another conditional (the per-kind result of an operation) holds for a random `kind` that names no known kind.
    if (!conditional.some((c) => JSON.stringify(c).includes('"verb"'))) return fc.constant(merged);
    return fc.string({ minLength: 1, maxLength: 6 }).map((action) => {
      const out = { ...merged };
      if (out.verb === "invoke") out.action = typeof out.action === "string" ? out.action : action;
      else delete out.action;
      return out;
    });
  });
}
