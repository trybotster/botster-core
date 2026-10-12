//! The code lists: `ABSENT_REASONS`, `LOCAL_ERROR_CODES` and `DIAGNOSTIC_CODES` agree with the frozen text, and the crate emits
//! exactly these codes.
//!
//! The producers: `absent` and `local_error` (realize.rs), and for diagnostics `diag` (realize.rs), `Diagnostic::new`
//! (admission.rs) and `diagnostic_code` (realize.rs and vectors.rs). Each one checks its code at run time (a debug assertion,
//! which the vectors exercise). This test reads every source file of the crate: the code arguments of the producer calls make the
//! emitted set, and a literal equal to a listed code anywhere else fails. A parameter named `code` is a producer passing its own
//! argument on, so it is skipped. `diagnostic_tree` takes an open code (UI-NV-4), so it is not a producer.

use botster_ui_contract::prelude::{
    diagnostic_tree, ABSENT_REASONS, DIAGNOSTIC_CODES, LOCAL_ERROR_CODES,
};
use std::collections::BTreeSet;

mod common;
use common::calls;

/// The frozen text of UI-NV-2 and UI-NV-4 (ui-contract 1.4, realize completeness).
const NV: &str =
    include_str!("../../../frozen/current/ui-contract-v1.4-realize-completeness-candidate3.md");

/// The local codes that a client sets and `realize` never emits: `required_missing` at submit (UI-NV-2, UI-FM-3).
const CLIENT_ONLY: &[&str] = &["required_missing"];

fn set(list: &[&str]) -> BTreeSet<String> {
    list.iter().map(|s| s.to_string()).collect()
}

/// The string literal of a code argument, or `None` for the forwarded parameter `code`.
fn literal(arg: &str) -> Option<String> {
    if arg == "code" {
        return None;
    }
    let lit = arg.strip_prefix('"').and_then(|a| a.strip_suffix('"'));
    Some(
        lit.unwrap_or_else(|| panic!("code argument {arg:?} is not a literal"))
            .to_string(),
    )
}

/// Every `.rs` file under `dir`, at any depth: (path relative to `dir`, text), sorted.
fn sources_in(dir: &std::path::Path) -> Vec<(String, String)> {
    let mut out = vec![];
    let mut todo = vec![dir.to_path_buf()];
    while let Some(d) = todo.pop() {
        for entry in std::fs::read_dir(&d).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                todo.push(path);
            } else if path.extension().is_some_and(|x| x == "rs") {
                let name = path
                    .strip_prefix(dir)
                    .unwrap()
                    .to_string_lossy()
                    .into_owned();
                out.push((name, std::fs::read_to_string(&path).unwrap()));
            }
        }
    }
    out.sort();
    out
}

/// The source directory of the crate.
fn src_dir() -> &'static std::path::Path {
    std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/src"))
}

/// Every source file of the crate, at any depth.
fn sources() -> Vec<(String, String)> {
    let out = sources_in(src_dir());
    assert!(out.iter().any(|(n, _)| n == "realize.rs"));
    out
}

/// The stray literals in every source file under `dir`.
fn stray_in(dir: &std::path::Path) -> Vec<String> {
    sources_in(dir)
        .iter()
        .flat_map(|(f, src)| stray(f, src))
        .collect()
}

/// The producers of each list: (call name, index of the code argument).
const ABSENT: &[(&str, usize)] = &[("absent", 0)];
const LOCAL: &[(&str, usize)] = &[("local_error", 0)];
const DIAGNOSTIC: &[(&str, usize)] = &[("diag", 0), ("Diagnostic::new", 1), ("diagnostic_code", 0)];

/// The code literals of the producer calls in `src`.
fn emitted_in(src: &str, producers: &[(&str, usize)]) -> Vec<String> {
    producers
        .iter()
        .flat_map(|(name, at)| {
            calls(src, name)
                .into_iter()
                .filter_map(move |args| literal(args[*at]))
        })
        .collect()
}

/// The codes that the producers emit, over every source file.
fn emitted(producers: &[(&str, usize)]) -> BTreeSet<String> {
    sources()
        .iter()
        .flat_map(|(_, src)| emitted_in(src, producers))
        .collect()
}

/// `src` without the definitions of the three lists.
fn without_lists(src: &str) -> String {
    let mut out = src.to_string();
    for name in ["ABSENT_REASONS", "LOCAL_ERROR_CODES", "DIAGNOSTIC_CODES"] {
        let head = format!("pub const {name}: &[&str] = &[");
        if let Some(at) = out.find(&head) {
            let end = at + out[at..].find("];").unwrap() + 2;
            out.replace_range(at..end, "");
        }
    }
    out
}

/// The literals in `src` that equal a listed code outside a producer call and outside the list definitions.
fn stray(file: &str, src: &str) -> Vec<String> {
    let src = without_lists(src);
    let mut produced = emitted_in(&src, ABSENT);
    produced.extend(emitted_in(&src, LOCAL));
    produced.extend(emitted_in(&src, DIAGNOSTIC));
    let mut out = vec![];
    for code in ABSENT_REASONS
        .iter()
        .chain(LOCAL_ERROR_CODES)
        .chain(DIAGNOSTIC_CODES)
    {
        let all = src.matches(&format!("\"{code}\"")).count();
        let ok = produced.iter().filter(|p| p == code).count();
        if all > ok {
            out.push(format!(
                "{file}: \"{code}\" appears {} time(s) outside a producer",
                all - ok
            ));
        }
    }
    out
}

