//! The vector runner (UI-RZ-3, UI-RZ-3b): runs every vector file against the reference `realize` and admission.
//!
//! A vector file is a transcript of the `realizer` subject (design 5.5). It has the envelope fields `id`, `contract`,
//! `clause`, `subject` and a list of `cases`. A case has a `name` and one of these forms:
//!
//! * a realization: `document`, and optionally `kinds`, `local_state`, `viewport`, `capabilities`,
//!   (the inputs of UI section 12), then the checks `expect`, `expect_nodes`, `expect_absent`, `expect_diagnostics`,
//!   `expect_no_diagnostics`, `same_ids_as` and `same_tree_as`;
//! * an admission: `document`, `context`, and `expect_admitted: true` or `expect_admission: { code, path? }`;
//! * `ids`: a check of the node id functions (UI-ID-3);
//! * `parse`: a check that a wire value is, or is not, a value of a named type;
//! * `expect_idempotent: true` on a realization: the renderer applies `local_changes` to `local_state` by the rules of
//!   UI-NV-4, and a second `realize` gives the same tree and no changes;
//! * `document_text` with `expect_admission`: the document is JSON text, so a number outside the double range can be
//!   written (UI-FD-2);
//! * `badge`, `home` and `url`: the pure functions of UI-NB-1, UI-PL-1, UI-SL-6 and UI-SL-9.
//!
//! A JSON value cannot hold a number outside the double range, so a vector writes one as `{"$raw_number": "<text>"}`.
//! This runner removes such a member before the input is parsed, which is the "absent" of UI-FD-2. A renderer whose
//! JSON parser produces `Infinity` (Web) treats the value as absent by the same rule.
//!
//! `expect` matches by subset: an object matcher lists the keys that must match, an array matcher matches element by
//! element and by length, `{"$any": true}` matches any present value and `{"$absent": true}` matches a missing key.
//! Every case has `"because": "UI <clause id>"`: a sentence of that clause fixes the expected values (design 5.6 rule 6).
//! A vector never asserts what the contract leaves open (design 5.6).

use crate::admission::{admit, admit_badge, admit_text, AdmissionContext};
use crate::doc::*;
use crate::ids::{child_id, draft_key, split_id};
use crate::input::RealizeInput;
use crate::realize::{diagnostic_tree, realize};
use include_dir::{include_dir, Dir};
use serde_json::{json, Map, Value};
use std::collections::BTreeMap;

static VECTORS: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/vectors");

/// Every embedded vector file as (file name, parsed JSON), sorted by name.
pub fn all() -> Vec<(String, Value)> {
    load(&VECTORS)
}

