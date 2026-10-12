//! The public API of the codec is an allowlist (BUILD.md rule 0; docs/tooling.md). A new `pub` item or a removed one fails
//! this test until `tests/public_api.txt` says so, so a change of the surface is a visible decision in review.
//!
//! The scan reads the source with rustfmt's layout: an item starts at column 0, and a method of an inherent `impl` starts at
//! column 4 under its `impl` header. A line of a macro body is deeper and never counts. The surface that a user imports is the
//! `prelude` module of `lib.rs`; every name in it must be an item of the scan.

use std::collections::BTreeSet;
use std::path::PathBuf;

const ALLOWLIST: &str = include_str!("public_api.txt");

fn src(file: &str) -> String {
    std::fs::read_to_string(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("src")
            .join(file),
    )
    .expect("read a source file")
}

/// The name after the keyword: `pub struct Foo<T>` is `Foo`, `pub fn bar(` is `bar`, `pub const X: u8` is `X`.
fn name_after(rest: &str) -> String {
    rest.trim_start()
        .split(|c: char| !(c.is_alphanumeric() || c == '_'))
        .next()
        .unwrap_or_default()
        .to_string()
}

/// The type of an `impl` header: `impl Foo {`, `impl<T> Foo<T> {`. A trait impl (`impl X for Y`) is not inherent and has none.
fn inherent_type(line: &str) -> Option<String> {
    let rest = line.strip_prefix("impl")?;
    let rest = if rest.starts_with('<') {
        &rest[rest.find('>')? + 1..]
    } else {
        rest
    };
    let header = rest.split('{').next()?;
    if header.contains(" for ") {
        return None;
    }
    Some(name_after(header))
}

fn scan_text(module: &str, text: &str, found: &mut BTreeSet<String>) {
    let mut current_impl: Option<String> = None;
    for line in text.lines() {
        if line.starts_with("impl") {
            current_impl = inherent_type(line);
            continue;
        }
        if line.starts_with('}') {
            current_impl = None;
            continue;
        }
        if let Some(rest) = line.strip_prefix("pub use ") {
            // A root re-export: `pub use path::Name;` or a list. (The prelude re-exports are indented and are read by `prelude_names`.)
            for name in use_names(rest) {
                found.insert(format!("{module}::use {name}"));
            }
        } else if let Some(rest) = line.strip_prefix("pub ") {
            for keyword in [
                "const ", "static ", "fn ", "struct ", "enum ", "trait ", "type ", "mod ",
            ] {
                if let Some(name) = rest.strip_prefix(keyword) {
                    found.insert(format!("{module}::{}", name_after(name)));
                }
            }
        } else if let (Some(owner), Some(rest)) = (&current_impl, line.strip_prefix("    pub ")) {
            for keyword in ["const ", "fn "] {
                if let Some(name) = rest.strip_prefix(keyword) {
                    found.insert(format!("{module}::{owner}::{}", name_after(name)));
                }
            }
        }
    }
}

fn scan_file(file: &str, found: &mut BTreeSet<String>) {
    scan_text(file.trim_end_matches(".rs"), &src(file), found);
}

/// The names of one `pub use` item: `a::b::Name`, `a::{X, Y}`, `a::b::Name as Alias` give `Name`, `X`, `Y`, `Alias`.
fn use_names(item: &str) -> Vec<String> {
    let list = item.split(';').next().unwrap_or_default();
    let list = list.rsplit('{').next().unwrap_or(list);
    list.split(',')
        .map(|n| {
            n.rsplit("::")
                .next()
                .unwrap_or(n)
                .trim()
                .trim_end_matches('}')
                .trim()
        })
        .map(|n| n.rsplit(" as ").next().unwrap_or(n).trim().to_string())
        .filter(|n| !n.is_empty())
        .collect()
}

/// The names of the `prelude` module of `lib.rs`: every identifier of its `pub use` lists.
fn prelude_names(lib: &str) -> BTreeSet<String> {
    let start = lib.find("pub mod prelude {").expect("the prelude module");
    let body = &lib[start..];
    let end = body.find("\n}\n").expect("the end of the prelude module");
    body[..end]
        .split("pub use ")
        .skip(1)
        .flat_map(use_names)
        .collect()
}

/// Every public item of the crate: the items of each module file, the root items and root re-exports of `lib.rs`
/// (given as text, so a test can change it), and the names of the prelude.
fn found_api(lib: &str) -> BTreeSet<String> {
    let mut found = BTreeSet::new();
    let mut files: Vec<String> =
        std::fs::read_dir(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src"))
            .unwrap()
            .map(|e| e.unwrap().file_name().into_string().unwrap())
            // `props.rs` is a `cfg(test)` module, so it is no API.
            .filter(|f| f.ends_with(".rs") && f != "lib.rs" && f != "props.rs")
            .collect();
    files.sort();
    for file in &files {
        scan_file(file, &mut found);
    }
    scan_text("lib", lib, &mut found);
    for name in prelude_names(lib) {
        found.insert(format!("prelude::{name}"));
    }
    found
}

fn allowed() -> BTreeSet<String> {
    ALLOWLIST
        .lines()
        .filter(|l| !l.trim().is_empty() && !l.starts_with('#'))
        .map(str::to_string)
        .collect()
}

#[test]
fn the_public_items_are_the_allowlist() {
    let found = found_api(&src("lib.rs"));
    let allowed = allowed();
    let added: Vec<_> = found.difference(&allowed).collect();
    let removed: Vec<_> = allowed.difference(&found).collect();
    assert!(
        added.is_empty() && removed.is_empty(),
        "the public API changed.\nin source, not in tests/public_api.txt:\n{}\nin tests/public_api.txt, not in source:\n{}",
        added.iter().map(|s| s.as_str()).collect::<Vec<_>>().join("\n"),
        removed.iter().map(|s| s.as_str()).collect::<Vec<_>>().join("\n"),
    );
}

// Negative control: an unlisted export at the crate root, as an item, as a re-export, and as a method of an inherent impl, is found.
#[test]
fn an_unlisted_root_export_is_found() {
    let lib = src("lib.rs");
    for (added, expected) in [
        ("\npub fn unlisted() {}\n", "lib::unlisted"),
        ("\npub mod unlisted_module;\n", "lib::unlisted_module"),
        (
            "\npub use crate::bytes::HexBytes as Unlisted;\n",
            "lib::use Unlisted",
        ),
        (
            "\npub struct Root;\nimpl Root {\n    pub fn method() {}\n}\n",
            "lib::Root::method",
        ),
    ] {
        let found = found_api(&format!("{lib}{added}"));
        assert!(found.contains(expected), "{expected}");
        assert!(!allowed().contains(expected));
    }
    // A name added to the prelude is found too.
    let lib = lib.replace(
        "pub use crate::bytes::{",
        "pub use crate::bytes::{\n        parse_op,",
    );
    assert!(found_api(&lib).contains("prelude::parse_op"));
    assert!(!allowed().contains("prelude::parse_op"));
}

#[test]
fn every_prelude_name_is_a_public_item() {
    let mut items = BTreeSet::new();
    for file in [
        "bytes.rs",
        "error.rs",
        "frames.rs",
        "input.rs",
        "types.rs",
        "wire.rs",
    ] {
        let mut found = BTreeSet::new();
        scan_file(file, &mut found);
        items.extend(
            found
                .into_iter()
                .map(|f| f.rsplit("::").next().unwrap().to_string()),
        );
    }
    for name in prelude_names(&src("lib.rs")) {
        assert!(
            items.contains(&name),
            "prelude exports `{name}`, which is no public item"
        );
    }
}
