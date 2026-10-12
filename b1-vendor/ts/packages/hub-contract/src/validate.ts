// Runtime validation of frames and rows against the checked-in schemas (Ajv 2020; design 5.4: a received frame is always validated).
import Ajv2020 from "ajv/dist/2020.js";
import type { ErrorObject, ValidateFunction } from "ajv/dist/2020.js";
import { schemas } from "./generated/schemas.js";

/** A schema keyword that this contract adds. Ajv would refuse an unknown keyword in strict mode. */
const extraKeywords = ["x-operation-kinds", "x-kind", "x-schema-version", "x-verbs", "x-actions"];
/** The number formats that schemars writes. They carry no extra rule here: the `minimum` and `maximum` keywords bound them. */
const numberFormats = ["uint", "uint8", "uint16", "uint32", "uint64", "int8", "int16", "int32", "int64", "double", "float"];

function makeAjv(): Ajv2020 {
  const ajv = new Ajv2020({ allErrors: true, strict: true, strictRequired: false, validateFormats: true });
  for (const keyword of extraKeywords) ajv.addKeyword(keyword);
  for (const format of numberFormats) ajv.addFormat(format, { type: "number", validate: () => true });
  for (const [path, schema] of Object.entries(schemas)) ajv.addSchema(schema as object, path);
  return ajv;
}

let shared: Ajv2020 | undefined;
const ajv = (): Ajv2020 => (shared ??= makeAjv());

/** The result of a validation: `ok`, or the Ajv errors. */
export type Validation = { readonly ok: true } | { readonly ok: false; readonly errors: readonly ErrorObject[] };

function run(validate: ValidateFunction, value: unknown): Validation {
  return validate(value) ? { ok: true } : { ok: false, errors: [...(validate.errors ?? [])] };
}

const compiled = new Map<string, ValidateFunction>();
function validator(ref: string): ValidateFunction {
  let fn = compiled.get(ref);
  if (!fn) {
    fn = ajv().getSchema(ref);
    if (!fn) throw new Error(`no schema ${ref}`);
    compiled.set(ref, fn);
  }
  return fn;
}

/** The path under `schemas/` of the envelope schema. */
export const ENVELOPE = "hub-client/envelope.schema.json";

/** Validates a frame that a client sends (HC EN-1, A4 EN-1a). */
export const validateClientFrame = (value: unknown): Validation => run(validator(`${ENVELOPE}#/$defs/ClientFrame`), value);
/** Validates a frame that the Hub sends (HC EN-2). */
export const validateServerFrame = (value: unknown): Validation => run(validator(`${ENVELOPE}#/$defs/ServerFrame`), value);
/** Validates a `describe` result (HC DS-1). */
export const validateDescribe = (value: unknown): Validation => run(validator(`${ENVELOPE}#/$defs/DescribeResult`), value);
/** Validates one row of a Hub-owned kind against the kind's schema (HC section 6). */
export const validateRow = (kind: string, value: unknown): Validation => run(validator(`hub-client/kinds/${kind}.schema.json`), value);
/** Validates a value against a definition of the envelope schema by name, for example `Error`, `Limits` or `WatchFrame`. */
export const validateDefinition = (name: string, value: unknown): Validation => run(validator(`${ENVELOPE}#/$defs/${name}`), value);
/** Validates a value against any checked-in schema by its path under `schemas/`, with an optional JSON Pointer fragment. */
export const validateWith = (path: string, value: unknown): Validation => run(validator(path), value);