fn load(dir: &Dir<'_>) -> Vec<(String, Value)> {
    let mut out: Vec<(String, Value)> = dir
        .files()
        .filter(|f| f.path().extension().is_some_and(|e| e == "json"))
        .map(|f| {
            let name = f
                .path()
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
            let text = f.contents_utf8().unwrap_or_default();
            let v = serde_json::from_str(text).unwrap_or_else(|e| panic!("{name}: {e}"));
            (name, v)
        })
        .collect();
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

/// Runs one vector file. `Err` lists every failed case.
pub fn run_file(file: &Value) -> Result<(), Vec<String>> {
    let mut failures = vec![];
    let id = file["id"].as_str().unwrap_or("?");
    if file["subject"] != "realizer" || file["contract"] != "ui" {
        failures.push(format!(
            "{id}: the envelope needs contract ui and subject realizer"
        ));
    }
    let Some(cases) = file["cases"].as_array() else {
        return Err(vec![format!("{id}: no cases")]);
    };
    if cases.is_empty() {
        failures.push(format!("{id}: no cases"));
    }
    let mut trees: BTreeMap<String, Value> = BTreeMap::new();
    for case in cases {
        let name = case["name"].as_str().unwrap_or("?").to_string();
        if let Err(e) = run_case(case, &mut trees) {
            failures.push(format!("{id} / {name}: {e}"));
        }
    }
    if failures.is_empty() {
        Ok(())
    } else {
        Err(failures)
    }
}

fn run_case(case: &Value, trees: &mut BTreeMap<String, Value>) -> Result<(), String> {
    let name = case["name"].as_str().unwrap_or("?").to_string();
    // Design 5.6 rule 6: every case cites the clause that fixes its expected values.
    if !case["because"]
        .as_str()
        .is_some_and(|b| b.starts_with("UI ") && b.len() > 3)
    {
        return Err("the case needs \"because\": \"UI <clause id>\"".into());
    }
    if let Some(ids) = case.get("ids") {
        return run_ids(ids);
    }
    if let Some(crate_name) = case.get("lives_in_crate").and_then(Value::as_str) {
        // UI-RZ-3b: the vectors are embedded from `vectors/` of this crate, and the file of this case is one of them.
        let here = all()
            .iter()
            .any(|(n, _)| n == "conf::ui_rz_3b_vectors_live_in_botster_ui_contract.json");
        return if crate_name == env!("CARGO_PKG_NAME") && here {
            Ok(())
        } else {
            Err(format!(
                "this crate is {}, and the vectors are embedded from it",
                env!("CARGO_PKG_NAME")
            ))
        };
    }
    if let Some(p) = case.get("parse") {
        return run_parse(p);
    }
    if case.get("badge").is_some() {
        return run_badge(case);
    }
    if let Some(h) = case.get("home") {
        return run_home(h);
    }
    if let Some(u) = case.get("url") {
        return run_url(u);
    }
    if case.get("expect_admitted").is_some()
        || case.get("expect_admission").is_some()
        || case.get("expect_badge_admission").is_some()
    {
        return run_admission(case);
    }
    if case.get("expect_stale").is_some() {
        return run_stale(case);
    }
    let tree = realize_case(case)?;
    let tree_json = serde_json::to_value(&tree).map_err(|e| e.to_string())?;
    // UI-RZ-1: the same inputs give the same tree.
    let again = realize_case(case)?;
    if again != tree {
        return Err("realize is not deterministic".into());
    }
    // UI-RZ-2c: every node is in the vocabulary.
    tree.check_vocabulary()?;
    if case.get("expect_idempotent") == Some(&json!(true)) {
        check_idempotent(case, &tree)?;
    }
    if let Some(expect) = case.get("expect") {
        subset(expect, Some(&tree_json), "$")?;
    }
    if let Some(nodes) = case.get("expect_nodes").and_then(Value::as_object) {
        for (id, want) in nodes {
            let node = tree
                .find(id)
                .ok_or_else(|| format!("no node with id {id}"))?;
            let got = serde_json::to_value(node).map_err(|e| e.to_string())?;
            subset(want, Some(&got), &format!("node {id}"))?;
        }
    }
    for id in case
        .get("expect_absent")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let id = id.as_str().unwrap_or("");
        if tree.find(id).is_some() {
            return Err(format!("node {id} must not be in the tree"));
        }
    }
    if let Some(codes) = case.get("expect_diagnostics").and_then(Value::as_array) {
        for c in codes {
            if !tree
                .diagnostics
                .iter()
                .any(|d| Some(d.code.as_str()) == c.as_str())
            {
                return Err(format!(
                    "missing diagnostic {c}; have {:?}",
                    tree.diagnostics
                ));
            }
        }
    }
    if case.get("expect_no_diagnostics") == Some(&json!(true)) && !tree.diagnostics.is_empty() {
        return Err(format!("unexpected diagnostics {:?}", tree.diagnostics));
    }
    if let Some(other) = case.get("same_ids_as").and_then(Value::as_str) {
        let before = trees
            .get(other)
            .ok_or_else(|| format!("no earlier case {other}"))?;
        let (mut a, mut b) = (node_ids(before), node_ids(&tree_json));
        a.sort();
        b.sort();
        let keys = case.get("compare_ids_under").and_then(Value::as_array);
        let pick = |ids: &Vec<String>| -> Vec<String> {
            match keys {
                None => ids.clone(),
                Some(prefixes) => ids
                    .iter()
                    .filter(|i| {
                        prefixes
                            .iter()
                            .any(|p| p.as_str().is_some_and(|p| i.starts_with(p)))
                    })
                    .cloned()
                    .collect(),
            }
        };
        if pick(&a) != pick(&b) {
            return Err(format!("node ids differ from case {other}"));
        }
    }
    if let Some(other) = case.get("same_tree_as").and_then(Value::as_str) {
        let before = trees
            .get(other)
            .ok_or_else(|| format!("no earlier case {other}"))?;
        if before != &tree_json {
            return Err(format!("the tree differs from case {other}"));
        }
    }
    trees.insert(name, tree_json);
    Ok(())
}

