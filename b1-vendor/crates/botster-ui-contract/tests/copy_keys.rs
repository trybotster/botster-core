//! The copy keys (UI-NV-6): `COPY_KEYS` is exactly the key set that UI-NV-6 names, and `realize` emits exactly these keys.
//!
//! `realize` builds every copy key with `copy_key`, `copy_key_args` or `copy_node` (realize.rs). This test reads the key argument
//! of each call in the source. A key is a string literal or a `pub const` of realize.rs. The parameter `default_title` is allowed
//! only inside `copy_node`, whose callers this test reads too. At run time, `copy_key` and `copy_key_args` also check each key
//! (a debug assertion, which the vectors exercise).

use botster_ui_contract::prelude::{Text, COPY_KEYS, COPY_KEY_ARGS};
use std::collections::{BTreeMap, BTreeSet};

mod common;
use common::calls;

const REALIZE: &str = include_str!("../src/realize.rs");
/// The frozen text of UI-NV-6 (ui-contract 1.4, realize completeness).
const NV: &str =
    include_str!("../../../frozen/current/ui-contract-v1.4-realize-completeness-candidate3.md");

/// The value of a key argument: a string literal, or a `pub const NAME: &str = "..."` of realize.rs.
fn key_of(arg: &str) -> Option<String> {
    if let Some(lit) = arg.strip_prefix('"').and_then(|a| a.strip_suffix('"')) {
        return Some(lit.to_string());
    }
    let decl = format!("pub const {arg}: &str = \"");
    let at = REALIZE.find(&decl)? + decl.len();
    Some(REALIZE[at..at + REALIZE[at..].find('"')?].to_string())
}

#[test]
fn ui_nv_6_realize_emits_exactly_the_copy_keys() {
    let mut emitted = BTreeSet::new();
    for args in calls(REALIZE, "copy_key")
        .into_iter()
        .chain(calls(REALIZE, "copy_key_args"))
    {
        if args[0] == "default_title" {
            continue; // `copy_node`'s parameter: its callers are read below.
        }
        let key = key_of(args[0]).unwrap_or_else(|| panic!("copy key argument {args:?}"));
        emitted.insert(key);
    }
    let nodes = calls(REALIZE, "copy_node");
    assert!(!nodes.is_empty());
    for args in nodes {
        let key = key_of(args[2]).unwrap_or_else(|| panic!("copy_node default {args:?}"));
        emitted.insert(key);
    }
    let keys: BTreeSet<String> = COPY_KEYS.iter().map(|k| k.to_string()).collect();
    assert_eq!(emitted, keys);
}

/// The UI-NV-6 row of the frozen text.
fn nv_6() -> &'static str {
    NV.lines()
        .find(|l| l.starts_with("| UI-NV-6 |"))
        .expect("the UI-NV-6 row")
}

#[test]
fn ui_nv_6_names_exactly_the_copy_keys_and_args() {
    let row = nv_6();
    let keys_part = &row[row.find("**Keys:**").expect("the key list")..];
    let keys_part = &keys_part[..keys_part
        .find("A renderer shows")
        .expect("the end of the key list")];
    let mut named = BTreeSet::new();
    let mut args = BTreeMap::new();
    for bullet in keys_part.split("<br>- ").skip(1) {
        // The keys of a bullet come before its parenthesis or its args.
        let head = bullet.split(" (").next().unwrap();
        let head = head.split(", with args").next().unwrap();
        for (i, part) in head.split('`').enumerate() {
            if i % 2 == 1 && part.chars().all(|c| c.is_ascii_lowercase() || c == '_') {
                named.insert(part.to_string());
            }
        }
        if let Some(at) = bullet.find("with args `{ ") {
            let rest = &bullet[at + "with args `{ ".len()..];
            let list = &rest[..rest.find(" }`").unwrap()];
            let key = head.trim_matches(|c| c == '`' || c == ',').to_string();
            args.insert(
                key,
                list.split(", ").map(str::to_string).collect::<Vec<_>>(),
            );
        }
    }
    let keys: BTreeSet<String> = COPY_KEYS.iter().map(|k| k.to_string()).collect();
    assert_eq!(named, keys);
    let table: BTreeMap<String, Vec<String>> = COPY_KEY_ARGS
        .iter()
        .map(|(k, a)| (k.to_string(), a.iter().map(|s| s.to_string()).collect()))
        .collect();
    assert_eq!(args, table);
}

#[test]
fn ui_nv_6_text_is_a_string_or_a_copy_key() {
    // A string is authored text, even when it equals a key name.
    let authored: Text = serde_json::from_value(serde_json::json!("cancel")).unwrap();
    assert_eq!(authored, Text::Authored("cancel".into()));
    // An object is a copy key, with optional args.
    let copy: Text = serde_json::from_value(serde_json::json!({"copy": "cancel"})).unwrap();
    assert_eq!(copy, Text::copy("cancel"));
    let wire =
        serde_json::json!({"copy": "view_unavailable", "args": {"view_id": "v", "reason": "r"}});
    let with_args: Text = serde_json::from_value(wire.clone()).unwrap();
    assert_eq!(serde_json::to_value(&with_args).unwrap(), wire);
    assert_eq!(
        serde_json::to_value(Text::copy("submit")).unwrap(),
        serde_json::json!({"copy": "submit"})
    );
    // A number is neither form.
    assert!(serde_json::from_value::<Text>(serde_json::json!(1)).is_err());
}