/// The words in backticks of `text` that are lowercase identifiers.
fn ticked(text: &str) -> BTreeSet<String> {
    text.split('`')
        .skip(1)
        .step_by(2)
        .filter(|w| !w.is_empty() && w.chars().all(|c| c.is_ascii_lowercase() || c == '_'))
        .map(str::to_string)
        .collect()
}

/// `text` without its parenthesized parts (a clause citation such as "(UI-FM-3 `required_when`, …)").
fn outside_parens(text: &str) -> String {
    let mut depth = 0;
    text.chars()
        .filter(|&c| {
            match c {
                '(' => depth += 1,
                ')' => depth -= 1,
                _ => return depth == 0,
            }
            false
        })
        .collect()
}

/// The row of `clause` in the frozen text.
fn row(clause: &str) -> &'static str {
    let head = format!("| {clause} |");
    NV.lines()
        .find(|l| l.starts_with(&head))
        .unwrap_or_else(|| panic!("the {clause} row"))
}

/// The part of `text` from `start` up to `end`.
fn between<'a>(text: &'a str, start: &str, end: &str) -> &'a str {
    let rest = &text[text.find(start).unwrap_or_else(|| panic!("{start:?}")) + start.len()..];
    &rest[..rest.find(end).unwrap_or_else(|| panic!("{end:?}"))]
}

#[test]
fn ui_nv_2_names_exactly_the_absent_reasons_and_the_local_error_codes() {
    let nv_2 = row("UI-NV-2");
    assert_eq!(
        ticked(between(nv_2, "- `absent_reason`:", "(UI-RF-1)")),
        set(ABSENT_REASONS)
    );
    assert_eq!(
        ticked(&outside_parens(between(
            nv_2,
            "a local code:",
            ". The `message`"
        ))),
        set(LOCAL_ERROR_CODES)
    );
}

#[test]
fn ui_nv_4_diagnostic_codes_are_a_subset_of_the_emitted_list() {
    // UI-NV-4 gives no closed list, so DIAGNOSTIC_CODES holds what the crate emits (the next test), and the named codes are a subset.
    let list = between(row("UI-NV-4"), "**Diagnostic codes** (", "|");
    assert!(
        list.starts_with("non-exhaustive;"),
        "UI-NV-4 now gives a closed list: {list}"
    );
    let named = ticked(list);
    assert!(!named.is_empty());
    let listed = set(DIAGNOSTIC_CODES);
    let missing: Vec<_> = named.difference(&listed).collect();
    assert!(
        missing.is_empty(),
        "UI-NV-4 names {missing:?}, which DIAGNOSTIC_CODES lacks"
    );
}

#[test]
fn the_crate_emits_exactly_the_listed_codes() {
    assert_eq!(emitted(ABSENT), set(ABSENT_REASONS));
    let local: Vec<&str> = LOCAL_ERROR_CODES
        .iter()
        .copied()
        .filter(|c| !CLIENT_ONLY.contains(c))
        .collect();
    assert_eq!(emitted(LOCAL), set(&local));
    assert_eq!(emitted(DIAGNOSTIC), set(DIAGNOSTIC_CODES));
}

#[test]
fn no_source_file_has_a_listed_code_outside_a_producer() {
    let hits = stray_in(src_dir());
    assert!(hits.is_empty(), "{hits:#?}");
    // A negative case: a bare literal is a hit, and a producer call is not.
    let src =
        "fn f() { self.diag(\"stale\", None, \"x\"); let c = \"stale\"; let r = \"not_found\"; }";
    assert_eq!(stray("sample.rs", src).len(), 2);
    assert!(stray("sample.rs", "fn f() { absent(\"not_found\") }").is_empty());
}

#[test]
fn the_scan_reaches_a_nested_source_file() {
    // A negative case: a stray listed code in `sub/mod.rs` of a sample tree is a hit; the producer call in `lib.rs` is not.
    let root = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join("codes_scan");
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("sub")).unwrap();
    std::fs::write(root.join("lib.rs"), "fn f() { absent(\"not_found\") }").unwrap();
    std::fs::write(root.join("sub/mod.rs"), "const C: &str = \"stale\";").unwrap();
    let hits = stray_in(&root);
    assert_eq!(hits.len(), 1, "{hits:?}");
    assert!(hits[0].starts_with("sub/mod.rs:"), "{hits:?}");
}

#[test]
fn diagnostic_tree_keeps_an_open_code() {
    // UI-NV-4: a renderer shows an unknown code as a generic diagnostic, so the public helper accepts any code.
    let tree = diagnostic_tree("v", "future_diagnostic", "detail");
    assert_eq!(tree.diagnostics[0].code, "future_diagnostic");
}