/// Every identity-bearing id of a serialized tree: nodes, slot nodes, table row entries and cell nodes (UI-RZ-4, UI-RZ-5).
fn node_ids(tree: &Value) -> Vec<String> {
    fn walk(n: &Value, out: &mut Vec<String>) {
        if let Some(id) = n["id"].as_str() {
            out.push(id.to_string());
        }
        for c in n["children"].as_array().into_iter().flatten() {
            walk(c, out);
        }
        for list in n["slots"].as_object().into_iter().flat_map(|m| m.values()) {
            for c in list.as_array().into_iter().flatten() {
                walk(c, out);
            }
        }
        for row in n["props"]["rows"].as_array().into_iter().flatten() {
            if let Some(id) = row["id"].as_str() {
                out.push(id.to_string());
            }
            for cell in row["cells"]
                .as_object()
                .into_iter()
                .flat_map(|c| c.values())
            {
                if let Some(id) = cell["id"].as_str() {
                    out.push(id.to_string());
                }
            }
        }
    }
    let mut out = vec![];
    walk(&tree["root"], &mut out);
    for o in tree["overlays"].as_array().into_iter().flatten() {
        walk(o, &mut out);
    }
    out
}

/// Builds the realize input of a case and runs `realize`. A document that does not parse is a local diagnostic (UI-CF-1, UI-VR-2).
fn realize_case(case: &Value) -> Result<crate::node::RealizedTree, String> {
    let mut input = Map::new();
    input.insert("document".into(), case["document"].clone());
    for (from, to) in [
        ("kinds", "kind_snapshots"),
        ("kind_snapshots", "kind_snapshots"),
        ("local_state", "local_state"),
        ("viewport", "viewport"),
        ("capabilities", "capabilities"),
    ] {
        if let Some(v) = case.get(from) {
            input.insert(to.into(), v.clone());
        }
    }
    // A renderer declares `ui_contract_view` (UI-CF-6). A case without `capabilities` is a renderer of ui-contract 1.4
    // that declares nothing else.
    input
        .entry("capabilities")
        .or_insert_with(|| json!({"ui_contract_view": ["1.4"]}));
    let mut input = Value::Object(input);
    strip_raw_numbers(&mut input);
    match serde_json::from_value::<RealizeInput>(input) {
        Ok(i) => Ok(realize(&i)),
        Err(e) => {
            // UI-VR-2: an unknown required property or part makes the view a local diagnostic.
            let id = case["document"]["id"].as_str().unwrap_or("?");
            if case.get("expect_unparsable") == Some(&json!(true)) {
                Ok(diagnostic_tree(
                    id,
                    crate::realize::diagnostic_code("unparsable_view"),
                    &e.to_string(),
                ))
            } else {
                Err(format!("the case does not parse: {e}"))
            }
        }
    }
}

