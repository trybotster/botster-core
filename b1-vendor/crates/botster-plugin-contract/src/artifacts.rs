//! The generated artifacts of HP Appendix B, A1 and A2 (written to `schemas/plugin/` by `cargo xtask codegen`).
//!
//! Clause: HP Appendix B, A2-25, A2-43, A2-44, PK-4, PK-6, section 15.
//!
//! Each schema artifact carries `$id: urn:botster:schema:plugin:v<botster_plugin_api>:<artifact>` (A2-43).

use crate::calls::{CALLS, COMMON_FAILURES};
use crate::errors;
use crate::events::{Aud, EVENTS};
use crate::files::{Accessory, AgentDefinition, Catalog};
use crate::frames::*;
use crate::globals;
use crate::handlers::HANDLERS;
use crate::limits::{LimitSpec, Range, LIMIT_SPECS, TRUSTED_HOSTS_KEY};
use crate::manifest::{Manifest, BOTSTER_PLUGIN_API};
use crate::operations::{OPERATION_KINDS, STATE_TABLES};
use crate::wire::{Direction, FRAMES};
use serde_json::{json, Map, Value};

const SCHEMA_URI: &str = "https://json-schema.org/draft/2020-12/schema";

/// The `$id` of an artifact (HP A2-43).
pub fn artifact_id(file: &str) -> String {
    format!("urn:botster:schema:plugin:v{BOTSTER_PLUGIN_API}:{file}")
}

fn pretty(v: &Value) -> String {
    let mut s = serde_json::to_string_pretty(v).expect("a value serializes");
    s.push('\n');
    s
}

/// Puts `$schema` and `$id` first in an artifact object.
fn stamp(file: &str, body: Value) -> Value {
    let mut out = Map::new();
    out.insert("$schema".into(), json!(SCHEMA_URI));
    out.insert("$id".into(), json!(artifact_id(file)));
    if let Value::Object(m) = body {
        for (k, v) in m {
            if k != "$schema" && k != "$id" {
                out.insert(k, v);
            }
        }
    }
    Value::Object(out)
}

fn range_json(r: &Range) -> Value {
    match r {
        Range::HubBound => json!({"kind": "hub_bound"}),
        Range::Between(a, b) => json!({"kind": "between", "min": a, "max": b}),
        Range::AtLeast(o) => json!({"kind": "at_least", "limit": o}),
        Range::AtMost(o) => json!({"kind": "at_most", "limit": o}),
    }
}

fn limit_json(l: &LimitSpec) -> Value {
    let toml = match l.toml {
        Some((t, m)) => json!({"table": t, "mode": m}),
        None => Value::Null,
    };
    json!({"name": l.name, "range": range_json(&l.range), "toml": toml, "clauses": l.clauses})
}

/// `plugin-frames.schema.json`: one schema per frame with its lane, type byte and class (HP Appendix B).
pub fn frames_schema() -> Value {
    let mut g = crate::schema::generator(false);
    macro_rules! sub {
        ($t:ty) => {
            g.subschema_for::<$t>()
        };
    }
    let refs: Vec<(&str, Value)> = vec![
        ("Hello", serde_json::to_value(sub!(Hello)).unwrap()),
        ("Ready", serde_json::to_value(sub!(Ready)).unwrap()),
        ("Activated", serde_json::to_value(sub!(Activated)).unwrap()),
        ("Shutdown", serde_json::to_value(sub!(Shutdown)).unwrap()),
        ("Fault", serde_json::to_value(sub!(Fault)).unwrap()),
        ("Cancel", serde_json::to_value(sub!(Cancel)).unwrap()),
        ("Invoke", serde_json::to_value(sub!(Invoke)).unwrap()),
        ("Result", serde_json::to_value(sub!(ResultFrame)).unwrap()),
        ("HostCall", serde_json::to_value(sub!(HostCall)).unwrap()),
        (
            "HostResult",
            serde_json::to_value(sub!(HostResult)).unwrap(),
        ),
    ];
    let defs = g.take_definitions(true);
    let frames: Vec<Value> = FRAMES
        .iter()
        .map(|f| {
            let r = refs.iter().find(|(n, _)| *n == f.name).expect("every frame has a schema").1.clone();
            let lane = match f.lanes {
                [l] => json!(l.byte()),
                _ => json!("2, or 3 for a bulk-class call"),
            };
            json!({
                "name": f.name,
                "type": f.byte,
                "type_hex": format!("0x{:02X}", f.byte),
                "lane": lane,
                "lane_names": f.lanes.iter().map(|l| serde_json::to_value(l).unwrap()).collect::<Vec<_>>(),
                "class": serde_json::to_value(crate::wire::FrameClass::of_byte(f.byte)).unwrap(),
                "direction": match f.direction { Direction::HubToPlugin => "hub_to_plugin", Direction::PluginToHub => "plugin_to_hub" },
                "clauses": f.clauses,
                "schema": r,
            })
        })
        .collect();
    let mut body = Map::new();
    body.insert(
        "description".into(),
        json!("HP Appendix B: one schema per frame type with the lane, the type byte and the class. The type bytes are pinned here (steward ruling R-4). A frame payload is one JSON object that also carries the connection epoch."),
    );
    body.insert(
        "lanes".into(),
        json!({"control": 0, "invoke": 1, "call": 2, "bulk": 3}),
    );
    body.insert(
        "classes".into(),
        json!({"state_changing": "0x00-0x7F", "advisory": "0x80-0xFF"}),
    );
    body.insert("bulk_call".into(), json!(crate::wire::BULK_CALL));
    body.insert("frames".into(), Value::Array(frames));
    body.insert(
        "limits".into(),
        Value::Array(LIMIT_SPECS.iter().map(limit_json).collect()),
    );
    body.insert(
        "hub_toml_extra_keys".into(),
        json!([{"table": TRUSTED_HOSTS_KEY.0, "key": TRUSTED_HOSTS_KEY.1, "mode": TRUSTED_HOSTS_KEY.2}]),
    );
    body.insert(
        "priorities".into(),
        serde_json::to_value(v1_priorities()).unwrap(),
    );
    body.insert("features".into(), json!({"mcp_proxy": FEATURE_MCP_PROXY}));
    body.insert("$defs".into(), Value::Object(defs));
    stamp("plugin-frames.schema.json", Value::Object(body))
}

