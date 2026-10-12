// The generated TypeScript must say what the schemas say, field by field (vault: "generated typescript dtos must encode serde field optionality",
// "generated dto drift tests need symmetric field and type checks"). `npm run codegen -- --check` in CI covers the files; this test covers the meaning.
import { describe, expect, it } from "vitest";
import ts from "typescript";
import { fileURLToPath } from "node:url";
import { schemas } from "../src/generated/schemas.js";
import { inventory } from "../src/generated/inventory.js";

type Schema = { [key: string]: unknown };

/** The object definitions of the Hub schemas: every `$defs` entry and every root (named by its title), by name. */
function objectSchemas(): Map<string, Schema> {
  const out = new Map<string, Schema>();
  for (const [path, schema] of Object.entries(schemas)) {
    if (!path.startsWith("hub-client/") || path.startsWith("hub-client/route/")) continue;
    const s = schema as Schema;
    const all: [string, Schema][] = Object.entries((s.$defs ?? {}) as Record<string, Schema>);
    if (typeof s.title === "string") all.push([s.title, s]);
    for (const [name, def] of all) out.set(name, def);
  }
  return out;
}

const conditionalOnly = (def: Schema): boolean => !def.anyOf && !def.oneOf && ((def.allOf as Schema[] | undefined) ?? []).every((p) => "if" in p);

const generated = fileURLToPath(new URL("../src/generated/hub.ts", import.meta.url));
const program = ts.createProgram([generated], { strict: true, target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.ESNext, moduleResolution: ts.ModuleResolutionKind.Bundler, noEmit: true });
const checker = program.getTypeChecker();
const moduleSymbol = checker.getSymbolAtLocation(program.getSourceFile(generated)!)!;
const exported = new Map(checker.getExportsOfModule(moduleSymbol).map((s) => [s.getName(), s]));

const coarse = (schema: Schema | undefined): string | undefined => {
  if (!schema || schema.enum || schema.const !== undefined || schema.anyOf || schema.oneOf || schema.$ref || schema.allOf) return undefined;
  const t = Array.isArray(schema.type) ? schema.type[0] : schema.type;
  return { string: "string", integer: "number", number: "number", boolean: "boolean", array: "array" }[t as string];
};

describe("generated types against the schemas", () => {
  const defs = [...objectSchemas()].filter(([, def]) => def.properties && conditionalOnly(def));

  it("checks a useful number of object types", () => {
    expect(defs.length).toBeGreaterThan(80);
  });

  it.each(defs)("%s has the schema's fields, the schema's optionality and the schema's primitive types", (name, def) => {
    const symbol = exported.get(name);
    expect(symbol, `${name} is exported`).toBeDefined();
    const type = checker.getDeclaredTypeOfSymbol(symbol!);
    const props = new Map(checker.getPropertiesOfType(type).map((p) => [p.getName(), p]));
    const schemaProps = def.properties as Record<string, Schema>;
    const required = new Set((def.required as string[] | undefined) ?? []);
    // Symmetric: no field only in the schema, no field only in the type.
    expect([...props.keys()].sort()).toEqual(Object.keys(schemaProps).sort());
    for (const [field, schema] of Object.entries(schemaProps)) {
      const prop = props.get(field)!;
      const optional = (prop.flags & ts.SymbolFlags.Optional) !== 0;
      expect(optional, `${name}.${field} optional`).toBe(!required.has(field));
      const want = coarse(schema);
      if (want) {
        const text = checker.typeToString(checker.getNonNullableType(checker.getTypeOfSymbol(prop)));
        const got = text.endsWith("[]") || text.startsWith("Array<") ? "array" : text;
        expect(got, `${name}.${field} type`).toBe(want);
      }
    }
  });

  it("lists every variant of every open set once, in the inventory", () => {
    const sets = [...objectSchemas()].filter(([, def]) => Array.isArray((def.anyOf as Schema[] | undefined)?.[0]?.enum));
    expect(sets.length).toBeGreaterThan(30);
    for (const [name, def] of sets) {
      const names = (def.anyOf as Schema[])[0]!.enum as string[];
      expect(new Set(names).size, `${name} has no duplicate`).toBe(names.length);
      expect((inventory as Record<string, readonly string[]>)[name], name).toEqual(names);
      // The generated union lists the same names, in the same order, and accepts any other string.
      const text = checker.typeToString(checker.getDeclaredTypeOfSymbol(exported.get(name)!), undefined, ts.TypeFormatFlags.NoTruncation | ts.TypeFormatFlags.InTypeAlias);
      for (const n of names) expect(text, `${name} lists ${n}`).toContain(JSON.stringify(n));
    }
  });
});