/// Removes the members that are `{"$raw_number": ...}`: a number outside the double range is absent (UI-FD-2).
fn strip_raw_numbers(v: &mut Value) {
    match v {
        Value::Object(m) => {
            m.retain(|_, x| !is_raw_number(x));
            m.values_mut().for_each(strip_raw_numbers);
        }
        Value::Array(a) => {
            a.iter_mut().for_each(|x| {
                *x = if is_raw_number(x) {
                    Value::Null
                } else {
                    x.clone()
                }
            });
            a.iter_mut().for_each(strip_raw_numbers);
        }
        _ => {}
    }
}

fn is_raw_number(v: &Value) -> bool {
    v.as_object()
        .is_some_and(|m| m.len() == 1 && m.contains_key("$raw_number"))
}

/// UI-NV-4 "Applying the changes": the renderer applies `local_changes` to `local_state` and draws again. The output
/// of `realize` wins, and a `realize` of the updated state gives the same tree and an empty `local_changes`.
fn check_idempotent(case: &Value, tree: &crate::node::RealizedTree) -> Result<(), String> {
    let mut next = case.clone();
    let ls = next
        .as_object_mut()
        .ok_or("case")?
        .entry("local_state")
        .or_insert_with(|| json!({}));
    let ls = ls.as_object_mut().ok_or("local_state is an object")?;
    let c = &tree.local_changes;
    for slot in &c.cleared_selections {
        if let Some(Value::Object(m)) = ls.get_mut("selections") {
            m.remove(slot);
        }
    }
    for key in &c.unset_drafts {
        let drafts = ls.entry("drafts").or_insert_with(|| json!({}));
        drafts[key] = json!({"unset": true});
    }
    for e in &c.stamped_errors {
        let errors = ls.entry("errors").or_insert_with(|| json!({}));
        errors["fields"][&e.draft_key] = json!({"code": e.code});
    }
    for node in &c.dropped_view_state {
        if ls.get("focus").and_then(Value::as_str) == Some(node) {
            ls.remove("focus");
        }
        if let Some(Value::Object(m)) = ls.get_mut("scroll") {
            m.remove(node);
        }
    }
    let again = realize_case(&next)?;
    if again.root != tree.root || again.overlays != tree.overlays {
        return Err("the tree of the updated state differs".into());
    }
    if again.local_changes != crate::node::LocalChanges::default() {
        return Err(format!(
            "the updated state still has changes: {:?}",
            again.local_changes
        ));
    }
    Ok(())
}

fn run_badge(case: &Value) -> Result<(), String> {
    let badge: Badge =
        serde_json::from_value(case["badge"].clone()).map_err(|e| format!("badge: {e}"))?;
    let snaps = serde_json::from_value(case.get("kinds").cloned().unwrap_or(json!({})))
        .map_err(|e| format!("kinds: {e}"))?;
    let caps = serde_json::from_value(
        case.get("capabilities")
            .cloned()
            .unwrap_or(json!({"ui_contract_view": ["1.4"]})),
    )
    .map_err(|e| format!("capabilities: {e}"))?;
    let got = crate::navigation::navigation_badge(&badge, &snaps, &caps);
    let got = serde_json::to_value(got).map_err(|e| e.to_string())?;
    subset(&case["expect"], Some(&got), "badge")
}

fn run_home(h: &Value) -> Result<(), String> {
    let views: Vec<(String, Placement)> = h["views"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|v| {
            let p: Placement = serde_json::from_value(v["placement"].clone())
                .map_err(|e| format!("placement: {e}"))?;
            Ok((v["id"].as_str().unwrap_or("").to_string(), p))
        })
        .collect::<Result<_, String>>()?;
    let area: Option<crate::navigation::HomeArea> = match h.get("area") {
        Some(Value::Null) | None => None,
        Some(a) => Some(serde_json::from_value(a.clone()).map_err(|e| format!("area: {e}"))?),
    };
    let got = crate::navigation::home_layout(&views, area.as_ref());
    let got = serde_json::to_value(got).map_err(|e| e.to_string())?;
    subset(&h["expect"], Some(&got), "home")
}

