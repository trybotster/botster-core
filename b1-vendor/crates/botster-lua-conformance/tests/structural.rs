//! The Lua binding transcripts in Phase 0 (design 7.1, ruling R-1.4): the Lua host has no fake, so each transcript gets the structural check, plus the checks in this file that
//! keep the scripts honest: the Lua parses, every call has the arguments of Appendix A, every scripted answer has the result of Appendix A, and every cited clause exists.

use botster_conformance::{load_dir, Transcript};
use botster_lua_conformance::{structural, LB_TRANSCRIPTS};
use serde_json::Value;
use std::collections::BTreeMap;

botster_lua_conformance::lua_host_conformance_tests!(structural::subject);

fn transcripts() -> Vec<Transcript> {
    load_dir(&LB_TRANSCRIPTS).expect("the transcripts load")
}

fn lua_files(t: &Transcript) -> BTreeMap<String, String> {
    t.setup["package"]["files"]
        .as_object()
        .map(|m| {
            m.iter()
                .filter(|(p, _)| p.ends_with(".lua"))
                .map(|(p, s)| (p.clone(), s.as_str().unwrap_or_default().to_string()))
                .collect()
        })
        .unwrap_or_default()
}

// The scripts are real Lua 5.4 (the host's language, binding section D), so a typo never reaches a real host as a confusing failure.
#[test]
fn every_lua_source_parses() {
    let mut bad = vec![];
    for t in transcripts() {
        for (path, source) in lua_files(&t) {
            if let Err(e) = full_moon::parse(&source) {
                bad.push(format!("{} {path}: {e:?}", t.id));
            }
        }
        // A package file that holds JSON (the agent definitions) parses as JSON.
        if let Some(files) = t.setup["package"]["files"].as_object() {
            for (path, text) in files.iter().filter(|(p, _)| p.ends_with(".json")) {
                if serde_json::from_str::<Value>(text.as_str().unwrap_or_default()).is_err() {
                    bad.push(format!("{} {path}: not JSON", t.id));
                }
            }
        }
    }
    assert!(bad.is_empty(), "{bad:#?}");
}

// Design 5.6, rule 6: every expectation of every transcript cites the clause that fixes its value.
#[test]
fn every_expectation_cites_its_clause() {
    let mut missing = vec![];
    for t in transcripts() {
        for step in botster_conformance::citation::missing_because(&t) {
            missing.push(format!("{} step {step}", t.id));
        }
    }
    assert!(missing.is_empty(), "no `because`: {missing:#?}");
}

fn files_text() -> (String, String) {
    let lb = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../frozen/current/hub-plugin-lua-binding-v1.1-r9.md"
    ))
    .unwrap();
    let a1 = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../frozen/current/hub-plugin-lua-binding-v1.1-r9-amendment-1-candidate1.md"
    ))
    .unwrap();
    let a2 = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../frozen/current/hub-plugin-lua-binding-v1.1-r9-amendment-2-candidate1.md"
    ))
    .unwrap();
    (lb, format!("{a1}\n{a2}"))
}

// Design 5.6, rule 6: every cited clause is a clause of the file its prefix names.
#[test]
fn every_cited_clause_exists() {
    let (lb, a1) = files_text();
    let hp: Vec<String> = [
        "hub-plugin-contract-v1.1.joint-J4e.md",
        "hub-plugin-contract-v1.1.amendment-1-candidate5.md",
        "hub-plugin-contract-v1.1.amendment-2-candidate7.md",
    ]
    .iter()
    .map(|f| {
        std::fs::read_to_string(format!(
            "{}/../../frozen/current/{f}",
            env!("CARGO_MANIFEST_DIR")
        ))
        .unwrap()
    })
    .collect();
    let has_lb = |id: &str| {
        lb.contains(&format!("**{id} "))
            || lb.contains(&format!("**{id}."))
            || a1.contains(&format!("| {id} |"))
    };
    let has_hp = |id: &str| {
        hp.iter().any(|f| {
            f.lines().any(|l| {
                l.starts_with(&format!("| {id} "))
                    || l.starts_with(&format!("| {id} ("))
                    || l.contains(&format!("**{id}"))
            })
        })
    };
    let mut unknown = vec![];
    for t in transcripts() {
        for step in &t.steps {
            let list: Vec<&str> = match step.get("because") {
                Some(Value::String(s)) => vec![s.as_str()],
                Some(Value::Array(a)) => a.iter().filter_map(Value::as_str).collect(),
                _ => vec![],
            };
            for c in list {
                let found = match c.split_once(' ') {
                    Some(("LB", id)) => has_lb(id),
                    Some(("HP", id)) => has_hp(id),
                    _ => false,
                };
                if !found {
                    unknown.push(format!("{}: {c}", t.id));
                }
            }
        }
    }
    assert!(unknown.is_empty(), "{unknown:#?}");
}

fn calls_doc() -> Value {
    serde_json::from_str(include_str!(
        "../../../schemas/plugin/plugin-calls.schema.json"
    ))
    .unwrap()
}

fn validator(schema: &Value) -> jsonschema::Validator {
    jsonschema::validator_for(schema).expect("the call schema compiles")
}