fn failures_json(fs: &[crate::calls::Failure]) -> Value {
    Value::Array(
        fs.iter()
            .map(|f| json!({"kind": f.kind, "codes": f.codes}))
            .collect(),
    )
}

/// `plugin-calls.schema.json`: one argument and one result schema per call, with the context class, grant, commit point and failures (HP Appendix B, section 24, A2-5, A2-6, A2-4).
pub fn calls_schema() -> Value {
    let calls: Vec<Value> = CALLS
        .iter()
        .map(|c| {
            json!({
                "name": c.name,
                "lua_name": c.lua_name(),
                "contexts": c.contexts.iter().map(|x| match x { crate::calls::Ctx::Req => "req", crate::calls::Ctx::Boot => "boot", crate::calls::Ctx::Auto => "auto" }).collect::<Vec<_>>(),
                "lane": if c.bulk { 3 } else { 2 },
                "grant": c.grant,
                "commit": c.commit,
                "mutating": c.mutating,
                "failures": failures_json(c.failures),
                "clauses": c.clauses,
                "args": (c.args)(),
                "result": (c.result)(),
            })
        })
        .collect();
    let handlers: Vec<Value> = HANDLERS
        .iter()
        .map(|h| {
            json!({
                "kind": h.kind.as_str(),
                "grant": h.grant,
                "clauses": h.clauses,
                "descriptor": (h.descriptor)(),
                "payload": (h.payload)(),
                "result": (h.result)(),
            })
        })
        .collect();
    let events: Vec<Value> = EVENTS
        .iter()
        .map(|e| {
            json!({
                "owner": e.owner,
                "name": e.name,
                "priority": e.priority,
                "audience": match e.audience { Aud::Clients => json!("clients"), Aud::Plugins => json!("plugins"), Aud::Both => json!("clients_and_plugins"), Aud::Unstated => Value::Null },
                "grant": e.grant,
                "subject": e.subject,
                "clauses": e.clauses,
                "payload": (e.payload)(),
            })
        })
        .collect();
    let states: Vec<Value> = STATE_TABLES
        .iter()
        .map(|t| json!({"kind": t.kind, "states": t.states, "terminal": t.terminal}))
        .collect();
    stamp(
        "plugin-calls.schema.json",
        json!({
            "description": "HP Appendix B, section 24: one argument and one result schema per call, with the context class, grant, commit point and failures; the handler kinds (A2-5); the Hub and terminal events (A2-6). Calls not listed do not exist in v1.",
            "common_failures": failures_json(COMMON_FAILURES),
            "calls": calls,
            "handler_kinds": handlers,
            "events": events,
            "operations": {"kinds": OPERATION_KINDS, "states": states},
        }),
    )
}

fn rooted_with_id(file: &str, v: Value) -> Value {
    stamp(file, v)
}

/// The artifacts as (file name, text) pairs under `schemas/plugin/`. They are made once per process, because each generator of `cargo xtask codegen` asks for the set.
pub fn all() -> Vec<(&'static str, String)> {
    static MADE: std::sync::OnceLock<Vec<(&'static str, String)>> = std::sync::OnceLock::new();
    MADE.get_or_init(make).clone()
}

fn make() -> Vec<(&'static str, String)> {
    let manifest = rooted_with_id(
        "plugin-manifest.schema.json",
        crate::schema::rooted::<Manifest>(),
    );
    let agent = rooted_with_id(
        "agent-definition.schema.json",
        crate::schema::rooted::<AgentDefinition>(),
    );
    let accessory = rooted_with_id(
        "accessory.schema.json",
        crate::schema::rooted::<Accessory>(),
    );
    let catalog = rooted_with_id("catalog.schema.json", crate::schema::rooted::<Catalog>());
    let errors = stamp("plugin-errors.json", errors::registry_json());
    let globals = stamp("lua-host-globals.json", globals::artifact_json());
    vec![
        ("plugin-frames.schema.json", pretty(&frames_schema())),
        ("plugin-manifest.schema.json", pretty(&manifest)),
        ("plugin-calls.schema.json", pretty(&calls_schema())),
        ("plugin-errors.json", pretty(&errors)),
        ("lua-host-globals.json", pretty(&globals)),
        ("agent-definition.schema.json", pretty(&agent)),
        ("accessory.schema.json", pretty(&accessory)),
        ("catalog.schema.json", pretty(&catalog)),
    ]
}