/// The deep-link functions (UI-SL-6, UI-SL-9).
fn run_url(u: &Value) -> Result<(), String> {
    let target = |v: &Value| -> Result<NavigationTarget, String> {
        serde_json::from_value(v.clone()).map_err(|e| format!("target: {e}"))
    };
    let got: Value = match u["op"].as_str() {
        Some("web_url") => json!(crate::links::web_url(
            u["origin"].as_str().unwrap_or(""),
            u["hub_id"].as_str().unwrap_or(""),
            u["package"].as_str().unwrap_or(""),
            &target(&u["target"])?
        )),
        Some("route_path") => json!(crate::links::route_path(
            u["package"].as_str().unwrap_or(""),
            &target(&u["target"])?
        )),
        Some("parse_route") => match crate::links::parse_route(u["route"].as_str().unwrap_or("")) {
            Some((package, t)) => json!({"package": package, "target": t}),
            None => Value::Null,
        },
        Some("is_view_route") => json!(crate::links::is_view_route(
            u["route"].as_str().unwrap_or("")
        )),
        other => return Err(format!("unknown url op {other:?}")),
    };
    subset(&u["expect"], Some(&got), "url")
}

fn run_admission(case: &Value) -> Result<(), String> {
    let ctx: AdmissionContext = match case.get("context") {
        Some(c) => serde_json::from_value(c.clone()).map_err(|e| format!("context: {e}"))?,
        None => AdmissionContext::default(),
    };
    if let Some(b) = case.get("expect_badge_admission") {
        let result = admit_badge(&case["badge_document"], &ctx);
        return match (b.get("code"), result) {
            (None, Ok(_)) => Ok(()),
            (None, Err(f)) => Err(format!("refused: {f:?}")),
            (Some(_), Ok(_)) => Err("admitted".into()),
            (Some(_), Err(f)) => subset(
                b,
                Some(&serde_json::to_value(&f).map_err(|e| e.to_string())?),
                "badge admission",
            ),
        };
    }
    let result = match case.get("document_text").and_then(Value::as_str) {
        Some(text) => admit_text(text, &ctx),
        None => admit(&case["document"], &ctx),
    };
    if case.get("expect_admitted") == Some(&json!(true)) {
        return result.map(|_| ()).map_err(|f| format!("refused: {f:?}"));
    }
    let want = &case["expect_admission"];
    match result {
        Ok(_) => Err(format!("admitted; expected {want}")),
        Err(f) => {
            let got = serde_json::to_value(&f).map_err(|e| e.to_string())?;
            subset(want, Some(&got), "admission")
        }
    }
}

fn run_stale(case: &Value) -> Result<(), String> {
    let ctx: AdmissionContext =
        serde_json::from_value(case["context"].clone()).map_err(|e| e.to_string())?;
    let doc: ViewDocument =
        serde_json::from_value(case["document"].clone()).map_err(|e| e.to_string())?;
    let stale = crate::admission::check_stale(&doc, &ctx).is_some();
    if stale == (case["expect_stale"] == json!(true)) {
        Ok(())
    } else {
        Err(format!("stale is {stale}"))
    }
}