// Appendix A: every HostCall that a transcript expects has the arguments of that call, and every answer that the script gives has the result of that call (or a failure shape).
// Arguments with a variable or a matcher are skipped; the transcript still names the call.
#[test]
fn calls_and_scripted_answers_follow_appendix_a() {
    let doc = calls_doc();
    let calls = doc["calls"].as_array().unwrap();
    let mut bad = vec![];
    for t in transcripts() {
        let mut by_var: BTreeMap<String, String> = BTreeMap::new();
        for step in &t.steps {
            if let Some(exp) = step.get("expect") {
                if exp["type"] == "HostCall" {
                    let p = &exp["payload"];
                    let name = p["call"].as_str().unwrap_or_default();
                    let Some(spec) = calls.iter().find(|c| c["name"] == name) else {
                        bad.push(format!("{}: no call '{name}'", t.id));
                        continue;
                    };
                    if let Some(var) = p["call_id"].as_str() {
                        by_var.insert(var.to_string(), name.to_string());
                    }
                    let args = &p["args"];
                    if !args.to_string().contains('$') && !validator(&spec["args"]).is_valid(args) {
                        bad.push(format!("{}: args of {name}: {args}", t.id));
                    }
                }
            }
            if let Some(f) = step.get("send_frame") {
                if f["type"] == "HostResult" {
                    let var = f["payload"]["call_id"].as_str().unwrap_or_default();
                    let Some(name) = by_var.get(var) else {
                        bad.push(format!("{}: answer to unknown call {var}", t.id));
                        continue;
                    };
                    let spec = calls.iter().find(|c| c["name"] == name.as_str()).unwrap();
                    let r = &f["payload"]["result"];
                    if r["ok"] == true && !validator(&spec["result"]).is_valid(&r["value"]) {
                        bad.push(format!("{}: result of {name}: {}", t.id, r["value"]));
                    }
                }
            }
        }
    }
    assert!(bad.is_empty(), "{bad:#?}");
}

// Every manifest of a package validates against the generated manifest schema (HP MN-1), except a transcript that sets a key wrong on purpose (the admission ids).
#[test]
fn package_manifests_validate() {
    let schema: Value = serde_json::from_str(include_str!(
        "../../../schemas/plugin/plugin-manifest.schema.json"
    ))
    .unwrap();
    let v = validator(&schema);
    let templates: Value =
        serde_json::from_str(include_str!("../../../conformance/hp/fixtures/frames.json")).unwrap();
    let mut bad = vec![];
    for t in transcripts() {
        let mut manifests = vec![];
        if let Some(m) = t.setup["package"].get("manifest") {
            manifests.push(m.clone());
        }
        // The admission controls carry their own packages.
        for s in &t.steps {
            if let Some(m) = s["control"]["package"].get("manifest") {
                manifests.push(m.clone());
            }
        }
        for mut m in manifests {
            if m.get("$ref").is_some() {
                let name = m["$ref"].as_str().unwrap().to_string();
                let mut base = templates[&name].clone();
                for (k, val) in m.as_object().unwrap() {
                    if k != "$ref" {
                        base[k] = val.clone();
                    }
                }
                m = base;
            }
            let wrong_on_purpose =
                t.id.contains("lb_17_manifest_keys_closed") || t.id.contains("lb_18_definition");
            if !wrong_on_purpose && !v.is_valid(&m) {
                bad.push(format!(
                    "{}: {}",
                    t.id,
                    v.iter_errors(&m)
                        .map(|e| e.to_string())
                        .collect::<Vec<_>>()
                        .join("; ")
                ));
            }
        }
    }
    assert!(bad.is_empty(), "{bad:#?}");
}

// LB-1: the names that the plugin probes are the names that the contract publishes (the extractor over the calls table, LH-4 artifact), not a hand-kept list.
#[test]
fn lb_1_probes_the_published_botster_names() {
    use botster_plugin_contract::prelude::*;
    let t = transcripts()
        .into_iter()
        .find(|t| t.id == "conf::lb_1_binding_covers_every_call")
        .unwrap();
    let source = &lua_files(&t)["main.lua"];
    let probed: Vec<String> = source
        .lines()
        .take_while(|l| !l.starts_with("botster.register"))
        .filter_map(|l| {
            l.trim()
                .strip_prefix('"')
                .and_then(|r| r.strip_suffix("\","))
                .map(String::from)
        })
        .collect();
    assert_eq!(probed, botster_names());
}

// Every transcript of this suite is structural (R-1.4) and names its ruling; no expected value is a hand-copied global list.
#[test]
fn every_transcript_is_structural_with_a_ruling() {
    for t in transcripts() {
        assert_eq!(
            t.fake,
            Some(botster_conformance::FakeMode::Structural),
            "{}",
            t.id
        );
        assert_eq!(t.ruling.as_deref(), Some("R-1.4"), "{}", t.id);
    }
}

// A transcript lists every control that its steps use (design 5.1: `requires.controls`), the barrier of a quiet step included.
#[test]
fn requires_lists_every_control_a_transcript_uses() {
    let mut bad = vec![];
    for t in transcripts() {
        let mut used: Vec<String> = vec![];
        for step in &t.steps {
            if let Some(op) = step["control"]["op"].as_str() {
                used.push(op.to_string());
            }
            if let Some(op) = step["barrier"]["control"]["op"].as_str() {
                used.push(op.to_string());
            }
        }
        for op in used {
            if !t.requires.controls.contains(&op) {
                bad.push(format!("{}: {op}", t.id));
            }
        }
    }
    assert!(bad.is_empty(), "{bad:#?}");
}

// The published control inventory names every control that a transcript uses, so a harness author sees the whole list (design 6.3).
#[test]
fn the_control_inventory_covers_every_control_a_transcript_requires() {
    let mut bad = vec![];
    for t in transcripts() {
        for op in &t.requires.controls {
            if !botster_lua_conformance::LUA_HOST_CONTROLS.contains(&op.as_str()) {
                bad.push(format!("{}: {op}", t.id));
            }
        }
    }
    assert!(bad.is_empty(), "{bad:#?}");
}