fn run_ids(ids: &Value) -> Result<(), String> {
    match ids["op"].as_str() {
        Some("child") => {
            let id = child_id(
                ids["parent"].as_str().unwrap_or(""),
                ids["segment"].as_str().unwrap_or(""),
            );
            if let Some(want) = ids.get("expect").and_then(Value::as_str) {
                if id != want {
                    return Err(format!("child id {id}, expected {want}"));
                }
            }
            // UI-ID-3: the inverse returns the parts.
            match split_id(&id) {
                Some((p, s))
                    if p == ids["parent"].as_str().unwrap_or("")
                        && s == ids["segment"].as_str().unwrap_or("") =>
                {
                    Ok(())
                }
                other => Err(format!("split_id gave {other:?}")),
            }
        }
        Some("split_none") => match split_id(ids["id"].as_str().unwrap_or("")) {
            None => Ok(()),
            Some(p) => Err(format!("split_id accepted it: {p:?}")),
        },
        Some("draft_key") => {
            let k = draft_key(
                ids["form_node"].as_str().unwrap_or(""),
                ids["field_id"].as_str().unwrap_or(""),
            );
            let want = ids["expect"].as_str().unwrap_or("");
            if k == want {
                Ok(())
            } else {
                Err(format!("draft key {k}"))
            }
        }
        Some("distinct") => {
            let a = child_id(
                ids["a"][0].as_str().unwrap_or(""),
                ids["a"][1].as_str().unwrap_or(""),
            );
            let b = child_id(
                ids["b"][0].as_str().unwrap_or(""),
                ids["b"][1].as_str().unwrap_or(""),
            );
            if a != b {
                Ok(())
            } else {
                Err("two different pairs gave one id".into())
            }
        }
        other => Err(format!("unknown ids op {other:?}")),
    }
}

fn run_parse(p: &Value) -> Result<(), String> {
    let value = p["value"].clone();
    let ok = match p["type"].as_str() {
        Some("ActionOk") => serde_json::from_value::<ActionOk>(value).is_ok(),
        Some("ActionError") => serde_json::from_value::<ActionError>(value).is_ok(),
        Some("FieldError") => serde_json::from_value::<FieldError>(value).is_ok(),
        Some("NavigationTarget") => serde_json::from_value::<NavigationTarget>(value).is_ok(),
        Some("Pred") => serde_json::from_value::<Pred>(value).is_ok(),
        Some("Operand") => serde_json::from_value::<Operand>(value).is_ok(),
        Some("Activate") => serde_json::from_value::<Activate>(value).is_ok(),
        Some("LocalState") => serde_json::from_value::<crate::input::LocalState>(value).is_ok(),
        Some("Capabilities") => serde_json::from_value::<crate::input::Capabilities>(value).is_ok(),
        Some("Badge") => serde_json::from_value::<Badge>(value).is_ok(),
        other => return Err(format!("unknown type {other:?}")),
    };
    if ok == (p["valid"] == json!(true)) {
        Ok(())
    } else {
        Err(format!("parse of {} is {ok}", p["type"]))
    }
}

/// Subset matching (see the module docs).
pub fn subset(want: &Value, got: Option<&Value>, at: &str) -> Result<(), String> {
    if let Some(o) = want.as_object() {
        if o.get("$absent") == Some(&json!(true)) {
            return match got {
                None => Ok(()),
                Some(g) => Err(format!("{at}: expected absent, got {g}")),
            };
        }
        if o.get("$any") == Some(&json!(true)) {
            return got
                .map(|_| ())
                .ok_or_else(|| format!("{at}: expected a value"));
        }
        // A missing object has no keys: it matches only when every wanted key is `$absent`.
        let empty = Map::new();
        let g = match got {
            Some(Value::Object(g)) => g,
            None => &empty,
            Some(other) => return Err(format!("{at}: expected an object, got {other}")),
        };
        for (k, w) in o {
            subset(w, g.get(k), &format!("{at}.{k}"))?;
        }
        return Ok(());
    }
    if let Some(a) = want.as_array() {
        let Some(Value::Array(g)) = got else {
            return Err(format!("{at}: expected an array, got {got:?}"));
        };
        if a.len() != g.len() {
            return Err(format!(
                "{at}: expected {} elements, got {}",
                a.len(),
                g.len()
            ));
        }
        for (i, (w, x)) in a.iter().zip(g).enumerate() {
            subset(w, Some(x), &format!("{at}[{i}]"))?;
        }
        return Ok(());
    }
    if got == Some(want) {
        Ok(())
    } else {
        Err(format!("{at}: expected {want}, got {got:?}"))
    }
}
